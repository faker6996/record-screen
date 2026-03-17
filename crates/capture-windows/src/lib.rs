#[cfg(target_os = "windows")]
pub mod native_audio_backend;
#[cfg(target_os = "windows")]
pub mod native_capture_backend;
#[cfg(target_os = "windows")]
pub mod native_encoder_backend;

#[cfg(target_os = "windows")]
mod platform {
    use capture::{
        AudioBackendFactory, AudioBackendStatus, AudioInputOption, CaptureBackendFactory,
        CaptureBackendRuntimeSnapshot, CaptureBackendStatus, CaptureError, CaptureTargetOption,
        EncoderBackendFactory, EncoderBackendRuntimeSnapshot, EncoderBackendStatus,
        RecordingOptions, audio_backend_runtime_snapshot,
        audio_backend_statuses as shared_audio_backend_statuses,
        backend_statuses as shared_backend_statuses, capture_backend_runtime_snapshot,
        default_audio_input, encoder_backend_runtime_snapshot,
        encoder_backend_statuses as shared_encoder_backend_statuses,
        explain_audio_backend_selection, explain_capture_backend_selection,
        explain_encoder_backend_selection, select_audio_backend, select_encoder_backend,
    };

    pub fn selected_backend() -> &'static dyn CaptureBackendFactory {
        super::native_capture_backend::backend()
    }

    fn backend_candidates() -> [&'static dyn CaptureBackendFactory; 1] {
        [super::native_capture_backend::backend()]
    }

    pub fn backend_statuses() -> Vec<CaptureBackendStatus> {
        shared_backend_statuses(&backend_candidates())
    }

    pub fn selected_audio_backend() -> &'static dyn AudioBackendFactory {
        select_audio_backend(&audio_backend_candidates())
    }

    fn audio_backend_candidates() -> [&'static dyn AudioBackendFactory; 1] {
        [super::native_audio_backend::backend()]
    }

    pub fn audio_backend_statuses() -> Vec<AudioBackendStatus> {
        shared_audio_backend_statuses(&audio_backend_candidates())
    }

    pub fn selected_encoder_backend() -> &'static dyn EncoderBackendFactory {
        select_encoder_backend(&encoder_backend_candidates())
    }

    fn encoder_backend_candidates() -> [&'static dyn EncoderBackendFactory; 1] {
        [super::native_encoder_backend::backend()]
    }

    pub fn encoder_backend_statuses() -> Vec<EncoderBackendStatus> {
        shared_encoder_backend_statuses(&encoder_backend_candidates())
    }

    #[allow(dead_code)]
    pub fn capture_selection_note() -> String {
        explain_capture_backend_selection(&backend_candidates()).note
    }

    pub fn capture_runtime_snapshot() -> CaptureBackendRuntimeSnapshot {
        capture_backend_runtime_snapshot(&backend_candidates())
    }

    pub fn capture_start_plan_summary(options: &RecordingOptions) -> Option<String> {
        super::native_capture_backend::start_plan(options)
            .ok()
            .map(|plan| plan.summary)
    }

    pub fn capture_execution_plan_summary(options: &RecordingOptions) -> Option<String> {
        super::native_capture_backend::execution_plan(options)
            .ok()
            .map(|plan| plan.summary)
    }

    pub fn capture_runtime_foundation_summary(options: &RecordingOptions) -> Option<String> {
        super::native_capture_backend::runtime_foundation_summary(options)
    }

    pub fn capture_prepared_runtime_summary(options: &RecordingOptions) -> Option<String> {
        super::native_capture_backend::prepared_runtime_summary(options)
    }

    pub fn capture_smoke_lifecycle_summary(options: &RecordingOptions) -> Option<String> {
        super::native_capture_backend::smoke_lifecycle_summary(options)
    }

    pub fn capture_encoder_bridge_smoke_summary(options: &RecordingOptions) -> Option<String> {
        super::native_capture_backend::encoder_bridge_smoke_summary(options)
    }

    #[allow(dead_code)]
    pub fn audio_selection_note() -> String {
        explain_audio_backend_selection(&audio_backend_candidates()).note
    }

    pub fn audio_runtime_snapshot() -> capture::AudioBackendRuntimeSnapshot {
        audio_backend_runtime_snapshot(&audio_backend_candidates())
    }

    #[allow(dead_code)]
    pub fn encoder_selection_note() -> String {
        explain_encoder_backend_selection(&encoder_backend_candidates()).note
    }

    pub fn encoder_runtime_snapshot() -> EncoderBackendRuntimeSnapshot {
        encoder_backend_runtime_snapshot(&encoder_backend_candidates())
    }

    pub fn encoder_output_plan_summary(options: &RecordingOptions) -> Option<String> {
        super::native_encoder_backend::output_plan_summary(options)
    }

    pub fn encoder_sample_bridge_summary(options: &RecordingOptions) -> Option<String> {
        super::native_encoder_backend::sample_bridge_summary(options)
    }

    pub fn encoder_runtime_foundation_summary(options: &RecordingOptions) -> Option<String> {
        super::native_encoder_backend::runtime_foundation_summary(options)
    }

    pub fn list_capture_targets() -> Vec<CaptureTargetOption> {
        super::native_capture_backend::list_capture_targets()
    }

    pub fn list_audio_inputs() -> Vec<AudioInputOption> {
        let mut default_input = default_audio_input();
        if let Some(route_plan) = super::native_audio_backend::route_plan() {
            default_input.description = route_plan.default_input_note;
        }

        let mut inputs = super::native_audio_backend::selectable_audio_inputs();
        inputs.insert(0, default_input);
        inputs
    }

    pub fn preview_target_bounds(
        capture_target_id: &str,
        region_x: u32,
        region_y: u32,
        region_width: u32,
        region_height: u32,
    ) -> Result<(i32, i32, u32, u32), CaptureError> {
        super::native_capture_backend::preview_target_bounds(
            capture_target_id,
            region_x,
            region_y,
            region_width,
            region_height,
        )
    }

    pub fn audio_input_support_summary() -> String {
        match super::native_audio_backend::runtime_plan() {
            Some(plan) => {
                let route_plan = super::native_audio_backend::route_plan();
                let runtime_intent = super::native_audio_backend::runtime_intent(true, false);
                let default_input = route_plan
                    .as_ref()
                    .and_then(|route_plan| route_plan.default_input_label.clone())
                    .or(plan.default_input_name)
                    .unwrap_or_else(|| "not detected".to_string());
                let wasapi_summary =
                    super::native_audio_backend::runtime_summary().unwrap_or_else(|| {
                        "Windows audio probing could not resolve a stable WASAPI candidate yet."
                            .to_string()
                    });
                format!(
                    "Windows reports default input `{default_input}`, {} capture endpoint{}, and {} render endpoint{}. {} {} {wasapi_summary}",
                    plan.capture_endpoint_count,
                    if plan.capture_endpoint_count == 1 {
                        ""
                    } else {
                        "s"
                    },
                    plan.render_endpoint_count,
                    if plan.render_endpoint_count == 1 {
                        ""
                    } else {
                        "s"
                    },
                    route_plan
                        .map(|route_plan| route_plan.default_input_note)
                        .unwrap_or_default(),
                    runtime_intent
                        .map(|intent| intent.summary)
                        .unwrap_or_default(),
                )
            }
            None => {
                "Windows could not inspect WASAPI audio endpoints from this session.".to_string()
            }
        }
    }

    pub fn custom_region_support_summary() -> (bool, String) {
        (
            true,
            "Custom region capture now uses the native Windows.Graphics.Capture path.".to_string(),
        )
    }

    pub fn system_audio_support_summary() -> (bool, String) {
        match super::native_audio_backend::runtime_plan() {
            Some(plan) if plan.render_endpoint_count > 0 => {
                let route_plan = super::native_audio_backend::route_plan();
                let runtime_intent = super::native_audio_backend::runtime_intent(false, true);
                let preferred_render = route_plan
                    .as_ref()
                    .and_then(|route_plan| route_plan.preferred_loopback_label.clone())
                    .unwrap_or_else(|| "not resolved".to_string());
                (
                    true,
                    format!(
                        "Windows reports {} render endpoint{} for the native WASAPI loopback path. {} {} The current preferred render candidate is `{preferred_render}`.",
                        plan.render_endpoint_count,
                        if plan.render_endpoint_count == 1 {
                            ""
                        } else {
                            "s"
                        },
                        route_plan
                            .map(|route_plan| route_plan.loopback_note)
                            .unwrap_or_default(),
                        runtime_intent
                            .map(|intent| intent.summary)
                            .unwrap_or_default(),
                    ),
                )
            }
            Some(_) => (
                false,
                "Windows did not resolve a stable render endpoint for native loopback capture yet."
                    .to_string(),
            ),
            None => (
                false,
                "Windows could not inspect WASAPI audio endpoints from this session.".to_string(),
            ),
        }
    }

    pub fn audio_start_plan_summary(options: &RecordingOptions) -> Option<String> {
        let discovered_audio_inputs = super::native_audio_backend::selectable_audio_inputs();
        Some(
            super::native_audio_backend::start_plan(
                &options.audio_input_id,
                options.mic_enabled,
                options.system_audio_enabled,
                &discovered_audio_inputs,
            )
            .summary,
        )
    }

    pub fn audio_runtime_foundation_summary(options: &RecordingOptions) -> Option<String> {
        super::native_audio_backend::runtime_foundation_summary(
            options.mic_enabled,
            options.system_audio_enabled,
        )
    }

    pub fn audio_smoke_lifecycle_summary(options: &RecordingOptions) -> Option<String> {
        super::native_audio_backend::smoke_lifecycle_summary(
            options.mic_enabled,
            options.system_audio_enabled,
        )
    }
}

#[cfg(target_os = "windows")]
pub use platform::{
    audio_backend_statuses, audio_input_support_summary, audio_runtime_foundation_summary,
    audio_runtime_snapshot, audio_smoke_lifecycle_summary, audio_start_plan_summary,
    backend_statuses, capture_encoder_bridge_smoke_summary, capture_execution_plan_summary,
    capture_prepared_runtime_summary, capture_runtime_foundation_summary, capture_runtime_snapshot,
    capture_smoke_lifecycle_summary, capture_start_plan_summary, custom_region_support_summary,
    encoder_backend_statuses, encoder_output_plan_summary, encoder_runtime_foundation_summary,
    encoder_runtime_snapshot, encoder_sample_bridge_summary, list_audio_inputs,
    list_capture_targets, preview_target_bounds, selected_audio_backend, selected_backend,
    selected_encoder_backend, system_audio_support_summary,
};

#[cfg(not(target_os = "windows"))]
pub fn list_capture_targets() -> Vec<capture::CaptureTargetOption> {
    vec![capture::full_desktop_target()]
}

#[cfg(not(target_os = "windows"))]
pub fn list_audio_inputs() -> Vec<capture::AudioInputOption> {
    vec![capture::default_audio_input()]
}

#[cfg(not(target_os = "windows"))]
pub fn audio_input_support_summary() -> String {
    "Windows native microphone discovery is only available on Windows.".to_string()
}

#[cfg(not(target_os = "windows"))]
pub fn audio_runtime_foundation_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn audio_smoke_lifecycle_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn selected_audio_backend() -> &'static dyn capture::AudioBackendFactory {
    panic!("selected_audio_backend is only available on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn audio_backend_statuses() -> Vec<capture::AudioBackendStatus> {
    vec![]
}

#[cfg(not(target_os = "windows"))]
pub fn selected_encoder_backend() -> &'static dyn capture::EncoderBackendFactory {
    panic!("selected_encoder_backend is only available on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn encoder_backend_statuses() -> Vec<capture::EncoderBackendStatus> {
    vec![]
}

#[cfg(not(target_os = "windows"))]
pub fn custom_region_support_summary() -> (bool, String) {
    (
        false,
        "Windows custom-region support is only available on Windows.".to_string(),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn system_audio_support_summary() -> (bool, String) {
    (
        false,
        "Windows system-audio support is only available on Windows.".to_string(),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn audio_start_plan_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn capture_start_plan_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn capture_execution_plan_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn capture_runtime_foundation_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn capture_prepared_runtime_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn capture_smoke_lifecycle_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn capture_encoder_bridge_smoke_summary(
    _options: &capture::RecordingOptions,
) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn encoder_output_plan_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn encoder_runtime_foundation_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn encoder_sample_bridge_summary(_options: &capture::RecordingOptions) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn preview_target_bounds(
    _capture_target_id: &str,
    _region_x: u32,
    _region_y: u32,
    _region_width: u32,
    _region_height: u32,
) -> Result<(i32, i32, u32, u32), capture::CaptureError> {
    Err(capture::CaptureError::UnsupportedPlatform)
}
