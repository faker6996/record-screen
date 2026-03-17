pub(crate) fn quality_settings(preset: &str) -> (u32, u32, u32) {
    match preset {
        "720p / 30 fps" => (1280, 720, 30),
        "1080p / 30 fps" => (1920, 1080, 30),
        "1080p / 60 fps" => (1920, 1080, 60),
        "1440p / 60 fps" => (2560, 1440, 60),
        "4K / 60 fps" => (3840, 2160, 60),
        _ => (1920, 1080, 60),
    }
}

pub(crate) fn cpu_preset_for_quality(preset: &str) -> &'static str {
    match preset {
        "4K / 60 fps" | "1440p / 60 fps" => "ultrafast",
        "1080p / 60 fps" => "superfast",
        _ => "veryfast",
    }
}

pub(crate) fn gst_bitrate_for_quality(preset: &str) -> u32 {
    match preset {
        "720p / 30 fps" => 4_000,
        "1080p / 30 fps" => 8_000,
        "1080p / 60 fps" => 12_000,
        "1440p / 60 fps" => 18_000,
        "4K / 60 fps" => 30_000,
        _ => 8_000,
    }
}
