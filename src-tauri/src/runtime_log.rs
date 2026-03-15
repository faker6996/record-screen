use std::{
    fs::{self, OpenOptions},
    io::Write,
    panic,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use app_core::RuntimeDiagnostics;

fn runtime_log_path() -> PathBuf {
    storage::app_config_directory().join("runtime.log")
}

fn write_line(level: &str, message: &str) -> Result<(), String> {
    let log_path = runtime_log_path();
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create runtime log directory: {error}"))?;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| format!("failed to open runtime log: {error}"))?;

    writeln!(file, "[{timestamp}] {level}: {message}")
        .map_err(|error| format!("failed to write runtime log: {error}"))?;
    Ok(())
}

pub fn init(version: &str) {
    let _ = write_line("INFO", &format!("Record Screen {version} launched"));

    panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown-location".to_string());
        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|payload| payload.to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_string());
        let _ = write_line("PANIC", &format!("{location} {payload}"));
    }));
}

pub fn log_runtime_error(message: &str) {
    let _ = write_line("ERROR", message);
}

pub fn log_runtime_info(message: &str) {
    let _ = write_line("INFO", message);
}

pub fn log_runtime_diagnostics(diagnostics: &RuntimeDiagnostics) {
    let message = format!(
        "runtime diagnostics | summary={} | capture={} | audio={} | encoder={} | capture_note={} | audio_note={} | encoder_note={} | preferred_target={} | preferred_input={} | preferred_system={} | preferred_encoder={}",
        diagnostics.summary,
        diagnostics.backend_path,
        diagnostics.audio_backend_path,
        diagnostics.encoder_backend_path,
        diagnostics.capture_selection_note,
        diagnostics.audio_selection_note,
        diagnostics.encoder_selection_note,
        "n/a",
        diagnostics
            .preferred_audio_input_label
            .as_deref()
            .unwrap_or("n/a"),
        diagnostics
            .preferred_system_audio_label
            .as_deref()
            .unwrap_or("n/a"),
        diagnostics
            .preferred_encoder_label
            .as_deref()
            .unwrap_or("n/a"),
    );
    let _ = write_line("INFO", &message);
}
