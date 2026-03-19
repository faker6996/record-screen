use std::env;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::{fs, path::Path};

#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn home_directory() -> Result<PathBuf, String> {
    env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME is not set".to_string())
}

#[cfg(target_os = "linux")]
fn launch_on_login_path() -> Result<PathBuf, String> {
    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home)
            .join("autostart")
            .join("record-screen.desktop"));
    }

    Ok(home_directory()?.join(".config/autostart/record-screen.desktop"))
}

#[cfg(target_os = "macos")]
fn launch_on_login_path() -> Result<PathBuf, String> {
    Ok(home_directory()?.join("Library/LaunchAgents/com.recordscreen.desktop.plist"))
}

#[cfg(target_os = "linux")]
fn launch_on_login_payload(executable_path: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nVersion=1.0\nName=Record Screen\nComment=Start Record Screen in the background on login\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        executable_path.display()
    )
}

#[cfg(target_os = "macos")]
fn launch_on_login_payload(executable_path: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.recordscreen.desktop</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
"#,
        xml_escape(&executable_path.display().to_string())
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn sync_file_backed_launch_on_login(enabled: bool) -> Result<(), String> {
    let entry_path = launch_on_login_path()?;

    if enabled {
        let executable_path = env::current_exe()
            .map_err(|error| format!("failed to locate current executable: {error}"))?;
        if let Some(parent) = entry_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create launch-on-login directory: {error}"))?;
        }
        fs::write(&entry_path, launch_on_login_payload(&executable_path))
            .map_err(|error| format!("failed to write launch-on-login entry: {error}"))?;
        return Ok(());
    }

    if entry_path.exists() {
        fs::remove_file(&entry_path)
            .map_err(|error| format!("failed to remove launch-on-login entry: {error}"))?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn sync_windows_launch_on_login(enabled: bool) -> Result<(), String> {
    let executable_path = env::current_exe()
        .map_err(|error| format!("failed to locate current executable: {error}"))?;
    let value = format!("\"{}\"", executable_path.display());

    let mut command = Command::new("reg");
    command.args([
        if enabled { "add" } else { "delete" },
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        "/v",
        "RecordScreenDesktop",
    ]);

    if enabled {
        command.args(["/t", "REG_SZ", "/d", &value, "/f"]);
    } else {
        command.arg("/f");
    }

    let output = command
        .output()
        .map_err(|error| format!("failed to update Windows Run registry key: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !enabled && stderr.contains("unable to find") {
        return Ok(());
    }

    let message = if !stderr.is_empty() { stderr } else { stdout };
    Err(if message.trim().is_empty() {
        "failed to update launch-on-login entry".to_string()
    } else {
        message
    })
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "linux")]
pub fn sync_launch_on_login(enabled: bool) -> Result<(), String> {
    sync_file_backed_launch_on_login(enabled)
}

#[cfg(target_os = "macos")]
pub fn sync_launch_on_login(enabled: bool) -> Result<(), String> {
    sync_file_backed_launch_on_login(enabled)
}

#[cfg(target_os = "windows")]
pub fn sync_launch_on_login(enabled: bool) -> Result<(), String> {
    sync_windows_launch_on_login(enabled)
}
