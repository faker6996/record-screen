use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use app_core::SessionSummary;
use storage::expand_home_path;
use tauri::State;
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::AppState;

fn ensure_existing_target(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    Err(format!("Recording path does not exist: {}", path.display()))
}

fn parent_directory(path: &Path) -> Result<PathBuf, String> {
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("Unable to resolve parent directory for {}", path.display()))
}

#[cfg(not(target_os = "linux"))]
fn run_command(mut command: Command, action_label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("Failed to {action_label}: {error}"))?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "Failed to {action_label}: process exited with {status}"
    ))
}

#[cfg(target_os = "linux")]
fn run_linux_open(path: &Path, action_label: &str) -> Result<(), String> {
    let mut attempts = Vec::new();

    for (program, args) in [
        ("xdg-open", vec![path.display().to_string()]),
        ("gio", vec!["open".to_string(), path.display().to_string()]),
    ] {
        let mut command = Command::new(program);
        command.args(&args);

        match command.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => {
                attempts.push(format!("{program} exited with {status}"));
            }
            Err(error) => {
                attempts.push(format!("{program}: {error}"));
            }
        }
    }

    Err(format!(
        "Failed to {action_label}. Tried Linux open handlers: {}",
        attempts.join("; ")
    ))
}

fn open_path(path: &Path) -> Result<(), String> {
    ensure_existing_target(path)?;

    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(path);
        return run_command(command, "open recording");
    }

    #[cfg(target_os = "linux")]
    {
        return run_linux_open(path, "open recording");
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", &path.display().to_string()]);
        return run_command(command, "open recording");
    }

    #[allow(unreachable_code)]
    Err("Opening recordings is not supported on this platform".to_string())
}

fn reveal_path(path: &Path) -> Result<(), String> {
    let fallback_directory = parent_directory(path)?;

    #[cfg(target_os = "macos")]
    {
        if path.exists() {
            let mut reveal_command = Command::new("open");
            reveal_command.args(["-R", &path.display().to_string()]);
            return run_command(reveal_command, "reveal recording");
        }

        let mut open_parent = Command::new("open");
        open_parent.arg(&fallback_directory);
        return run_command(open_parent, "open recording folder");
    }

    #[cfg(target_os = "linux")]
    {
        return run_linux_open(&fallback_directory, "open recording folder");
    }

    #[cfg(target_os = "windows")]
    {
        if path.exists() {
            let mut reveal_command = Command::new("explorer");
            reveal_command.arg(format!("/select,{}", path.display()));
            return run_command(reveal_command, "reveal recording");
        }

        let mut open_parent = Command::new("explorer");
        open_parent.arg(&fallback_directory);
        return run_command(open_parent, "open recording folder");
    }

    #[allow(unreachable_code)]
    Err("Revealing recordings is not supported on this platform".to_string())
}

pub fn scan_recent_recordings(output_directory: &str) -> Vec<SessionSummary> {
    let directory = expand_home_path(output_directory);
    let Ok(entries) = fs::read_dir(&directory) else {
        return Vec::new();
    };

    let mut sessions = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let extension = path.extension()?.to_str()?.to_ascii_lowercase();
            if !matches!(extension.as_str(), "mp4" | "mov" | "mkv" | "webm") {
                return None;
            }

            let metadata = entry.metadata().ok()?;
            let modified = metadata.modified().ok().unwrap_or(SystemTime::UNIX_EPOCH);
            let filename = path.file_name()?.to_string_lossy().to_string();

            Some((
                modified,
                SessionSummary {
                    id: format!("file-{}", filename),
                    title: filename,
                    started_at: format_modified_at(modified),
                    duration_label: String::new(),
                    location: path.display().to_string(),
                    size_label: format_size(metadata.len()),
                },
            ))
        })
        .collect::<Vec<_>>();

    sessions.sort_by(|left, right| right.0.cmp(&left.0));
    sessions.truncate(20);
    sessions.into_iter().map(|(_, session)| session).collect()
}

fn format_modified_at(modified_at: SystemTime) -> String {
    let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let format = format_description!("[month repr:short] [day], [year] · [hour]:[minute]");

    OffsetDateTime::from(modified_at)
        .to_offset(local_offset)
        .format(&format)
        .unwrap_or_else(|_| "Just now".to_string())
}

fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes as f64 >= GB {
        format!("{:.2} GB", bytes as f64 / GB)
    } else if bytes as f64 >= MB {
        format!("{:.1} MB", bytes as f64 / MB)
    } else {
        format!("{:.0} KB", bytes as f64 / KB)
    }
}

#[tauri::command]
pub fn open_recording(recording_path: String) -> Result<(), String> {
    let resolved_path = expand_home_path(&recording_path);
    open_path(&resolved_path)
}

#[tauri::command]
pub fn reveal_recording_in_folder(recording_path: String) -> Result<(), String> {
    let resolved_path = expand_home_path(&recording_path);
    reveal_path(&resolved_path)
}

#[tauri::command]
pub fn get_recent_recordings(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    let output_directory = {
        let core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.settings().output_directory
    };

    let sessions = scan_recent_recordings(&output_directory);

    {
        let mut core = state
            .core
            .lock()
            .map_err(|_| "failed to lock app state".to_string())?;
        core.sync_recent_sessions(sessions.clone());
    }
    Ok(sessions)
}
