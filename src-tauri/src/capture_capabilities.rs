#[derive(Debug, Clone)]
pub struct CaptureCapabilities {
    pub supports_custom_region: bool,
    pub custom_region_note: String,
    pub supports_system_audio: bool,
    pub system_audio_note: String,
}

pub fn current_capture_capabilities() -> CaptureCapabilities {
    #[cfg(target_os = "macos")]
    {
        return CaptureCapabilities {
            supports_custom_region: false,
            custom_region_note:
                "Custom region capture is not wired into the macOS AVFoundation backend yet."
                    .to_string(),
            supports_system_audio: false,
            system_audio_note: "System-audio mixing is not wired into the macOS backend yet."
                .to_string(),
        };
    }

    #[cfg(target_os = "windows")]
    {
        let (supports_custom_region, custom_region_note) =
            capture_windows::custom_region_support_summary();
        let (supports_system_audio, system_audio_note) =
            capture_windows::system_audio_support_summary();

        return CaptureCapabilities {
            supports_custom_region,
            custom_region_note,
            supports_system_audio,
            system_audio_note,
        };
    }

    #[cfg(target_os = "linux")]
    {
        let (supports_custom_region, custom_region_note) =
            capture_linux::custom_region_support_summary();
        let (supports_system_audio, system_audio_note) =
            capture_linux::system_audio_support_summary();

        return CaptureCapabilities {
            supports_custom_region,
            custom_region_note,
            supports_system_audio,
            system_audio_note,
        };
    }

    #[allow(unreachable_code)]
    CaptureCapabilities {
        supports_custom_region: false,
        custom_region_note: "This platform does not have a recording backend.".to_string(),
        supports_system_audio: false,
        system_audio_note: "This platform does not have a recording backend.".to_string(),
    }
}
