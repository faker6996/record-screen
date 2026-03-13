#[cfg(target_os = "linux")]
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    thread,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

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

    #[cfg(target_os = "linux")]
    fn listening() -> Self {
        Self {
            active: true,
            level: 0.0,
            has_signal: false,
            error: None,
        }
    }

    #[cfg(target_os = "linux")]
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
    #[cfg(target_os = "linux")]
    child: Child,
}

pub fn start_mic_check(app: &AppHandle) -> Result<MicCheckSnapshot, String> {
    let _ = stop_mic_check(app);

    #[cfg(target_os = "linux")]
    {
        start_linux_mic_check(app)
    }

    #[cfg(not(target_os = "linux"))]
    {
        Err("Native mic check is currently implemented for Linux only.".to_string())
    }
}

pub fn stop_mic_check(app: &AppHandle) -> Result<MicCheckSnapshot, String> {
    let state = app.state::<AppState>();
    let mut mic_check = state
        .mic_check
        .lock()
        .map_err(|_| "failed to lock mic check runtime".to_string())?;

    #[cfg(target_os = "linux")]
    if let Some(mut process) = mic_check.take() {
        let _ = process.child.kill();
        let _ = process.child.wait();
    }

    #[cfg(not(target_os = "linux"))]
    let _ = mic_check.take();

    let snapshot = MicCheckSnapshot::inactive();
    emit_mic_check_state(app, &snapshot);
    Ok(snapshot)
}

pub fn emit_mic_check_state(app: &AppHandle, snapshot: &MicCheckSnapshot) {
    let _ = app.emit(MIC_CHECK_EVENT, snapshot);
}

#[cfg(target_os = "linux")]
fn start_linux_mic_check(app: &AppHandle) -> Result<MicCheckSnapshot, String> {
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
            "default",
            "-af",
            "astats=metadata=1:reset=0.15,ametadata=print:key=lavfi.astats.Overall.RMS_level",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start microphone check: {error}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to read microphone monitor output".to_string())?;

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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
fn detect_probe_error(line: &str) -> Option<String> {
    let lowered = line.to_ascii_lowercase();
    if lowered.contains("permission denied") || lowered.contains("not allowed") {
        return Some("Microphone access was denied by the desktop audio stack.".to_string());
    }

    if lowered.contains("no such process")
        || lowered.contains("no such file or directory")
        || lowered.contains("cannot open audio device")
        || lowered.contains("input/output error")
    {
        return Some("The default microphone source is not available right now.".to_string());
    }

    None
}
