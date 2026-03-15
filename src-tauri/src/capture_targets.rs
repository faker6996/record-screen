use capture::{
    CUSTOM_REGION_TARGET_ID, CaptureTargetOption, FULL_DESKTOP_TARGET_ID, custom_region_target,
};
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

pub fn normalize_custom_region_source_target_id(
    current_target_id: &str,
    capture_targets: &[CaptureTargetOption],
) -> Option<String> {
    if capture_targets
        .iter()
        .any(|target| target.id == current_target_id && target.id != CUSTOM_REGION_TARGET_ID)
    {
        return Some(current_target_id.to_string());
    }

    capture_targets
        .iter()
        .find(|target| target.id == FULL_DESKTOP_TARGET_ID)
        .or_else(|| {
            capture_targets
                .iter()
                .find(|target| target.id != CUSTOM_REGION_TARGET_ID)
        })
        .map(|target| target.id.clone())
}

#[cfg(test)]
mod tests {
    use super::normalize_custom_region_source_target_id;
    use capture::{
        CUSTOM_REGION_TARGET_ID, CaptureTargetOption, FULL_DESKTOP_TARGET_ID, full_desktop_target,
    };

    fn target(id: &str, label: &str) -> CaptureTargetOption {
        CaptureTargetOption {
            id: id.to_string(),
            label: label.to_string(),
            description: String::new(),
        }
    }

    #[test]
    fn keeps_existing_non_custom_target() {
        let capture_targets = vec![full_desktop_target(), target("monitor:1", "Display 1")];

        let normalized = normalize_custom_region_source_target_id("monitor:1", &capture_targets);

        assert_eq!(normalized.as_deref(), Some("monitor:1"));
    }

    #[test]
    fn falls_back_to_full_desktop_when_target_is_stale() {
        let capture_targets = vec![full_desktop_target(), target("monitor:1", "Display 1")];

        let normalized = normalize_custom_region_source_target_id("monitor:9", &capture_targets);

        assert_eq!(normalized.as_deref(), Some(FULL_DESKTOP_TARGET_ID));
    }

    #[test]
    fn skips_custom_region_when_picking_fallback_target() {
        let capture_targets = vec![
            target(CUSTOM_REGION_TARGET_ID, "Custom region"),
            target("monitor:2", "Display 2"),
        ];

        let normalized =
            normalize_custom_region_source_target_id(CUSTOM_REGION_TARGET_ID, &capture_targets);

        assert_eq!(normalized.as_deref(), Some("monitor:2"));
    }
}
