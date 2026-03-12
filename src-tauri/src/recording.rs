use std::{path::Path, thread, time::Duration};

use app_core::{CompletedRecording, RecorderSnapshot, RecorderStatus};
use capture::{CaptureController, RecordingOptions};
use tauri::{AppHandle, Manager};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{AppState, emit_recorder_state, emit_runtime_error, window, with_core};

pub fn toggle_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    let status = with_core(app, |core| core.recorder_status())?;

    match status {
        RecorderStatus::Idle => start_recording(app),
        RecorderStatus::Recording | RecorderStatus::Paused => stop_recording(app),
    }
}

pub fn start_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    let settings = with_core(app, |core| {
        core.bootstrap(crate::bootstrap::platform_name()).settings
    })?;
    let output_path = storage::next_recording_path(&settings.output_directory)
        .map_err(|error| error.to_string())?;
    let controller = create_capture_controller(RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: settings.quality_preset,
        mic_enabled: settings.mic_enabled,
    })?;
    let active = controller.active_recording().clone();

    {
        let state = app.state::<AppState>();
        let mut recorder = state
            .recorder
            .lock()
            .map_err(|_| "failed to lock recording runtime".to_string())?;

        if recorder.is_some() {
            return Err("a recording session is already active".to_string());
        }

        *recorder = Some(controller);
    }

    let snapshot = with_core(app, |core| {
        core.start_recording(active.target_label, output_path.display().to_string())
    })?;

    emit_recorder_state(app, &snapshot);
    let _ = window::sync_hud_visibility(app, &snapshot);
    spawn_recorder_ticker(app.clone());
    Ok(snapshot)
}

pub fn stop_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    let mut controller = take_controller(app)?;
    let completed = controller.stop().map_err(|error| error.to_string())?;
    let summary = CompletedRecording {
        title: file_stem(&completed.output_path),
        started_at_label: format_started_at(completed.started_at),
        duration: completed.duration,
        location: completed.output_path.display().to_string(),
        size_bytes: completed.bytes_written,
    };

    let snapshot = with_core(app, |core| core.stop_recording(Some(summary)))?;
    emit_recorder_state(app, &snapshot);
    let _ = window::sync_hud_visibility(app, &snapshot);
    Ok(snapshot)
}

pub fn pause_resume(app: &AppHandle) -> Result<Option<RecorderSnapshot>, String> {
    let status = with_core(app, |core| core.recorder_status())?;

    match status {
        RecorderStatus::Recording => {
            {
                let state = app.state::<AppState>();
                let mut controller = state
                    .recorder
                    .lock()
                    .map_err(|_| "failed to lock recording runtime".to_string())?;
                let controller = controller
                    .as_mut()
                    .ok_or_else(|| "no active recorder process".to_string())?;
                controller.pause().map_err(|error| error.to_string())?;
            }
            let snapshot = with_core(app, |core| core.pause_recording())?;
            if let Some(recorder) = snapshot.as_ref() {
                emit_recorder_state(app, recorder);
                let _ = window::sync_hud_visibility(app, recorder);
            }
            Ok(snapshot)
        }
        RecorderStatus::Paused => {
            {
                let state = app.state::<AppState>();
                let mut controller = state
                    .recorder
                    .lock()
                    .map_err(|_| "failed to lock recording runtime".to_string())?;
                let controller = controller
                    .as_mut()
                    .ok_or_else(|| "no active recorder process".to_string())?;
                controller.resume().map_err(|error| error.to_string())?;
            }
            let snapshot = with_core(app, |core| core.resume_recording())?;
            if let Some(recorder) = snapshot.as_ref() {
                emit_recorder_state(app, recorder);
                let _ = window::sync_hud_visibility(app, recorder);
            }
            Ok(snapshot)
        }
        RecorderStatus::Idle => Ok(None),
    }
}

fn spawn_recorder_ticker(app: AppHandle) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));

            match poll_runtime(&app) {
                Ok(Some(snapshot)) => {
                    emit_recorder_state(&app, &snapshot);
                    let _ = window::sync_hud_visibility(&app, &snapshot);
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    emit_runtime_error(&app, &error);
                    if let Ok(snapshot) = with_core(&app, |core| core.stop_recording(None)) {
                        emit_recorder_state(&app, &snapshot);
                        let _ = window::sync_hud_visibility(&app, &snapshot);
                    }
                    break;
                }
            }

            let Ok((status, snapshot)) =
                with_core(&app, |core| (core.recorder_status(), core.snapshot()))
            else {
                break;
            };

            emit_recorder_state(&app, &snapshot);

            if status == RecorderStatus::Idle {
                break;
            }
        }
    });
}

fn poll_runtime(app: &AppHandle) -> Result<Option<RecorderSnapshot>, String> {
    let completed = {
        let state = app.state::<AppState>();
        let mut recorder = state
            .recorder
            .lock()
            .map_err(|_| "failed to lock recording runtime".to_string())?;

        let Some(controller) = recorder.as_mut() else {
            return Ok(None);
        };

        match controller.poll_finished() {
            Ok(Some(artifact)) => {
                *recorder = None;
                Some(Ok(artifact))
            }
            Ok(None) => None,
            Err(error) => {
                *recorder = None;
                Some(Err(error.to_string()))
            }
        }
    };

    let Some(completed) = completed else {
        return Ok(None);
    };

    let artifact = completed?;
    let summary = CompletedRecording {
        title: file_stem(&artifact.output_path),
        started_at_label: format_started_at(artifact.started_at),
        duration: artifact.duration,
        location: artifact.output_path.display().to_string(),
        size_bytes: artifact.bytes_written,
    };

    with_core(app, |core| core.stop_recording(Some(summary))).map(Some)
}

fn take_controller(app: &AppHandle) -> Result<Box<dyn CaptureController>, String> {
    let state = app.state::<AppState>();
    let mut controller = state
        .recorder
        .lock()
        .map_err(|_| "failed to lock recording runtime".to_string())?;
    controller
        .take()
        .ok_or_else(|| "no active recorder process".to_string())
}

#[cfg(target_os = "macos")]
fn create_capture_controller(
    options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, String> {
    Ok(Box::new(
        capture_macos::FfmpegMacosCapture::start(options).map_err(|error| error.to_string())?,
    ))
}

#[cfg(not(target_os = "macos"))]
fn create_capture_controller(
    _options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, String> {
    Err("real recording backend is only implemented for macOS right now".to_string())
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace('-', " "))
        .unwrap_or_else(|| "Screen recording".to_string())
}

fn format_started_at(started_at: std::time::SystemTime) -> String {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let format = format_description!("[month repr:short] [day], [year] · [hour]:[minute]");

    OffsetDateTime::from(started_at)
        .to_offset(local_offset)
        .format(&format)
        .unwrap_or_else(|_| "Just now".to_string())
}
