#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::sync::mpsc::{self, Sender};
#[cfg(target_os = "windows")]
use std::time::Duration;
use std::{
    io::{BufRead, BufReader},
    process::{Child, ChildStderr},
    thread,
};
#[cfg(target_os = "linux")]
use std::process::ChildStdout;

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
    pub supported: bool,
    pub error: Option<String>,
}

impl MicCheckSnapshot {
    fn inactive() -> Self {
        Self {
            active: false,
            level: 0.0,
            has_signal: false,
            supported: true,
            error: None,
        }
    }

    fn listening() -> Self {
        Self {
            active: true,
            level: 0.0,
            has_signal: false,
            supported: true,
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            active: false,
            level: 0.0,
            has_signal: false,
            supported: true,
            error: Some(message),
        }
    }

    fn unsupported(message: String) -> Self {
        Self {
            active: false,
            level: 0.0,
            has_signal: false,
            supported: false,
            error: Some(message),
        }
    }
}

pub struct MicCheckProcess {
    runtime: MicCheckRuntime,
}

enum MicCheckRuntime {
    Process {
        child: Child,
    },
    #[cfg(target_os = "windows")]
    NativeWindows {
        stop_tx: Sender<()>,
        worker_handle: thread::JoinHandle<()>,
    },
}

enum MicCheckStart {
    #[allow(dead_code)]
    Process {
        child: Child,
        stdout: Option<ChildStdout>,
        stderr: Option<ChildStderr>,
    },
    #[cfg(target_os = "windows")]
    NativeWindows {
        stop_tx: Sender<()>,
        worker_handle: thread::JoinHandle<()>,
    },
}

pub fn start_mic_check(app: &AppHandle) -> Result<MicCheckSnapshot, String> {
    let _ = stop_mic_check(app);
    let runtime = match spawn_platform_mic_check(app) {
        Ok(process) => process,
        Err(error) => {
            let snapshot = if error.contains("temporarily unavailable on the native") {
                MicCheckSnapshot::unsupported(error)
            } else {
                MicCheckSnapshot::error(error)
            };
            emit_mic_check_state(app, &snapshot);
            return Ok(snapshot);
        }
    };

    {
        let state = app.state::<AppState>();
        let mut mic_check = state
            .mic_check
            .lock()
            .map_err(|_| "failed to lock mic check runtime".to_string())?;
        *mic_check = Some(match runtime {
            MicCheckStart::Process {
                child,
                stdout,
                stderr,
            } => {
                #[cfg(target_os = "linux")]
                if let Some(stdout) = stdout {
                    let app_handle = app.clone();
                    thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            if let Some(level) = parse_rms_level(&line) {
                                emit_mic_check_state(
                                    &app_handle,
                                    &MicCheckSnapshot {
                                        active: true,
                                        level,
                                        has_signal: level >= 0.08,
                                        supported: true,
                                        error: None,
                                    },
                                );
                            }
                        }

                        let _ = stop_mic_check(&app_handle);
                    });
                }

                if let Some(stderr) = stderr {
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
                                        supported: true,
                                        error: None,
                                    },
                                );
                                continue;
                            }

                            if let Some(message) = detect_probe_error(&line) {
                                emit_mic_check_state(&app_handle, &MicCheckSnapshot::error(message));
                            }
                        }
                    });
                }

                MicCheckProcess {
                    runtime: MicCheckRuntime::Process { child },
                }
            }
            #[cfg(target_os = "windows")]
            MicCheckStart::NativeWindows {
                stop_tx,
                worker_handle,
            } => MicCheckProcess {
                runtime: MicCheckRuntime::NativeWindows {
                    stop_tx,
                    worker_handle,
                },
            },
        });
    }

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

    if let Some(process) = mic_check.take() {
        match process.runtime {
            MicCheckRuntime::Process { mut child } => {
                let _ = child.kill();
                let _ = child.wait();
            }
            #[cfg(target_os = "windows")]
            MicCheckRuntime::NativeWindows {
                stop_tx,
                worker_handle,
            } => {
                let _ = stop_tx.send(());
                let _ = worker_handle.join();
            }
        }
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
fn spawn_platform_mic_check(app: &AppHandle) -> Result<MicCheckStart, String> {
    let audio_input = selected_audio_input_id(app)?;
    let mut command = Command::new("gst-launch-1.0");
    command
        .args([
            "-m",
            "pulsesrc",
            "do-timestamp=true",
            &format!("device={audio_input}"),
            "!",
            "audioconvert",
            "!",
            "audioresample",
            "!",
            "level",
            "interval=150000000",
            "post-messages=true",
            "!",
            "fakesink",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    spawn_probe_process(command)
}

#[cfg(target_os = "macos")]
fn spawn_platform_mic_check(app: &AppHandle) -> Result<MicCheckStart, String> {
    let _ = selected_audio_input_id(app)?;
    Err(
        "Live microphone level testing is temporarily unavailable on the native macOS runtime."
            .to_string(),
    )
}

#[cfg(target_os = "windows")]
fn spawn_platform_mic_check(app: &AppHandle) -> Result<MicCheckStart, String> {
    let selected_audio_input = selected_audio_input_id(app)?;
    let available_audio_inputs = audio_inputs::available_audio_inputs();
    let mut worker = capture_windows::native_audio_backend::start_microphone_worker_for_input(
        &selected_audio_input,
        &available_audio_inputs,
    )?;
    let bits_per_sample = worker.foundation().bits_per_sample;
    let (stop_tx, stop_rx) = mpsc::channel();
    let app_handle = app.clone();
    let worker_handle = thread::spawn(move || {
        loop {
            match stop_rx.try_recv() {
                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }

            if let Some(packet) = worker.try_recv_packet() {
                let level = estimate_windows_packet_level(&packet.bytes, bits_per_sample);
                emit_mic_check_state(
                    &app_handle,
                    &MicCheckSnapshot {
                        active: true,
                        level,
                        has_signal: level >= 0.05,
                        supported: true,
                        error: None,
                    },
                );
                continue;
            }

            thread::sleep(Duration::from_millis(24));
        }

        let _ = worker.stop();
    });

    Ok(MicCheckStart::NativeWindows {
        stop_tx,
        worker_handle,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn spawn_platform_mic_check(_app: &AppHandle) -> Result<MicCheckStart, String> {
    Err("Native mic check is currently implemented for macOS, Linux, and Windows.".to_string())
}

#[cfg(target_os = "linux")]
fn spawn_probe_process(mut command: Command) -> Result<MicCheckStart, String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start microphone check: {error}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    Ok(MicCheckStart::Process {
        child,
        stdout,
        stderr,
    })
}

#[cfg(target_os = "windows")]
fn estimate_windows_packet_level(bytes: &[u8], bits_per_sample: u16) -> f32 {
    match bits_per_sample {
        16 => {
            let mut sum = 0.0f32;
            let mut count = 0u32;
            for chunk in bytes.chunks_exact(2) {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32;
                sum += sample * sample;
                count += 1;
            }
            if count == 0 {
                0.0
            } else {
                (sum / count as f32).sqrt().clamp(0.0, 1.0)
            }
        }
        32 => {
            let mut float_sum = 0.0f32;
            let mut float_count = 0u32;
            for chunk in bytes.chunks_exact(4) {
                let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                if sample.is_finite() && sample.abs() <= 8.0 {
                    float_sum += sample * sample;
                    float_count += 1;
                }
            }
            if float_count > 0 {
                return (float_sum / float_count as f32).sqrt().clamp(0.0, 1.0);
            }

            let mut pcm_sum = 0.0f32;
            let mut pcm_count = 0u32;
            for chunk in bytes.chunks_exact(4) {
                let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f32
                    / i32::MAX as f32;
                pcm_sum += sample * sample;
                pcm_count += 1;
            }
            if pcm_count == 0 {
                0.0
            } else {
                (pcm_sum / pcm_count as f32).sqrt().clamp(0.0, 1.0)
            }
        }
        _ => {
            let average = bytes
                .iter()
                .map(|value| (*value as f32 - 128.0).abs() / 128.0)
                .sum::<f32>();
            if bytes.is_empty() {
                0.0
            } else {
                (average / bytes.len() as f32).clamp(0.0, 1.0)
            }
        }
    }
}

fn parse_rms_level(line: &str) -> Option<f32> {
    if let Some(level) = parse_gstreamer_rms_level(line) {
        return Some(level);
    }

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

fn parse_gstreamer_rms_level(line: &str) -> Option<f32> {
    let (_, tail) = line.split_once("rms=(GValueArray)<")?;
    let values = tail.split('>').next()?.trim();
    let decibels = values
        .split(',')
        .filter_map(|value| value.trim().parse::<f32>().ok())
        .collect::<Vec<_>>();

    if decibels.is_empty() {
        return None;
    }

    let average = decibels.iter().copied().sum::<f32>() / decibels.len() as f32;
    Some((10f32.powf(average / 20.0) * 6.0).clamp(0.0, 1.0))
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

    if lowered.contains("no element")
        || lowered.contains("pulsesrc")
        || lowered.contains("failed to connect")
        || lowered.contains("could not open audio")
    {
        return Some(
            "The Linux microphone probe could not attach to the selected audio source.".to_string(),
        );
    }

    if lowered.contains("no such process")
        || lowered.contains("no such file or directory")
        || lowered.contains("cannot open audio device")
        || lowered.contains("input/output error")
        || lowered.contains("could not find audio only device")
        || lowered.contains("audio device")
        || lowered.contains("directshow audio")
        || lowered.contains("pulse")
    {
        return Some("The selected microphone input is not available right now.".to_string());
    }

    None
}
