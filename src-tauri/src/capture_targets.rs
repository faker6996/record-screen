use capture::{CaptureTargetOption, custom_region_target};
use storage::AppSettings;

pub fn initial_capture_targets(settings: &AppSettings) -> Vec<CaptureTargetOption> {
    with_custom_region(
        crate::device_discovery::initial_snapshot().capture_targets,
        settings,
    )
}

pub fn available_capture_targets(settings: &AppSettings) -> Vec<CaptureTargetOption> {
    with_custom_region(
        crate::device_discovery::current_snapshot().capture_targets,
        settings,
    )
}

pub fn refreshed_capture_targets(settings: &AppSettings) -> Vec<CaptureTargetOption> {
    with_custom_region(
        crate::device_discovery::refreshed_snapshot().capture_targets,
        settings,
    )
}

fn with_custom_region(
    mut capture_targets: Vec<CaptureTargetOption>,
    settings: &AppSettings,
) -> Vec<CaptureTargetOption> {
    if crate::capture_capabilities::current_capture_capabilities().supports_custom_region {
        capture_targets.push(custom_region_target(
            settings.region_x,
            settings.region_y,
            settings.region_width,
            settings.region_height,
        ));
    }

    capture_targets
}
