use std::{
    io::{BufRead, BufReader},
    process::{Child, ChildStderr, Command, Stdio},
    thread,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, audio_inputs};

const MIC_CHECK_EVENT: &str = "recorder://mic-check-state";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicCheckSnapshot {
    pub active: bool,
    pub level: f32,
    pub has_signal: bool,
    pub error: Option<String>,
}

impl MicCheckSnapshot {
    fn inactive() -> Self {
        Self {
            active: false,
            level: 0.0,
            has_signal: false,
            error: None,
        }
    }

    fn listening() -> Self {
        Self {
            active: true,
            level: 0.0,
            has_signal: false,
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            active: false,
            level: 0.0,
            has_signal: false,
            error: Some(message),
        }
    }
}

pub struct MicCheckProcess {
    child: Child,
}

pub fn start_mic_check(app: &AppHandle) -> Result<MicCheckSnapshot, String> {
    let _ = stop_mic_check(app);
    let (child, stderr) = spawn_platform_mic_check(app)?;

    {
        let state = app.state::<AppState>();
        let mut mic_check = state
            .mic_check
            .lock()
            .map_err(|_| "failed to lock mic check runtime".to_string())?;
        *mic_check = Some(MicCheckProcess { child });
    }

    let app_handle = app.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(level) = parse_rms_level(&line) {
                emit_mic_check_state(
                    &app_handle,
                    &MicCheckSnapshot {
                        active: true,
                        level,
                        has_signal: level >= 0.08,
                        error: None,
                    },
                );
                continue;
            }

            if let Some(message) = detect_probe_error(&line) {
                emit_mic_check_state(&app_handle, &MicCheckSnapshot::error(message));
            }
        }

        let _ = stop_mic_check(&app_handle);
    });

    let snapshot = MicCheckSnapshot::listening();
    emit_mic_check_state(app, &snapshot);
    Ok(snapshot)
}

pub fn stop_mic_check(app: &AppHandle) -> Result<MicCheckSnapshot, String> {
    let state = app.state::<AppState>();
    let mut mic_check = state
        .mic_check
        .lock()
        .map_err(|_| "failed to lock mic check runtime".to_string())?;

    if let Some(mut process) = mic_check.take() {
        let _ = process.child.kill();
        let _ = process.child.wait();
    }

    let snapshot = MicCheckSnapshot::inactive();
    emit_mic_check_state(app, &snapshot);
    Ok(snapshot)
}

pub fn emit_mic_check_state(app: &AppHandle, snapshot: &MicCheckSnapshot) {
    let _ = app.emit(MIC_CHECK_EVENT, snapshot);
}

fn selected_audio_input_id(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let core = state
        .core
        .lock()
        .map_err(|_| "failed to lock app state".to_string())?;
    let selected_audio_input_id = core.settings().audio_input_id;
    let available_audio_inputs = audio_inputs::available_audio_inputs();

    if available_audio_inputs.len() == 1
        && available_audio_inputs
            .first()
            .map(|input| input.id.as_str() == capture::DEFAULT_AUDIO_INPUT_ID)
            .unwrap_or(false)
    {
        return Err(available_audio_inputs[0].description.trim().to_string());
    }

    audio_inputs::normalize_audio_input_selection(&selected_audio_input_id, &available_audio_inputs)
        .ok_or_else(|| "Unable to find a usable microphone input.".to_string())
}

#[cfg(target_os = "linux")]
fn spawn_platform_mic_check(app: &AppHandle) -> Result<(Child, ChildStderr), String> {
    let audio_input = selected_audio_input_id(app)?;
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "info",
            "-nostdin",
            "-f",
            "pulse",
            "-i",
            &audio_input,
            "-af",
            "astats=metadata=1:reset=0.15,ametadata=print:key=lavfi.astats.Overall.RMS_level",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    spawn_probe_process(command)
}

#[cfg(target_os = "macos")]
fn spawn_platform_mic_check(app: &AppHandle) -> Result<(Child, ChildStderr), String> {
    let audio_input = selected_audio_input_id(app)?;
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "info",
            "-nostdin",
            "-f",
            "avfoundation",
            "-i",
            &format!(":{audio_input}"),
            "-af",
            "astats=metadata=1:reset=0.15,ametadata=print:key=lavfi.astats.Overall.RMS_level",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    spawn_probe_process(command)
}

#[cfg(target_os = "windows")]
fn spawn_platform_mic_check(app: &AppHandle) -> Result<(Child, ChildStderr), String> {
    let audio_input = selected_audio_input_id(app)?;
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "info",
            "-nostdin",
            "-f",
            "dshow",
            "-i",
            &format!("audio={audio_input}"),
            "-af",
            "astats=metadata=1:reset=0.15,ametadata=print:key=lavfi.astats.Overall.RMS_level",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    spawn_probe_process(command)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn spawn_platform_mic_check(_app: &AppHandle) -> Result<(Child, ChildStderr), String> {
    Err("Native mic check is currently implemented for macOS, Linux, and Windows.".to_string())
}

fn spawn_probe_process(mut command: Command) -> Result<(Child, ChildStderr), String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start microphone check: {error}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to read microphone monitor output".to_string())?;

    Ok((child, stderr))
}

fn parse_rms_level(line: &str) -> Option<f32> {
    let (_, value) = line.split_once("lavfi.astats.Overall.RMS_level=")?;
    let normalized = match value.trim() {
        "-inf" => 0.0,
        other => {
            let decibels = other.parse::<f32>().ok()?;
            (10f32.powf(decibels / 20.0) * 6.0).clamp(0.0, 1.0)
        }
    };

    Some(normalized)
}

fn detect_probe_error(line: &str) -> Option<String> {
    let lowered = line.to_ascii_lowercase();
    if lowered.contains("permission denied")
        || lowered.contains("not allowed")
        || lowered.contains("not authorized")
        || lowered.contains("operation not permitted")
    {
        return Some("Microphone access was denied by the operating system.".to_string());
    }

    if lowered.contains("no such process")
        || lowered.contains("no such file or directory")
        || lowered.contains("cannot open audio device")
        || lowered.contains("input/output error")
        || lowered.contains("could not find audio only device")
        || lowered.contains("audio device")
        || lowered.contains("directshow audio")
        || lowered.contains("avfoundation")
        || lowered.contains("pulse")
    {
        return Some("The selected microphone input is not available right now.".to_string());
    }

    None
}
