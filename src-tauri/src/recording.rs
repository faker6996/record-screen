use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

use app_core::{CompletedRecording, RecorderSnapshot, RecorderStatus};
use capture::{CaptureController, RecordingOptions};
use tauri::{AppHandle, Manager};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    AppState, audio_inputs, emit_recent_sessions_refresh_request, emit_recorder_state,
    emit_runtime_error, persist_settings, runtime_log, window, with_core,
};

fn sync_hud_for_current_settings(app: &AppHandle, snapshot: &RecorderSnapshot) {
    let show_hud_during_recording =
        with_core(app, |core| core.settings().show_hud_during_recording).unwrap_or(true);
    let _ = window::sync_hud_visibility(app, snapshot, show_hud_during_recording);
}

pub fn toggle_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    let status = with_core(app, |core| core.recorder_status())?;

    match status {
        RecorderStatus::Idle => start_recording(app),
        RecorderStatus::Recording | RecorderStatus::Paused => stop_recording(app),
        RecorderStatus::Finalizing => {
            Err("Recording is still finalizing the output file.".to_string())
        }
    }
}

pub fn start_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    let _ = crate::mic_check::stop_mic_check(app);
    let mut settings = with_core(app, |core| core.settings())?;
    let available_audio_inputs = audio_inputs::available_audio_inputs();
    let available_capture_targets = crate::capture_targets::available_capture_targets(&settings);
    let mut settings_changed = false;
    if let Some(next_audio_input_id) = audio_inputs::normalize_audio_input_selection(
        &settings.audio_input_id,
        &available_audio_inputs,
    ) {
        if next_audio_input_id != settings.audio_input_id {
            settings = with_core(app, |core| core.update_audio_input(next_audio_input_id))?;
            settings_changed = true;
        }
    }
    if let Some(next_region_source_capture_target_id) =
        crate::capture_targets::normalize_custom_region_source_target_id(
            &settings.region_source_capture_target_id,
            &available_capture_targets,
        )
    {
        if next_region_source_capture_target_id != settings.region_source_capture_target_id {
            let previous_target_id = settings.region_source_capture_target_id.clone();
            settings = with_core(app, |core| {
                core.update_custom_region(
                    settings.region_x,
                    settings.region_y,
                    settings.region_width,
                    settings.region_height,
                    Some(next_region_source_capture_target_id.clone()),
                    Some(settings.region_source_origin_x),
                    Some(settings.region_source_origin_y),
                    Some(settings.region_source_scale_factor_milli),
                )
            })?;
            runtime_log::log_runtime_info(&format!(
                "normalized custom-region source target before recording | from={} | to={}",
                previous_target_id, next_region_source_capture_target_id
            ));
            settings_changed = true;
        }
    }
    if settings_changed {
        persist_settings(app)?;
    }
    let output_path = storage::next_recording_path(&settings.output_directory)
        .map_err(|error| error.to_string())?;
    storage::validate_recording_output_path(&output_path).map_err(|error| error.to_string())?;
    let recording_options = RecordingOptions {
        output_path: output_path.clone(),
        quality_preset: settings.quality_preset,
        mic_enabled: settings.mic_enabled,
        system_audio_enabled: settings.system_audio_enabled,
        capture_target_id: settings.capture_target_id,
        audio_input_id: settings.audio_input_id,
        portal_parent_window: window::portal_parent_window_handle(app),
        portal_restore_token: settings.wayland_restore_token.clone(),
        region_x: settings.region_x,
        region_y: settings.region_y,
        region_width: settings.region_width,
        region_height: settings.region_height,
        region_source_capture_target_id: settings.region_source_capture_target_id,
        region_source_origin_x: settings.region_source_origin_x,
        region_source_origin_y: settings.region_source_origin_y,
        region_source_scale_factor_milli: settings.region_source_scale_factor_milli,
    };
    let start_started_at = Instant::now();
    let controller = create_capture_controller(recording_options)?;
    let controller_ready_after = start_started_at.elapsed();

    #[cfg(target_os = "linux")]
    if let Some(next_restore_token) = capture_linux::current_wayland_restore_token() {
        if settings.wayland_restore_token.as_deref() != Some(next_restore_token.as_str()) {
            with_core(app, |core| {
                core.update_wayland_restore_token(Some(next_restore_token.clone()))
            })?;
            persist_settings(app)?;
        }
    }
    let active = controller.active_recording().clone();
    let active_target_label = active.target_label.clone();
    let active_encoder_label = active.encoder_label.clone();
    let can_pause = controller.supports_pause_resume();
    let pause_note = controller.pause_resume_note();

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
        core.start_recording(
            active.target_label,
            active.encoder_label,
            output_path.display().to_string(),
            can_pause,
            pause_note.clone(),
        )
    })?;

    let diagnostics = crate::diagnostics::initial_runtime_diagnostics();
    runtime_log::log_runtime_info(&format!(
        "recording started | target={} | output={} | encoder={} | capture_backend={} | audio_backend={} | encoder_backend={} | can_pause={} | pause_note={} | capture_note={} | audio_note={} | encoder_note={} | controller_ready_ms={}",
        active_target_label,
        output_path.display(),
        active_encoder_label,
        diagnostics.backend_path,
        diagnostics.audio_backend_path,
        diagnostics.encoder_backend_path,
        can_pause,
        pause_note.clone().unwrap_or_else(|| "n/a".to_string()),
        diagnostics.capture_selection_note,
        diagnostics.audio_selection_note,
        diagnostics.encoder_selection_note,
        controller_ready_after.as_millis(),
    ));

    sync_hud_for_current_settings(app, &snapshot);
    emit_recorder_state(app, &snapshot);
    spawn_recorder_ticker(app.clone());
    Ok(snapshot)
}

pub fn stop_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    let controller = take_controller(app)?;
    let snapshot = with_core(app, |core| core.begin_finalizing())?
        .ok_or_else(|| "no active recorder process".to_string())?;
    emit_recorder_state(app, &snapshot);
    sync_hud_for_current_settings(app, &snapshot);
    spawn_stop_finalizer(app.clone(), controller);
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
                if !controller.supports_pause_resume() {
                    return Err(controller.pause_resume_note().unwrap_or_else(|| {
                        "Pause/resume is not available for the active recording backend."
                            .to_string()
                    }));
                }
                controller.pause().map_err(|error| error.to_string())?;
            }
            let snapshot = with_core(app, |core| core.pause_recording())?;
            if let Some(recorder) = snapshot.as_ref() {
                emit_recorder_state(app, recorder);
                sync_hud_for_current_settings(app, recorder);
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
                if !controller.supports_pause_resume() {
                    return Err(controller.pause_resume_note().unwrap_or_else(|| {
                        "Pause/resume is not available for the active recording backend."
                            .to_string()
                    }));
                }
                controller.resume().map_err(|error| error.to_string())?;
            }
            let snapshot = with_core(app, |core| core.resume_recording())?;
            if let Some(recorder) = snapshot.as_ref() {
                emit_recorder_state(app, recorder);
                sync_hud_for_current_settings(app, recorder);
            }
            Ok(snapshot)
        }
        RecorderStatus::Idle => Ok(None),
        RecorderStatus::Finalizing => Err(
            "Recording is finalizing the output file. Pause and resume are unavailable."
                .to_string(),
        ),
    }
}

fn spawn_recorder_ticker(app: AppHandle) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));

            match poll_runtime(&app) {
                Ok(Some(snapshot)) => {
                    emit_recorder_state(&app, &snapshot);
                    sync_hud_for_current_settings(&app, &snapshot);
                    emit_recent_sessions_refresh_request(&app);
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    emit_runtime_error(&app, &error);
                    if let Ok(snapshot) = with_core(&app, |core| core.finish_recording(None)) {
                        emit_recorder_state(&app, &snapshot);
                        sync_hud_for_current_settings(&app, &snapshot);
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

    with_core(app, |core| core.finish_recording(Some(summary))).map(Some)
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

fn spawn_stop_finalizer(app: AppHandle, mut controller: Box<dyn CaptureController>) {
    thread::spawn(move || {
        let finalize_started_at = Instant::now();
        match controller.stop() {
            Ok(completed) => {
                runtime_log::log_runtime_info(&format!(
                    "recording finalized | output={} | bytes={} | duration_secs={} | finalize_ms={}",
                    completed.output_path.display(),
                    completed.bytes_written,
                    completed.duration.as_secs(),
                    finalize_started_at.elapsed().as_millis(),
                ));
                let summary = CompletedRecording {
                    title: file_stem(&completed.output_path),
                    started_at_label: format_started_at(completed.started_at),
                    duration: completed.duration,
                    location: completed.output_path.display().to_string(),
                    size_bytes: completed.bytes_written,
                };

                let _ = with_core(&app, |core| {
                    core.push_completed_recording(summary);
                });
                if let Ok(snapshot) = with_core(&app, |core| core.finish_recording(None)) {
                    emit_recorder_state(&app, &snapshot);
                    sync_hud_for_current_settings(&app, &snapshot);
                }
                emit_recent_sessions_refresh_request(&app);
            }
            Err(error) => {
                runtime_log::log_runtime_error(&format!(
                    "recording finalize failed after {} ms: {}",
                    finalize_started_at.elapsed().as_millis(),
                    error
                ));
                if let Ok(snapshot) = with_core(&app, |core| core.finish_recording(None)) {
                    emit_recorder_state(&app, &snapshot);
                    sync_hud_for_current_settings(&app, &snapshot);
                }
                emit_runtime_error(&app, &error.to_string());
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn create_capture_controller(
    options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, String> {
    capture_macos::selected_backend()
        .start(options)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "linux")]
fn create_capture_controller(
    options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, String> {
    capture_linux::selected_backend()
        .start(options)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn create_capture_controller(
    options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, String> {
    capture_windows::selected_backend()
        .start(options)
        .map_err(|error| error.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn create_capture_controller(
    _options: RecordingOptions,
) -> Result<Box<dyn CaptureController>, String> {
    Err(
        "real recording backend is currently implemented for macOS, Linux, and Windows."
            .to_string(),
    )
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
