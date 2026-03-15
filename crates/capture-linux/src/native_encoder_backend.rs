use capture::{
    EncoderBackendAvailability, EncoderBackendDescriptor, EncoderBackendFactory,
    EncoderBackendFamily, EncoderBackendRuntimeReport,
};
use std::process::Command;

pub struct GstreamerLinuxEncoderBackend;

static GSTREAMER_LINUX_ENCODER_BACKEND: GstreamerLinuxEncoderBackend = GstreamerLinuxEncoderBackend;

pub fn backend() -> &'static dyn EncoderBackendFactory {
    &GSTREAMER_LINUX_ENCODER_BACKEND
}

impl EncoderBackendFactory for GstreamerLinuxEncoderBackend {
    fn descriptor(&self) -> EncoderBackendDescriptor {
        EncoderBackendDescriptor {
            id: "linux-gstreamer-encoder",
            label: "Linux GStreamer encoder",
            family: EncoderBackendFamily::Native,
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        let reason = match runtime_summary() {
            Some(summary) => format!(
                "{summary} The recorder still writes output through ffmpeg instead of a production GStreamer/PipeWire-native encoder pipeline."
            ),
            None => "A GStreamer/PipeWire-native output path is planned for Phase 3, but the recorder still writes output through ffmpeg today.".to_string(),
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
    if gst_inspect_available("vaapih264enc") {
        return Some("VAAPI H.264".to_string());
    }
    if gst_inspect_available("nvh264enc") {
        return Some("NVENC H.264".to_string());
    }
    if gst_inspect_available("x264enc") {
        return Some("x264".to_string());
    }
    None
}

pub fn runtime_summary() -> Option<String> {
    let mut parts = Vec::new();
    if gst_inspect_available("pipewiresrc") {
        parts.push("pipewiresrc available".to_string());
    }
    if let Some(encoder) = preferred_encoder_label() {
        parts.push(format!("preferred GStreamer encoder `{encoder}`"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!(
            "Linux native encoder probing resolved {}.",
            parts.join(", ")
        ))
    }
}

fn gst_inspect_available(element: &str) -> bool {
    Command::new("gst-inspect-1.0")
        .arg(element)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
