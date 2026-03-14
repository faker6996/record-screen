use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn home_directory() -> Result<PathBuf, String> {
    env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME is not set".to_string())
}

#[cfg(target_os = "linux")]
fn autostart_directory() -> Result<PathBuf, String> {
    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home).join("autostart"));
    }

    Ok(home_directory()?.join(".config/autostart"))
}

#[cfg(target_os = "linux")]
fn autostart_entry_path() -> Result<PathBuf, String> {
    Ok(autostart_directory()?.join("record-screen.desktop"))
}

#[cfg(target_os = "linux")]
fn desktop_entry(executable_path: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=Record Screen\nComment=Start Record Screen in the background on login\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        executable_path.display()
    )
}

#[cfg(target_os = "linux")]
pub fn sync_launch_on_login(enabled: bool) -> Result<(), String> {
    let entry_path = autostart_entry_path()?;

    if enabled {
        let executable_path = env::current_exe()
            .map_err(|error| format!("failed to locate current executable: {error}"))?;
        if let Some(parent) = entry_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create autostart directory: {error}"))?;
        }
        fs::write(&entry_path, desktop_entry(&executable_path))
            .map_err(|error| format!("failed to write autostart entry: {error}"))?;
        return Ok(());
    }

    if entry_path.exists() {
        fs::remove_file(&entry_path)
            .map_err(|error| format!("failed to remove autostart entry: {error}"))?;
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn sync_launch_on_login(_enabled: bool) -> Result<(), String> {
    Ok(())
}
