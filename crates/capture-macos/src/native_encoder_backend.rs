use capture::{
    EncoderBackendAvailability, EncoderBackendDescriptor, EncoderBackendFactory,
    EncoderBackendFamily, EncoderBackendRuntimeReport, RecordingOptions,
};
use std::process::Command;

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
            id: "macos-avassetwriter",
            label: "macOS AVAssetWriter",
            family: EncoderBackendFamily::Native,
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        let reason = match runtime_summary() {
            Some(summary) => format!(
                "{summary} The recorder still writes output through the ffmpeg / AVFoundation pipeline instead of an AVAssetWriter-native encoder runtime."
            ),
            None => "A native AVAssetWriter / VideoToolbox encoder path is planned for Phase 3, but the recorder still writes output through ffmpeg today.".to_string(),
        };

        EncoderBackendAvailability::Unavailable { reason }
    }

    fn runtime_report(&self) -> EncoderBackendRuntimeReport {
        EncoderBackendRuntimeReport {
            summary: runtime_summary(),
            preferred_encoder_label: preferred_encoder_label(),
        }
    }
}

pub fn preferred_encoder_label() -> Option<String> {
    runtime_summary().map(|_| "H.264 VideoToolbox".to_string())
}

pub fn runtime_summary() -> Option<String> {
    let version = macos_version()?;
    Some(format!(
        "macOS reports version `{version}`; AVAssetWriter with VideoToolbox is the planned native encoder path."
    ))
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
    let (codec_name, codec_preset) = current_runtime_encoder_profile(&options.quality_preset);
    let encoder_label = match codec_preset.as_deref() {
        Some(preset) => format!("{codec_name} · {preset}"),
        None => codec_name.clone(),
    };
    let summary = format!(
        "AVAssetWriter output plan would write {} using {} for preset `{}` to `{}`.",
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
    let output = Command::new("sw_vers")
        .args(["-productVersion"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

fn current_runtime_encoder_profile(quality_preset: &str) -> (String, Option<String>) {
    let encoders = ffmpeg_encoders().unwrap_or_default();
    if encoders.contains("h264_videotoolbox") {
        ("h264_videotoolbox".to_string(), None)
    } else {
        (
            "libx264".to_string(),
            Some(cpu_preset_for_quality(quality_preset).to_string()),
        )
    }
}

fn ffmpeg_encoders() -> Option<String> {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .ok()?;
    Some(
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .to_ascii_lowercase(),
    )
}

fn cpu_preset_for_quality(preset: &str) -> &'static str {
    match preset {
        "4K / 60 fps" | "1440p / 60 fps" => "ultrafast",
        "1080p / 60 fps" => "superfast",
        _ => "veryfast",
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
        assert!(plan.summary.contains("AVAssetWriter output plan"));
        assert!(plan.summary.contains("1080p / 30 fps"));
    }
}
