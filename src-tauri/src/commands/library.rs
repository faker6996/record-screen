use std::{
    path::{Path, PathBuf},
    process::Command,
};

use storage::expand_home_path;

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

fn run_command(mut command: Command, action_label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("Failed to {action_label}: {error}"))?;

    if status.success() {
        return Ok(());
    }

    Err(format!("Failed to {action_label}: process exited with {status}"))
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
        let mut command = Command::new("xdg-open");
        command.arg(path);
        return run_command(command, "open recording");
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
        let mut command = Command::new("xdg-open");
        command.arg(&fallback_directory);
        return run_command(command, "open recording folder");
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
