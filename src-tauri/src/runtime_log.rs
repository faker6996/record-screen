use std::{
    fs::{self, OpenOptions},
    io::Write,
    panic,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

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
