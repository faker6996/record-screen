use capture::AudioInputOption;

pub fn initial_audio_inputs() -> Vec<AudioInputOption> {
    crate::device_discovery::initial_snapshot().audio_inputs
}

pub fn available_audio_inputs() -> Vec<AudioInputOption> {
    crate::device_discovery::current_snapshot().audio_inputs
}

pub fn refreshed_audio_inputs() -> Vec<AudioInputOption> {
    crate::device_discovery::refreshed_snapshot().audio_inputs
}

pub fn normalize_audio_input_selection(
    selected_audio_input_id: &str,
    audio_inputs: &[AudioInputOption],
) -> Option<String> {
    capture::resolve_microphone_input_id(selected_audio_input_id, audio_inputs)
}
