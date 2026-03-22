use capture::{
    EncoderBackendAvailability, EncoderBackendDescriptor, EncoderBackendFactory,
    EncoderBackendRuntimeReport, RecordingOptions,
};

pub struct AvAssetWriterMacosEncoderBackend;

static AVASSETWRITER_MACOS_ENCODER_BACKEND: AvAssetWriterMacosEncoderBackend =
    AvAssetWriterMacosEncoderBackend;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvAssetWriterOutputPlan {
    pub container_label: String,
    pub encoder_label: String,
    pub codec_name: String,
    pub codec_preset: Option<String>,
    pub quality_preset: String,
    pub output_path: String,
    pub summary: String,
}

pub fn backend() -> &'static dyn EncoderBackendFactory {
    &AVASSETWRITER_MACOS_ENCODER_BACKEND
}

pub fn output_plan(options: &RecordingOptions) -> AvAssetWriterOutputPlan {
    build_output_plan(options)
}

impl EncoderBackendFactory for AvAssetWriterMacosEncoderBackend {
    fn descriptor(&self) -> EncoderBackendDescriptor {
        EncoderBackendDescriptor {
            id: "macos-native-recording-output",
            label: "macOS native recording output",
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        if native_recording_output_runtime_is_supported() {
            EncoderBackendAvailability::Available
        } else {
            let reason = match runtime_summary() {
                Some(summary) => format!(
                    "{summary} Older macOS runtimes do not expose the direct native recording-output lane, so those runtimes fail explicitly instead of using this backend."
                ),
                None => "A fully native macOS recording-output path is only active on macOS 15+; older runtimes fail explicitly instead of using this backend.".to_string(),
            };

            EncoderBackendAvailability::Unavailable { reason }
        }
    }

    fn runtime_report(&self) -> EncoderBackendRuntimeReport {
        EncoderBackendRuntimeReport {
            summary: runtime_summary(),
            preferred_encoder_label: preferred_encoder_label(),
        }
    }
}

pub fn preferred_encoder_label() -> Option<String> {
    runtime_summary().map(|_| "H.264 native recording output".to_string())
}

pub fn runtime_summary() -> Option<String> {
    let version = macos_version()?;
    Some(if native_recording_output_runtime_is_supported() {
        format!(
            "macOS reports version `{version}`; ScreenCaptureKit recording output is active for native file creation on supported runtimes."
        )
    } else {
        format!(
            "macOS reports version `{version}`; native recording output is only fully active on macOS 15+, so older runtimes do not expose this lane."
        )
    })
}

fn build_output_plan(options: &RecordingOptions) -> AvAssetWriterOutputPlan {
    let container_label = options
        .output_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_uppercase())
        .filter(|ext| !ext.is_empty())
        .map(|ext| format!("{ext} container"))
        .unwrap_or_else(|| "QuickTime-compatible container".to_string());
    let (codec_name, codec_preset) = native_recording_output_profile(&options.quality_preset);
    let encoder_label = match codec_preset.as_deref() {
        Some(preset) => format!("{codec_name} · {preset}"),
        None => codec_name.clone(),
    };
    let summary = format!(
        "Native recording output would write {} using {} for preset `{}` to `{}`.",
        container_label,
        encoder_label,
        options.quality_preset,
        options.output_path.display()
    );

    AvAssetWriterOutputPlan {
        container_label,
        encoder_label,
        codec_name,
        codec_preset,
        quality_preset: options.quality_preset.clone(),
        output_path: options.output_path.display().to_string(),
        summary,
    }
}

fn macos_version() -> Option<String> {
    let (major, minor, patch) = super::current_macos_version()?;
    Some(format!("{major}.{minor}.{patch}"))
}

fn native_recording_output_runtime_is_supported() -> bool {
    matches!(super::current_macos_version(), Some((major, _, _)) if major >= 15)
}

fn native_recording_output_profile(quality_preset: &str) -> (String, Option<String>) {
    match quality_preset {
        "4K / 60 fps" | "1440p / 60 fps" => {
            ("hevc".to_string(), Some("high-efficiency".to_string()))
        }
        _ => ("h264".to_string(), Some("balanced".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::build_output_plan;
    use capture::RecordingOptions;
    use std::path::PathBuf;

    #[test]
    fn builds_output_plan_for_mp4() {
        let plan = build_output_plan(&RecordingOptions {
            output_path: PathBuf::from("/tmp/output.mp4"),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: true,
            system_audio_enabled: false,
            capture_target_id: "full-desktop".to_string(),
            audio_input_id: "default".to_string(),
            portal_parent_window: None,
            portal_restore_token: None,
            region_x: 0,
            region_y: 0,
            region_width: 0,
            region_height: 0,
            region_source_capture_target_id: "full-desktop".to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        });

        assert_eq!(plan.container_label, "MP4 container");
        assert!(!plan.codec_name.is_empty());
        assert!(plan.summary.contains("Native recording output"));
        assert!(plan.summary.contains("1080p / 30 fps"));
    }
}
