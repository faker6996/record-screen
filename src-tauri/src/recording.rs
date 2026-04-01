use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use app_core::{CompletedRecording, RecorderSnapshot, RecorderStatus};
use capture::{CaptureController, RecordingOptions};
use tauri::{AppHandle, Manager};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    AppState, audio_inputs, emit_recent_sessions_refresh_request, emit_recorder_state,
    emit_runtime_error, persist_settings, runtime_log, window, with_core,
};

const RECORDING_STARTUP_WATCHDOG_MS: u64 = 15_000;
const RECORDING_TOGGLE_GUARD_MS: u64 = 900;
static LAST_TOGGLE_REQUEST_AT_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub(crate) enum RecordingRuntime {
    Starting {
        cancel: Arc<AtomicBool>,
        output_path: String,
    },
    Active(Box<dyn CaptureController>),
}

struct StartupContext {
    recording_options: RecordingOptions,
    startup_pending: Arc<AtomicBool>,
    startup_cancel: Arc<AtomicBool>,
    start_started_at: Instant,
    requested_capture_target_id: String,
    output_path_display: String,
    capture_start_plan: String,
    capture_execution_plan: String,
    capture_runtime_foundation: String,
    capture_prepared_runtime: String,
    capture_smoke_lifecycle: String,
    capture_encoder_bridge_smoke: String,
    audio_start_plan: String,
    audio_runtime_foundation: String,
    audio_smoke_lifecycle: String,
    encoder_output_plan: String,
    encoder_runtime_foundation: String,
    encoder_sample_bridge: String,
    #[cfg(target_os = "linux")]
    initial_wayland_restore_token: Option<String>,
}

fn sync_hud_for_current_settings(app: &AppHandle, snapshot: &RecorderSnapshot) {
    let show_hud_during_recording =
        with_core(app, |core| core.settings().show_hud_during_recording).unwrap_or(true);
    if let Err(error) = window::sync_hud_visibility(app, snapshot, show_hud_during_recording) {
        runtime_log::log_runtime_error(&format!(
            "unable to sync HUD visibility while recorder status was {:?}: {}",
            snapshot.status, error
        ));
    }
}

fn sync_custom_region_preview_for_snapshot(app: &AppHandle, _snapshot: &RecorderSnapshot) {
    let capture_target_id = with_core(app, |core| core.settings().capture_target_id)
        .unwrap_or_else(|_| capture::FULL_DESKTOP_TARGET_ID.to_string());
    if capture_target_id != capture::CUSTOM_REGION_TARGET_ID
        || !matches!(_snapshot.status, RecorderStatus::Idle)
    {
        let _ = window::hide_target_preview(app);
    }
}

pub fn toggle_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    let status = with_core(app, |core| core.recorder_status())?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let previous_toggle_at = LAST_TOGGLE_REQUEST_AT_MS.swap(now_ms, Ordering::SeqCst);

    if matches!(status, RecorderStatus::Idle)
        && previous_toggle_at != 0
        && now_ms.saturating_sub(previous_toggle_at) < RECORDING_TOGGLE_GUARD_MS
    {
        runtime_log::log_runtime_info(&format!(
            "recording toggle ignored | status={:?} | delta_ms={}",
            status,
            now_ms.saturating_sub(previous_toggle_at)
        ));
        return with_core(app, |core| core.snapshot());
    }

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
    #[cfg(target_os = "macos")]
    if settings.mic_enabled && permissions::microphone_permission_blocked("macos") {
        let message = permissions::microphone_permission_guidance("macos")
            .unwrap_or_else(|| "Microphone access is blocked in macOS settings.".to_string());
        runtime_log::log_runtime_error(&format!(
            "recording startup blocked before native capture because microphone access is unavailable | target_id={} | reason={}",
            settings.capture_target_id, message
        ));
        return Err(message);
    }
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
    #[cfg(target_os = "windows")]
    let verbose_startup_probes = verbose_startup_probes_enabled();
    #[cfg(target_os = "windows")]
    let capture_start_plan = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::capture_start_plan_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let capture_start_plan = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let capture_execution_plan = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::capture_execution_plan_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let capture_execution_plan = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let capture_runtime_foundation = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::capture_runtime_foundation_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let capture_runtime_foundation = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let capture_prepared_runtime = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::capture_prepared_runtime_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let capture_prepared_runtime = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let capture_smoke_lifecycle = if verbose_startup_probes
        && std::env::var_os("RECORD_SCREEN_WINDOWS_WGC_SMOKE").is_some()
    {
        capture_windows::capture_smoke_lifecycle_summary(&recording_options)
            .unwrap_or_else(|| "n/a".to_string())
    } else {
        "skipped".to_string()
    };
    #[cfg(not(target_os = "windows"))]
    let capture_smoke_lifecycle = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let capture_encoder_bridge_smoke = if verbose_startup_probes
        && std::env::var_os("RECORD_SCREEN_WINDOWS_WGC_MF_SMOKE").is_some()
    {
        capture_windows::capture_encoder_bridge_smoke_summary(&recording_options)
            .unwrap_or_else(|| "n/a".to_string())
    } else {
        "skipped".to_string()
    };
    #[cfg(not(target_os = "windows"))]
    let capture_encoder_bridge_smoke = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let audio_start_plan = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::audio_start_plan_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let audio_start_plan = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let audio_runtime_foundation = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::audio_runtime_foundation_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let audio_runtime_foundation = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let audio_smoke_lifecycle = if verbose_startup_probes
        && std::env::var_os("RECORD_SCREEN_WINDOWS_WASAPI_SMOKE").is_some()
    {
        capture_windows::audio_smoke_lifecycle_summary(&recording_options)
            .unwrap_or_else(|| "n/a".to_string())
    } else {
        "skipped".to_string()
    };
    #[cfg(not(target_os = "windows"))]
    let audio_smoke_lifecycle = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let encoder_output_plan = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::encoder_output_plan_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let encoder_output_plan = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let encoder_runtime_foundation =
        if verbose_startup_probes && std::env::var_os("RECORD_SCREEN_WINDOWS_MF_SMOKE").is_some() {
            capture_windows::encoder_runtime_foundation_summary(&recording_options)
                .unwrap_or_else(|| "n/a".to_string())
        } else {
            "skipped".to_string()
        };
    #[cfg(not(target_os = "windows"))]
    let encoder_runtime_foundation = "n/a".to_string();
    #[cfg(target_os = "windows")]
    let encoder_sample_bridge = windows_startup_probe_summary(verbose_startup_probes, || {
        capture_windows::encoder_sample_bridge_summary(&recording_options)
    });
    #[cfg(not(target_os = "windows"))]
    let encoder_sample_bridge = "n/a".to_string();
    let requested_capture_target_id = recording_options.capture_target_id.clone();
    runtime_log::log_runtime_info(&format!(
        "recording startup requested | target_id={} | audio_input_id={} | output={} | quality={} | mic_enabled={} | system_audio_enabled={} | capture_start_plan={} | capture_execution_plan={} | audio_start_plan={} | encoder_output_plan={}",
        requested_capture_target_id,
        recording_options.audio_input_id,
        output_path.display(),
        recording_options.quality_preset,
        recording_options.mic_enabled,
        recording_options.system_audio_enabled,
        capture_start_plan,
        capture_execution_plan,
        audio_start_plan,
        encoder_output_plan,
    ));
    let startup_pending = Arc::new(AtomicBool::new(true));
    let startup_cancel = Arc::new(AtomicBool::new(false));
    let startup_pending_for_watchdog = Arc::clone(&startup_pending);
    let startup_watch_target = requested_capture_target_id.clone();
    let startup_watch_output = output_path.display().to_string();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(RECORDING_STARTUP_WATCHDOG_MS));
        if startup_pending_for_watchdog.load(Ordering::SeqCst) {
            runtime_log::log_runtime_error(&format!(
                "recording startup still pending after {} ms | target_id={} | output={}",
                RECORDING_STARTUP_WATCHDOG_MS, startup_watch_target, startup_watch_output
            ));
        }
    });
    let selected_target_label = available_capture_targets
        .iter()
        .find(|target| target.id == requested_capture_target_id)
        .map(|target| target.label.clone())
        .unwrap_or_else(|| "Display".to_string());
    let startup_pause_note = "Recorder is still preparing the native capture session.".to_string();

    {
        let state = app.state::<AppState>();
        let mut recorder = state
            .recorder
            .lock()
            .map_err(|_| "failed to lock recording runtime".to_string())?;

        if recorder.is_some() {
            return Err("a recording session is already active".to_string());
        }

        *recorder = Some(RecordingRuntime::Starting {
            cancel: Arc::clone(&startup_cancel),
            output_path: output_path.display().to_string(),
        });
    }

    let snapshot = with_core(app, |core| {
        core.start_recording(
            selected_target_label.clone(),
            output_path.display().to_string(),
            Some(startup_pause_note.clone()),
        )
    })?;
    sync_custom_region_preview_for_snapshot(app, &snapshot);
    sync_hud_for_current_settings(app, &snapshot);
    emit_recorder_state(app, &snapshot);
    spawn_recorder_ticker(app.clone());
    spawn_recording_startup(
        app.clone(),
        StartupContext {
            recording_options,
            startup_pending,
            startup_cancel,
            start_started_at,
            requested_capture_target_id,
            output_path_display: output_path.display().to_string(),
            capture_start_plan,
            capture_execution_plan,
            capture_runtime_foundation,
            capture_prepared_runtime,
            capture_smoke_lifecycle,
            capture_encoder_bridge_smoke,
            audio_start_plan,
            audio_runtime_foundation,
            audio_smoke_lifecycle,
            encoder_output_plan,
            encoder_runtime_foundation,
            encoder_sample_bridge,
            #[cfg(target_os = "linux")]
            initial_wayland_restore_token: settings.wayland_restore_token,
        },
    );
    Ok(snapshot)
}

pub fn stop_recording(app: &AppHandle) -> Result<RecorderSnapshot, String> {
    runtime_log::log_runtime_info("recording stop requested | reason=toggle");
    if cancel_pending_startup(app)? {
        let snapshot = with_core(app, |core| core.finish_recording(None))?;
        emit_recorder_state(app, &snapshot);
        sync_hud_for_current_settings(app, &snapshot);
        sync_custom_region_preview_for_snapshot(app, &snapshot);
        return Ok(snapshot);
    }

    let controller = take_controller(app)?;
    let snapshot = with_core(app, |core| core.begin_finalizing())?
        .ok_or_else(|| "no active recorder process".to_string())?;
    emit_recorder_state(app, &snapshot);
    sync_hud_for_current_settings(app, &snapshot);
    sync_custom_region_preview_for_snapshot(app, &snapshot);
    spawn_stop_finalizer(app.clone(), controller);
    Ok(snapshot)
}

pub fn finalize_active_recording_before_exit(app: &AppHandle) {
    let recorder_status = match with_core(app, |core| core.recorder_status()) {
        Ok(status) => status,
        Err(error) => {
            runtime_log::log_runtime_error(&format!(
                "unable to inspect recorder state during app exit: {}",
                error
            ));
            return;
        }
    };

    if !matches!(
        recorder_status,
        RecorderStatus::Recording | RecorderStatus::Paused | RecorderStatus::Finalizing
    ) {
        return;
    }

    runtime_log::log_runtime_info(&format!(
        "recording exit cleanup requested | recorder_status={:?}",
        recorder_status
    ));

    let runtime = {
        let state = app.state::<AppState>();
        let mut recorder = match state.recorder.lock() {
            Ok(recorder) => recorder,
            Err(_) => {
                runtime_log::log_runtime_error(
                    "failed to lock recording runtime during app-exit cleanup",
                );
                return;
            }
        };

        recorder.take()
    };

    let Some(runtime) = runtime else {
        runtime_log::log_runtime_info(
            "recording exit cleanup skipped because no active controller was present",
        );
        return;
    };

    let mut controller = match runtime {
        RecordingRuntime::Starting { cancel, .. } => {
            cancel.store(true, Ordering::SeqCst);
            if let Ok(snapshot) = with_core(app, |core| core.finish_recording(None)) {
                emit_recorder_state(app, &snapshot);
                sync_hud_for_current_settings(app, &snapshot);
                sync_custom_region_preview_for_snapshot(app, &snapshot);
            }
            runtime_log::log_runtime_info(
                "recording exit cleanup canceled a native startup that was still pending",
            );
            return;
        }
        RecordingRuntime::Active(controller) => controller,
    };

    if matches!(
        recorder_status,
        RecorderStatus::Recording | RecorderStatus::Paused
    ) {
        match with_core(app, |core| core.begin_finalizing()) {
            Ok(Some(snapshot)) => {
                emit_recorder_state(app, &snapshot);
                sync_hud_for_current_settings(app, &snapshot);
            }
            Ok(None) => {
                runtime_log::log_runtime_info(
                    "recording exit cleanup found no active recorder after entering finalizing",
                );
            }
            Err(error) => runtime_log::log_runtime_error(&format!(
                "recording exit cleanup could not transition recorder to finalizing: {}",
                error
            )),
        }
    }

    let finalize_started_at = Instant::now();
    match controller.stop() {
        Ok(completed) => {
            runtime_log::log_runtime_info(&format!(
                "recording finalized during app exit | output={} | bytes={} | duration_secs={} | finalize_ms={}",
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

            let _ = with_core(app, |core| {
                core.push_completed_recording(summary);
            });
            if let Ok(snapshot) = with_core(app, |core| core.finish_recording(None)) {
                emit_recorder_state(app, &snapshot);
                sync_hud_for_current_settings(app, &snapshot);
                sync_custom_region_preview_for_snapshot(app, &snapshot);
            }
            emit_recent_sessions_refresh_request(app);
        }
        Err(error) => {
            runtime_log::log_runtime_error(&format!(
                "recording finalize failed during app exit after {} ms: {}",
                finalize_started_at.elapsed().as_millis(),
                error
            ));
            if let Ok(snapshot) = with_core(app, |core| core.finish_recording(None)) {
                emit_recorder_state(app, &snapshot);
                sync_hud_for_current_settings(app, &snapshot);
            }
        }
    }
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
                let controller = match controller.as_mut() {
                    Some(RecordingRuntime::Active(controller)) => controller,
                    Some(RecordingRuntime::Starting { .. }) => {
                        return Err(
                            "Recording is still preparing the native capture session.".to_string()
                        );
                    }
                    None => return Err("no active recorder process".to_string()),
                };
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
                let controller = match controller.as_mut() {
                    Some(RecordingRuntime::Active(controller)) => controller,
                    Some(RecordingRuntime::Starting { .. }) => {
                        return Err(
                            "Recording is still preparing the native capture session.".to_string()
                        );
                    }
                    None => return Err("no active recorder process".to_string()),
                };
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

fn cancel_pending_startup(app: &AppHandle) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let mut recorder = state
        .recorder
        .lock()
        .map_err(|_| "failed to lock recording runtime".to_string())?;

    match recorder.take() {
        Some(RecordingRuntime::Starting {
            cancel,
            output_path,
        }) => {
            cancel.store(true, Ordering::SeqCst);
            cleanup_aborted_output_path(Path::new(&output_path));
            runtime_log::log_runtime_info(&format!(
                "recording startup canceled before native controller was ready | output={}",
                output_path
            ));
            Ok(true)
        }
        Some(runtime) => {
            *recorder = Some(runtime);
            Ok(false)
        }
        None => Ok(false),
    }
}

fn spawn_recording_startup(app: AppHandle, context: StartupContext) {
    thread::spawn(move || {
        let StartupContext {
            recording_options,
            startup_pending,
            startup_cancel,
            start_started_at,
            requested_capture_target_id,
            output_path_display,
            capture_start_plan,
            capture_execution_plan,
            capture_runtime_foundation,
            capture_prepared_runtime,
            capture_smoke_lifecycle,
            capture_encoder_bridge_smoke,
            audio_start_plan,
            audio_runtime_foundation,
            audio_smoke_lifecycle,
            encoder_output_plan,
            encoder_runtime_foundation,
            encoder_sample_bridge,
            #[cfg(target_os = "linux")]
            initial_wayland_restore_token,
        } = context;

        let controller = match create_capture_controller(recording_options) {
            Ok(controller) => controller,
            Err(error) => {
                startup_pending.store(false, Ordering::SeqCst);
                if startup_cancel.load(Ordering::SeqCst) {
                    return;
                }

                {
                    let state = app.state::<AppState>();
                    if let Ok(mut recorder) = state.recorder.lock() {
                        *recorder = None;
                    }
                }

                runtime_log::log_runtime_error(&format!(
                    "recording startup failed after {} ms | target_id={} | output={} | error={}",
                    start_started_at.elapsed().as_millis(),
                    requested_capture_target_id,
                    output_path_display,
                    error
                ));

                if let Ok(snapshot) = with_core(&app, |core| core.finish_recording(None)) {
                    emit_recorder_state(&app, &snapshot);
                    sync_hud_for_current_settings(&app, &snapshot);
                }
                emit_runtime_error(&app, &error);
                return;
            }
        };

        startup_pending.store(false, Ordering::SeqCst);

        if startup_cancel.load(Ordering::SeqCst) {
            let mut controller = controller;
            let aborted_output_path = controller.active_recording().output_path.clone();
            let _ = controller.stop();
            cleanup_aborted_output_path(&aborted_output_path);
            return;
        }

        #[cfg(target_os = "linux")]
        if let Some(next_restore_token) = capture_linux::current_wayland_restore_token() {
            if initial_wayland_restore_token.as_deref() != Some(next_restore_token.as_str()) {
                if with_core(&app, |core| {
                    core.update_wayland_restore_token(Some(next_restore_token.clone()))
                })
                .is_ok()
                {
                    let _ = persist_settings(&app);
                }
            }
        }

        let active = controller.active_recording().clone();
        let active_target_label = active.target_label.clone();
        let active_encoder_label = active.encoder_label.clone();
        let can_pause = controller.supports_pause_resume();
        let pause_note = controller.pause_resume_note();
        let controller_ready_after = start_started_at.elapsed();

        {
            let state = app.state::<AppState>();
            let mut recorder = match state.recorder.lock() {
                Ok(recorder) => recorder,
                Err(_) => {
                    let mut controller = controller;
                    let _ = controller.stop();
                    emit_runtime_error(
                        &app,
                        "failed to lock recording runtime after the native controller became ready",
                    );
                    return;
                }
            };

            match recorder.take() {
                Some(RecordingRuntime::Starting { cancel, .. }) => {
                    if cancel.load(Ordering::SeqCst) {
                        drop(recorder);
                        let mut controller = controller;
                        let _ = controller.stop();
                        return;
                    }
                    *recorder = Some(RecordingRuntime::Active(controller));
                }
                Some(runtime) => {
                    *recorder = Some(runtime);
                    let mut controller = controller;
                    let _ = controller.stop();
                    emit_runtime_error(
                        &app,
                        "recording runtime changed unexpectedly while the native controller was starting",
                    );
                    return;
                }
                None => {
                    let mut controller = controller;
                    let _ = controller.stop();
                    return;
                }
            }
        }

        let snapshot = match with_core(&app, |core| {
            core.complete_recording_startup(
                active.target_label,
                active.encoder_label,
                can_pause,
                pause_note.clone(),
            )
        }) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                let _ = cancel_pending_startup(&app);
                return;
            }
            Err(error) => {
                emit_runtime_error(&app, &error);
                return;
            }
        };

        let diagnostics = crate::diagnostics::initial_runtime_diagnostics();
        runtime_log::log_runtime_info(&format!(
            "recording started | target={} | output={} | encoder={} | capture_backend={} | audio_backend={} | encoder_backend={} | can_pause={} | pause_note={} | capture_note={} | audio_note={} | encoder_note={} | capture_start_plan={} | capture_execution_plan={} | capture_runtime_foundation={} | capture_prepared_runtime={} | capture_smoke_lifecycle={} | capture_encoder_bridge_smoke={} | audio_start_plan={} | audio_runtime_foundation={} | audio_smoke_lifecycle={} | encoder_output_plan={} | encoder_runtime_foundation={} | encoder_sample_bridge={} | controller_ready_ms={}",
            active_target_label,
            output_path_display,
            active_encoder_label,
            diagnostics.backend_path,
            diagnostics.audio_backend_path,
            diagnostics.encoder_backend_path,
            can_pause,
            pause_note.clone().unwrap_or_else(|| "n/a".to_string()),
            diagnostics.capture_selection_note,
            diagnostics.audio_selection_note,
            diagnostics.encoder_selection_note,
            capture_start_plan,
            capture_execution_plan,
            capture_runtime_foundation,
            capture_prepared_runtime,
            capture_smoke_lifecycle,
            capture_encoder_bridge_smoke,
            audio_start_plan,
            audio_runtime_foundation,
            audio_smoke_lifecycle,
            encoder_output_plan,
            encoder_runtime_foundation,
            encoder_sample_bridge,
            controller_ready_after.as_millis(),
        ));

        emit_recorder_state(&app, &snapshot);
        sync_hud_for_current_settings(&app, &snapshot);
    });
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

        let Some(runtime) = recorder.as_mut() else {
            return Ok(None);
        };

        match runtime {
            RecordingRuntime::Starting { .. } => None,
            RecordingRuntime::Active(controller) => match controller.poll_finished() {
                Ok(Some(artifact)) => {
                    *recorder = None;
                    Some(Ok(artifact))
                }
                Ok(None) => None,
                Err(error) => {
                    *recorder = None;
                    Some(Err(error.to_string()))
                }
            },
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
    match controller.take() {
        Some(RecordingRuntime::Active(controller)) => Ok(controller),
        Some(runtime) => {
            *controller = Some(runtime);
            Err("recording is still preparing the native capture session.".to_string())
        }
        None => Err("no active recorder process".to_string()),
    }
}

fn spawn_stop_finalizer(app: AppHandle, mut controller: Box<dyn CaptureController>) {
    thread::spawn(move || {
        let finalize_started_at = Instant::now();
        let output_path = controller.active_recording().output_path.clone();
        match controller.stop() {
            Ok(completed) => {
                if completed.bytes_written == 0 {
                    cleanup_aborted_output_path(&output_path);
                    runtime_log::log_runtime_info(&format!(
                        "recording stopped before any media data was written | finalize_ms={}",
                        finalize_started_at.elapsed().as_millis(),
                    ));
                    if let Ok(snapshot) = with_core(&app, |core| core.finish_recording(None)) {
                        emit_recorder_state(&app, &snapshot);
                        sync_hud_for_current_settings(&app, &snapshot);
                        sync_custom_region_preview_for_snapshot(&app, &snapshot);
                    }
                    return;
                }

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
                    sync_custom_region_preview_for_snapshot(&app, &snapshot);
                }
                emit_recent_sessions_refresh_request(&app);
            }
            Err(error) => {
                if should_treat_finalize_as_empty_stop(&error.to_string()) {
                    cleanup_aborted_output_path(&output_path);
                    runtime_log::log_runtime_info(&format!(
                        "recording stopped before any media data was written | finalize_ms={}",
                        finalize_started_at.elapsed().as_millis(),
                    ));
                    if let Ok(snapshot) = with_core(&app, |core| core.finish_recording(None)) {
                        emit_recorder_state(&app, &snapshot);
                        sync_hud_for_current_settings(&app, &snapshot);
                        sync_custom_region_preview_for_snapshot(&app, &snapshot);
                    }
                    return;
                }

                runtime_log::log_runtime_error(&format!(
                    "recording finalize failed after {} ms: {}",
                    finalize_started_at.elapsed().as_millis(),
                    error
                ));
                if let Ok(snapshot) = with_core(&app, |core| core.finish_recording(None)) {
                    emit_recorder_state(&app, &snapshot);
                    sync_hud_for_current_settings(&app, &snapshot);
                    sync_custom_region_preview_for_snapshot(&app, &snapshot);
                }
                emit_runtime_error(&app, &error.to_string());
            }
        }
    });
}

#[cfg(target_os = "windows")]
fn verbose_startup_probes_enabled() -> bool {
    std::env::var_os("RECORD_SCREEN_VERBOSE_STARTUP").is_some()
}

#[cfg(target_os = "windows")]
fn windows_startup_probe_summary(enabled: bool, load: impl FnOnce() -> Option<String>) -> String {
    if !enabled {
        return "skipped".to_string();
    }

    load().unwrap_or_else(|| "n/a".to_string())
}

fn should_treat_finalize_as_empty_stop(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("failed to inspect recording output")
        || normalized.contains("no such file or directory")
}

fn cleanup_aborted_output_path(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {
            runtime_log::log_runtime_info(&format!(
                "removed aborted recording output | output={}",
                path.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            runtime_log::log_runtime_error(&format!(
                "failed to remove aborted recording output | output={} | error={}",
                path.display(),
                error
            ));
        }
    }
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
