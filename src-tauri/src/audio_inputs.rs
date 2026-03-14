use capture::AudioInputOption;

pub fn initial_audio_inputs() -> Vec<AudioInputOption> {
    platform_audio_inputs()
}

pub fn available_audio_inputs() -> Vec<AudioInputOption> {
    platform_audio_inputs()
}

pub fn normalize_audio_input_selection(
    selected_audio_input_id: &str,
    audio_inputs: &[AudioInputOption],
) -> Option<String> {
    capture::resolve_audio_input_id(selected_audio_input_id, audio_inputs)
}

#[cfg(target_os = "macos")]
fn platform_audio_inputs() -> Vec<AudioInputOption> {
    capture_macos::list_audio_inputs()
}

#[cfg(target_os = "linux")]
fn platform_audio_inputs() -> Vec<AudioInputOption> {
    capture_linux::list_audio_inputs()
}

#[cfg(target_os = "windows")]
fn platform_audio_inputs() -> Vec<AudioInputOption> {
    capture_windows::list_audio_inputs()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_audio_inputs() -> Vec<AudioInputOption> {
    vec![capture::default_audio_input()]
}
