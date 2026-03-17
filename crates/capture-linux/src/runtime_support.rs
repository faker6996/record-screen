use std::{
    process::Child,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use capture::CaptureError;

const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STARTUP_POLL_ATTEMPTS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxCaptureProcessKind {
    GstreamerX11,
    GstreamerWayland,
}

pub(crate) fn verify_process_started(
    child: &mut Child,
    stderr_buffer: &Arc<Mutex<String>>,
    process_kind: LinuxCaptureProcessKind,
) -> Result<(), CaptureError> {
    for _ in 0..STARTUP_POLL_ATTEMPTS {
        thread::sleep(STARTUP_POLL_INTERVAL);
        if child
            .try_wait()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?
            .is_some()
        {
            return Err(CaptureError::SpawnFailed(describe_process_failure(
                process_kind,
                &read_stderr_buffer(stderr_buffer),
            )));
        }
    }

    Ok(())
}

pub(crate) fn read_stderr_buffer(buffer: &Arc<Mutex<String>>) -> String {
    buffer.lock().map(|log| log.clone()).unwrap_or_default()
}

pub(crate) fn describe_process_failure(
    process_kind: LinuxCaptureProcessKind,
    stderr_log: &str,
) -> String {
    match process_kind {
        LinuxCaptureProcessKind::GstreamerX11 => describe_gstreamer_x11_failure(stderr_log),
        LinuxCaptureProcessKind::GstreamerWayland => describe_gstreamer_failure(stderr_log),
    }
}

pub(crate) fn request_process_stop(
    _process_kind: LinuxCaptureProcessKind,
    pid: u32,
    _stdin: Option<&mut std::process::ChildStdin>,
) -> Result<(), CaptureError> {
    let result = unsafe { libc::kill(pid as i32, libc::SIGINT) };
    if result != 0 {
        return Err(CaptureError::StopFailed(
            "failed to send SIGINT to gst-launch".to_string(),
        ));
    }

    Ok(())
}

fn describe_gstreamer_failure(stderr_log: &str) -> String {
    let stderr_lower = stderr_log.to_lowercase();
    let tail_lines: Vec<_> = stderr_log
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(6)
        .collect();
    let tail = if tail_lines.is_empty() {
        "the Wayland GStreamer capture process exited before capture could start.".to_string()
    } else {
        tail_lines
            .into_iter()
            .rev()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" | ")
    };

    if stderr_lower.contains("no element \"pipewiresrc\"") {
        return format!(
            "GStreamer PipeWire support is missing on this machine. Install the PipeWire GStreamer plugin first. Last log: {tail}"
        );
    }

    if stderr_lower.contains("could not open resource for reading")
        || stderr_lower.contains("failed to connect")
        || stderr_lower.contains("pipewire")
    {
        return format!(
            "The ScreenCast portal returned a PipeWire stream, but GStreamer could not attach to it. Check that PipeWire and xdg-desktop-portal are running in this Wayland session. Last log: {tail}"
        );
    }

    if stderr_lower.contains("pulsesrc") || stderr_lower.contains("pulse") {
        return format!(
            "GStreamer could not open the selected microphone source. Disable microphone capture and try again. Last log: {tail}"
        );
    }

    tail
}

fn describe_gstreamer_x11_failure(stderr_log: &str) -> String {
    let stderr_lower = stderr_log.to_lowercase();
    let tail_lines: Vec<_> = stderr_log
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(6)
        .collect();
    let tail = if tail_lines.is_empty() {
        "the X11 GStreamer capture process exited before capture could start.".to_string()
    } else {
        tail_lines
            .into_iter()
            .rev()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" | ")
    };

    if stderr_lower.contains("no element \"ximagesrc\"") {
        return format!(
            "GStreamer X11 support is missing on this machine. Install the ximagesrc plugin first. Last log: {tail}"
        );
    }

    if stderr_lower.contains("cannot open display")
        || stderr_lower.contains("display-name")
        || stderr_lower.contains("ximagesrc")
    {
        return format!(
            "The GStreamer X11 lane could not access the X11 display. Make sure this app is started inside the desktop session and DISPLAY is exported. Last log: {tail}"
        );
    }

    if stderr_lower.contains("pulsesrc") || stderr_lower.contains("pulse") {
        return format!(
            "GStreamer could not open the selected microphone or system-audio source. Disable audio capture and try again. Last log: {tail}"
        );
    }

    tail
}
