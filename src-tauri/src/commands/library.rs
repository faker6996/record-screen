use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

#[cfg(target_os = "macos")]
use std::env;

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

fn filename_for_save_dialog(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .ok_or_else(|| format!("Unable to resolve filename for {}", path.display()))
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

fn pick_export_destination(source_path: &Path) -> Result<Option<PathBuf>, String> {
    let starting_directory = parent_directory(source_path)?;
    let filename = filename_for_save_dialog(source_path)?;

    #[cfg(target_os = "linux")]
    {
        let start = starting_directory.join(&filename).display().to_string();

        if command_exists("zenity") {
            let output = Command::new("zenity")
                .args([
                    "--file-selection",
                    "--save",
                    "--confirm-overwrite",
                    "--title=Save recording as",
                    "--filename",
                    &start,
                ])
                .output()
                .map_err(|error| format!("failed to open export dialog: {error}"))?;

            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if value.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(PathBuf::from(value)));
            }

            return Ok(None);
        }

        if command_exists("kdialog") {
            let output = Command::new("kdialog")
                .args(["--getsavefilename", &start])
                .output()
                .map_err(|error| format!("failed to open export dialog: {error}"))?;

            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if value.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(PathBuf::from(value)));
            }

            return Ok(None);
        }

        return Err("no supported save dialog found. Install zenity or kdialog.".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "POSIX path of (choose file name with prompt \"Save recording as\" default location POSIX file \"{}\" default name \"{}\")",
            escape_applescript_path(&starting_directory),
            escape_applescript_text(&filename),
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| format!("failed to open export dialog: {error}"))?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(value)));
        }

        return Ok(None);
    }

    #[cfg(target_os = "windows")]
    {
        let extension = source_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("mp4");

        let script = format!(
            "$dialog = New-Object System.Windows.Forms.SaveFileDialog; \
             $dialog.Title = 'Save recording as'; \
             $dialog.InitialDirectory = '{}'; \
             $dialog.FileName = '{}'; \
             $dialog.DefaultExt = '{}'; \
             $dialog.Filter = 'Video files|*.mp4;*.mov;*.mkv;*.webm|All files|*.*'; \
             if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{ \
               Write-Output $dialog.FileName \
             }}",
            starting_directory.display().to_string().replace('\'', "''"),
            filename.replace('\'', "''"),
            extension.replace('\'', "''"),
        );

        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; {script}"
                ),
            ])
            .output()
            .map_err(|error| format!("failed to open export dialog: {error}"))?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(value)));
        }

        return Ok(None);
    }

    #[allow(unreachable_code)]
    Err("exporting recordings is not supported on this platform".to_string())
}

fn export_copy(source_path: &Path) -> Result<Option<String>, String> {
    ensure_existing_target(source_path)?;

    let Some(destination_path) = pick_export_destination(source_path)? else {
        return Ok(None);
    };

    if destination_path == source_path {
        return Ok(Some(destination_path.display().to_string()));
    }

    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to prepare export destination: {error}"))?;
    }

    fs::copy(source_path, &destination_path)
        .map_err(|error| format!("Failed to export recording copy: {error}"))?;

    Ok(Some(destination_path.display().to_string()))
}

fn move_to_trash(path: &Path) -> Result<(), String> {
    ensure_existing_target(path)?;

    #[cfg(target_os = "macos")]
    {
        return move_to_macos_trash(path);
    }

    #[cfg(target_os = "linux")]
    {
        let mut attempts = Vec::new();

        for (program, args) in [
            ("gio", vec!["trash".to_string(), path.display().to_string()]),
            ("trash-put", vec![path.display().to_string()]),
        ] {
            let mut command = Command::new(program);
            command.args(&args);

            match command.status() {
                Ok(status) if status.success() => return Ok(()),
                Ok(status) => attempts.push(format!("{program} exited with {status}")),
                Err(error) => attempts.push(format!("{program}: {error}")),
            }
        }

        return Err(format!(
            "Failed to move recording to Trash. Tried Linux trash handlers: {}",
            attempts.join("; ")
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "Add-Type -AssemblyName Microsoft.VisualBasic; \
             [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile('{}', \
             [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs, \
             [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin)",
            path.display().to_string().replace('\'', "''"),
        );
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-Command", &script]);
        return run_command(command, "move recording to Recycle Bin");
    }

    #[allow(unreachable_code)]
    Err("Moving recordings to Trash is not supported on this platform".to_string())
}

#[cfg(target_os = "macos")]
fn move_to_macos_trash(path: &Path) -> Result<(), String> {
    let home = env::var("HOME")
        .map_err(|error| format!("Failed to resolve home directory for Trash: {error}"))?;
    let trash_directory = Path::new(&home).join(".Trash");
    fs::create_dir_all(&trash_directory)
        .map_err(|error| format!("Failed to prepare Trash directory: {error}"))?;

    let file_name = path
        .file_name()
        .ok_or_else(|| format!("Unable to resolve filename for {}", path.display()))?;

    let destination_path = unique_trash_destination(&trash_directory.join(file_name));

    match fs::rename(path, &destination_path) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            fs::copy(path, &destination_path).map_err(|copy_error| {
                format!(
                    "Failed to move recording to Trash: rename failed with {rename_error}; copy failed with {copy_error}"
                )
            })?;
            fs::remove_file(path).map_err(|error| {
                format!("Moved copy to Trash, but failed to remove original file: {error}")
            })
        }
    }
}

#[cfg(target_os = "macos")]
fn unique_trash_destination(base_path: &Path) -> PathBuf {
    if !base_path.exists() {
        return base_path.to_path_buf();
    }

    let stem = base_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("recording");
    let extension = base_path.extension().and_then(|value| value.to_str());
    let parent = base_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    for index in 1.. {
        let candidate_name = match extension {
            Some(extension) => format!("{stem} {index}.{extension}"),
            None => format!("{stem} {index}"),
        };
        let candidate_path = parent.join(candidate_name);
        if !candidate_path.exists() {
            return candidate_path;
        }
    }

    unreachable!("trash destination search should always terminate")
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
pub fn save_recording_copy(recording_path: String) -> Result<Option<String>, String> {
    let resolved_path = expand_home_path(&recording_path);
    export_copy(&resolved_path)
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

fn refresh_recent_recordings(state: &State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
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

#[tauri::command]
pub fn trash_recordings(
    recording_paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionSummary>, String> {
    if recording_paths.is_empty() {
        return refresh_recent_recordings(&state);
    }

    for recording_path in recording_paths {
        let resolved_path = expand_home_path(&recording_path);
        move_to_trash(&resolved_path)?;
    }

    refresh_recent_recordings(&state)
}

#[cfg(target_os = "linux")]
fn command_exists(command_name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path).any(|directory| directory.join(command_name).is_file())
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn escape_applescript_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
}

#[cfg(target_os = "macos")]
fn escape_applescript_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use std::ffi::OsString;
    #[cfg(target_os = "linux")]
    use std::sync::{Mutex, OnceLock};
    use std::{
        env,
        time::{Duration, UNIX_EPOCH},
    };

    #[cfg(target_os = "linux")]
    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        let directory = env::temp_dir().join(format!("record-screen-{name}-{suffix}"));
        fs::create_dir_all(&directory).expect("create test directory");
        directory
    }

    fn write_file(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, contents).expect("write test file");
    }

    #[cfg(target_os = "linux")]
    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    #[cfg(target_os = "linux")]
    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let original = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                unsafe {
                    env::set_var(self.key, value);
                }
            } else {
                unsafe {
                    env::remove_var(self.key);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn prepend_path(directory: &Path) -> OsString {
        let mut paths = vec![directory.to_path_buf()];
        if let Some(existing) = env::var_os("PATH") {
            paths.extend(env::split_paths(&existing));
        }
        env::join_paths(paths).expect("join PATH entries")
    }

    #[cfg(target_os = "linux")]
    fn write_executable_script(path: &Path, contents: &str) {
        use std::os::unix::fs::PermissionsExt;

        write_file(path, contents.as_bytes());
        let mut permissions = fs::metadata(path).expect("script metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("set script permissions");
    }

    #[test]
    fn scan_recent_recordings_sorts_latest_first_and_filters_extensions() {
        let directory = unique_test_dir("scan-recent");
        let older = directory.join("older.mp4");
        let newer = directory.join("newer.webm");
        let ignored = directory.join("notes.txt");

        write_file(&older, b"older");
        std::thread::sleep(Duration::from_millis(20));
        write_file(&newer, b"newer");
        write_file(&ignored, b"ignored");

        let sessions = scan_recent_recordings(
            directory
                .to_str()
                .expect("test directory should be valid utf-8"),
        );

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].title, "newer.webm");
        assert_eq!(sessions[1].title, "older.mp4");
        assert!(
            sessions
                .iter()
                .all(|session| !session.location.ends_with(".txt"))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn export_copy_uses_linux_save_dialog_and_copies_file() {
        let _guard = test_lock().lock().expect("test lock");
        let test_root = unique_test_dir("save-as");
        let source = test_root.join("recording.mp4");
        let destination = test_root.join("exports").join("saved-copy.mp4");
        let bin_dir = test_root.join("bin");
        let zenity = bin_dir.join("zenity");

        write_file(&source, b"recording-payload");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        write_executable_script(
            &zenity,
            &format!(
                "#!/usr/bin/env bash\nprintf '{}\\n'\n",
                destination.display()
            ),
        );

        let _path_guard = EnvVarGuard::set("PATH", prepend_path(&bin_dir));
        let result = export_copy(&source).expect("export copy should succeed");

        assert_eq!(
            result,
            Some(destination.display().to_string()),
            "save-as should return the chosen destination"
        );
        assert_eq!(
            fs::read(&destination).expect("destination contents"),
            b"recording-payload"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn move_to_trash_uses_linux_handler() {
        let _guard = test_lock().lock().expect("test lock");
        let test_root = unique_test_dir("trash");
        let source = test_root.join("recording-trash.mp4");
        let trash_dir = test_root.join("trash-bin");
        let bin_dir = test_root.join("bin");
        let gio = bin_dir.join("gio");

        write_file(&source, b"trash-payload");
        fs::create_dir_all(&trash_dir).expect("create trash dir");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        write_executable_script(
            &gio,
            "#!/usr/bin/env bash\nset -euo pipefail\nif [ \"$1\" != \"trash\" ]; then exit 2; fi\nmkdir -p \"$TEST_TRASH_DIR\"\nmv \"$2\" \"$TEST_TRASH_DIR\"/\n",
        );

        let _path_guard = EnvVarGuard::set("PATH", prepend_path(&bin_dir));
        let _trash_guard = EnvVarGuard::set("TEST_TRASH_DIR", trash_dir.as_os_str());
        move_to_trash(&source).expect("trash should succeed");

        assert!(!source.exists(), "source file should be gone after trash");
        assert!(
            trash_dir.join("recording-trash.mp4").exists(),
            "fake trash handler should move the file into the configured trash directory"
        );
    }
}
