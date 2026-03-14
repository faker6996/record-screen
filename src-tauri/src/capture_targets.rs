use capture::CaptureTargetOption;

pub fn initial_capture_targets() -> Vec<CaptureTargetOption> {
    crate::device_discovery::initial_snapshot().capture_targets
}

pub fn available_capture_targets() -> Vec<CaptureTargetOption> {
    crate::device_discovery::current_snapshot().capture_targets
}

pub fn refreshed_capture_targets() -> Vec<CaptureTargetOption> {
    crate::device_discovery::refreshed_snapshot().capture_targets
}
