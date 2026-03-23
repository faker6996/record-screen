use capture::{
    EncoderBackendAvailability, EncoderBackendDescriptor, EncoderBackendFactory,
    EncoderBackendRuntimeReport,
};
use std::process::Command;

use super::{LinuxDesktopSession, current_desktop_session, wayland_portal};

pub struct GstreamerLinuxEncoderBackend;

static GSTREAMER_LINUX_ENCODER_BACKEND: GstreamerLinuxEncoderBackend = GstreamerLinuxEncoderBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GstreamerEncoderSupport {
    nvh264enc: bool,
    x264enc: bool,
    vaapih264enc: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GstreamerEncoderPlan {
    pub element_name: &'static str,
    pub label: String,
    pub property_args: Vec<String>,
}

pub fn backend() -> &'static dyn EncoderBackendFactory {
    &GSTREAMER_LINUX_ENCODER_BACKEND
}

impl EncoderBackendFactory for GstreamerLinuxEncoderBackend {
    fn descriptor(&self) -> EncoderBackendDescriptor {
        EncoderBackendDescriptor {
            id: "linux-gstreamer-encoder",
            label: "Linux GStreamer encoder",
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        availability_for(
            &current_desktop_session(),
            wayland_portal::gstreamer_pipewire_support(),
        )
    }

    fn runtime_report(&self) -> EncoderBackendRuntimeReport {
        EncoderBackendRuntimeReport {
            summary: runtime_summary(),
            preferred_encoder_label: preferred_encoder_label(),
        }
    }
}

pub fn preferred_encoder_label() -> Option<String> {
    encoder_plan_for_quality("1080p / 30 fps").map(|plan| plan.label)
}

pub fn encoder_plan_for_quality(preset: &str) -> Option<GstreamerEncoderPlan> {
    encoder_plan_for_quality_with_support(preset, probe_encoder_support())
}

fn encoder_plan_for_quality_with_support(
    preset: &str,
    support: GstreamerEncoderSupport,
) -> Option<GstreamerEncoderPlan> {
    let bitrate = crate::gst_bitrate_for_quality(preset).to_string();
    let gop_size = crate::quality_settings(preset).2.to_string();

    // Prefer the most stable Linux short-recording path first.
    if support.x264enc {
        return Some(GstreamerEncoderPlan {
            element_name: "x264enc",
            label: "x264".to_string(),
            property_args: vec![
                format!("speed-preset={}", crate::cpu_preset_for_quality(preset)),
                "tune=zerolatency".to_string(),
                format!("bitrate={bitrate}"),
                format!("key-int-max={gop_size}"),
            ],
        });
    }

    if support.nvh264enc {
        return Some(GstreamerEncoderPlan {
            element_name: "nvh264enc",
            label: "NVENC H.264".to_string(),
            property_args: vec![
                "preset=low-latency-hq".to_string(),
                "rc-mode=cbr".to_string(),
                format!("bitrate={bitrate}"),
                format!("gop-size={gop_size}"),
            ],
        });
    }

    if support.vaapih264enc {
        return Some(GstreamerEncoderPlan {
            element_name: "vaapih264enc",
            label: "VAAPI H.264".to_string(),
            property_args: vec![
                format!("bitrate={bitrate}"),
                format!("keyframe-period={gop_size}"),
            ],
        });
    }

    None
}

fn probe_encoder_support() -> GstreamerEncoderSupport {
    GstreamerEncoderSupport {
        nvh264enc: gst_inspect_available("nvh264enc"),
        x264enc: gst_inspect_available("x264enc"),
        vaapih264enc: gst_inspect_available("vaapih264enc"),
    }
}

pub fn runtime_summary() -> Option<String> {
    let mut parts = Vec::new();
    if gst_inspect_available("pipewiresrc") {
        parts.push("pipewiresrc available".to_string());
    }
    if gst_inspect_available("ximagesrc") {
        parts.push("ximagesrc available".to_string());
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

pub(crate) fn availability_for(
    session: &LinuxDesktopSession,
    support: wayland_portal::PipeWireGstreamerSupport,
) -> EncoderBackendAvailability {
    match session {
        LinuxDesktopSession::WaylandOnly { wayland_display } => match support {
            wayland_portal::PipeWireGstreamerSupport::Available => {
                EncoderBackendAvailability::Available
            }
            wayland_portal::PipeWireGstreamerSupport::Missing => {
                EncoderBackendAvailability::Unavailable {
                    reason: format!(
                        "Wayland session {wayland_display} needs GStreamer PipeWire plugins such as `pipewiresrc`, `x264enc`, and `mp4mux` before the native encoder lane can run."
                    ),
                }
            }
            wayland_portal::PipeWireGstreamerSupport::Unknown => {
                EncoderBackendAvailability::Unavailable {
                    reason: format!(
                        "Wayland session {wayland_display} could not confirm the required GStreamer/PipeWire runtime for the native encoder lane."
                    ),
                }
            }
        },
        LinuxDesktopSession::X11 { .. } | LinuxDesktopSession::WaylandWithX11 { .. } => {
            if gst_inspect_available("ximagesrc")
                && gst_inspect_available("mp4mux")
                && encoder_plan_for_quality("1080p / 30 fps").is_some()
            {
                EncoderBackendAvailability::Available
            } else {
                EncoderBackendAvailability::Unavailable {
                    reason: "X11/XWayland native capture needs `ximagesrc`, `mp4mux`, and at least one usable native H.264 GStreamer encoder before the encoder lane can run.".to_string(),
                }
            }
        }
        LinuxDesktopSession::Headless => EncoderBackendAvailability::Unavailable {
            reason:
                "A desktop session is required before the Linux native encoder lane can be probed."
                    .to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GstreamerEncoderSupport, availability_for, encoder_plan_for_quality,
        encoder_plan_for_quality_with_support, preferred_encoder_label,
    };
    use crate::{LinuxDesktopSession, wayland_portal::PipeWireGstreamerSupport};
    use capture::EncoderBackendAvailability;

    #[test]
    fn preferred_encoder_label_uses_supported_known_values() {
        let label = preferred_encoder_label();
        assert!(
            label.is_none()
                || matches!(
                    label.as_deref(),
                    Some("VAAPI H.264") | Some("NVENC H.264") | Some("x264")
                )
        );
    }

    #[test]
    fn pure_wayland_encoder_availability_can_be_available() {
        assert!(matches!(
            availability_for(
                &LinuxDesktopSession::WaylandOnly {
                    wayland_display: "wayland-0".to_string()
                },
                PipeWireGstreamerSupport::Available
            ),
            EncoderBackendAvailability::Available
        ));
    }

    #[test]
    fn encoder_plan_label_matches_supported_known_values() {
        let plan = encoder_plan_for_quality("1080p / 60 fps");
        assert!(
            plan.is_none()
                || matches!(
                    plan.as_ref().map(|plan| plan.label.as_str()),
                    Some("NVENC H.264") | Some("x264") | Some("VAAPI H.264")
                )
        );
    }

    #[test]
    fn prefers_x264_when_x264_and_nvenc_are_both_available() {
        let plan = encoder_plan_for_quality_with_support(
            "1080p / 30 fps",
            GstreamerEncoderSupport {
                nvh264enc: true,
                x264enc: true,
                vaapih264enc: false,
            },
        )
        .expect("expected a preferred encoder plan");

        assert_eq!(plan.element_name, "x264enc");
        assert_eq!(plan.label, "x264");
    }
}
