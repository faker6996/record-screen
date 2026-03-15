use capture::{
    EncoderBackendAvailability, EncoderBackendDescriptor, EncoderBackendFactory,
    EncoderBackendFamily, EncoderBackendRuntimeReport,
};

pub struct MediaFoundationWindowsEncoderBackend;

static MEDIA_FOUNDATION_WINDOWS_ENCODER_BACKEND: MediaFoundationWindowsEncoderBackend =
    MediaFoundationWindowsEncoderBackend;

pub fn backend() -> &'static dyn EncoderBackendFactory {
    &MEDIA_FOUNDATION_WINDOWS_ENCODER_BACKEND
}

impl EncoderBackendFactory for MediaFoundationWindowsEncoderBackend {
    fn descriptor(&self) -> EncoderBackendDescriptor {
        EncoderBackendDescriptor {
            id: "windows-media-foundation",
            label: "Windows Media Foundation",
            family: EncoderBackendFamily::Native,
        }
    }

    fn availability(&self) -> EncoderBackendAvailability {
        let reason = match runtime_summary() {
            Some(summary) => format!(
                "{summary} The recorder still writes output through ffmpeg instead of a Media Foundation-native encoder pipeline."
            ),
            None => "A Media Foundation-native output path is planned for Phase 3, but the recorder still writes output through ffmpeg today.".to_string(),
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
    Some("H.264 hardware encoder candidate".to_string())
}

pub fn runtime_summary() -> Option<String> {
    Some(
        "Windows encoder migration is targeting Media Foundation with hardware-backed H.264 output."
            .to_string(),
    )
}
