use capture::CaptureTargetOption;

pub fn initial_capture_targets() -> Vec<CaptureTargetOption> {
    platform_capture_targets()
}

pub fn available_capture_targets() -> Vec<CaptureTargetOption> {
    platform_capture_targets()
}

#[cfg(target_os = "linux")]
fn platform_capture_targets() -> Vec<CaptureTargetOption> {
    capture_linux::list_capture_targets()
}

#[cfg(target_os = "macos")]
fn platform_capture_targets() -> Vec<CaptureTargetOption> {
    capture_macos::list_capture_targets()
}

#[cfg(target_os = "windows")]
fn platform_capture_targets() -> Vec<CaptureTargetOption> {
    capture_windows::list_capture_targets()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_capture_targets() -> Vec<CaptureTargetOption> {
    vec![capture::full_desktop_target()]
}
