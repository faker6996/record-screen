#[cfg(target_os = "windows")]
mod platform {
    use std::{
        fs,
        io::{Read, Write},
        process::{Child, ChildStdin, Command, Stdio},
        sync::{Arc, Mutex},
        thread,
        time::{Duration, SystemTime},
    };

    use capture::{
        ActiveRecording, AudioInputOption, CaptureController, CaptureError, CaptureTargetOption,
        DEFAULT_AUDIO_INPUT_ID, FULL_DESKTOP_TARGET_ID, RecordingArtifact, RecordingOptions,
        default_audio_input, full_desktop_target, resolve_audio_input_id,
    };
    use serde::Deserialize;

    const MONITOR_TARGET_PREFIX: &str = "monitor:";
    const WINDOW_TARGET_PREFIX: &str = "window:";

    pub struct FfmpegWindowsCapture {
        active_recording: ActiveRecording,
        child: Child,
        stdin: Option<ChildStdin>,
        stderr_buffer: Arc<Mutex<String>>,
        finished_artifact: Option<RecordingArtifact>,
        paused: bool,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct MonitorDescriptor {
        device_name: String,
        label: String,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct WindowDescriptor {
        id: i64,
        title: String,
        process_name: String,
        width: u32,
        height: u32,
        x: i32,
        y: i32,
    }

    #[derive(Debug, Clone)]
    struct ResolvedTarget {
        label: String,
        source: String,
        offset_x: Option<i32>,
        offset_y: Option<i32>,
        video_size: Option<(u32, u32)>,
    }

    impl FfmpegWindowsCapture {
        pub fn start(options: RecordingOptions) -> Result<Self, CaptureError> {
            let started_at = SystemTime::now();
            let stderr_buffer = Arc::new(Mutex::new(String::new()));
            let target = resolve_target(&options.capture_target_id)?;
            let (child, stdin) = spawn_ffmpeg(&options, &target, Arc::clone(&stderr_buffer))?;

            Ok(Self {
                active_recording: ActiveRecording {
                    backend_name: "Windows ffmpeg / gdigrab".to_string(),
                    output_path: options.output_path,
                    started_at,
                    target_label: target.label,
                },
                child,
                stdin,
                stderr_buffer,
                finished_artifact: None,
                paused: false,
            })
        }

        fn build_artifact(
            &self,
            finished_at: SystemTime,
        ) -> Result<RecordingArtifact, CaptureError> {
            let metadata = fs::metadata(&self.active_recording.output_path)
                .map_err(|error| CaptureError::OutputInspectionFailed(error.to_string()))?;

            let duration = finished_at
                .duration_since(self.active_recording.started_at)
                .unwrap_or_default();

            Ok(RecordingArtifact {
                output_path: self.active_recording.output_path.clone(),
                started_at: self.active_recording.started_at,
                finished_at,
                duration,
                bytes_written: metadata.len(),
            })
        }
    }

    impl CaptureController for FfmpegWindowsCapture {
        fn active_recording(&self) -> &ActiveRecording {
            &self.active_recording
        }

        fn pause(&mut self) -> Result<(), CaptureError> {
            if self.paused {
                return Ok(());
            }

            run_powershell(&format!("Suspend-Process -Id {}", self.child.id()))
                .map_err(CaptureError::SignalFailed)?;
            self.paused = true;
            Ok(())
        }

        fn resume(&mut self) -> Result<(), CaptureError> {
            if !self.paused {
                return Ok(());
            }

            run_powershell(&format!("Resume-Process -Id {}", self.child.id()))
                .map_err(CaptureError::SignalFailed)?;
            self.paused = false;
            Ok(())
        }

        fn stop(&mut self) -> Result<RecordingArtifact, CaptureError> {
            if let Some(artifact) = self.finished_artifact.clone() {
                return Ok(artifact);
            }

            if self.paused {
                self.resume()?;
            }

            if let Some(stdin) = self.stdin.as_mut() {
                stdin
                    .write_all(b"q\n")
                    .and_then(|_| stdin.flush())
                    .map_err(|error| CaptureError::StopFailed(error.to_string()))?;
            }

            let status = self
                .child
                .wait()
                .map_err(|error| CaptureError::StopFailed(error.to_string()))?;

            if !status.success() {
                return Err(CaptureError::StopFailed(format!(
                    "ffmpeg exited with status {status}: {}",
                    describe_ffmpeg_failure(&read_stderr_buffer(&self.stderr_buffer))
                )));
            }

            let finished_at = SystemTime::now();
            let artifact = self.build_artifact(finished_at)?;
            self.finished_artifact = Some(artifact.clone());
            Ok(artifact)
        }

        fn poll_finished(&mut self) -> Result<Option<RecordingArtifact>, CaptureError> {
            if let Some(artifact) = self.finished_artifact.clone() {
                return Ok(Some(artifact));
            }

            let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| CaptureError::StopFailed(error.to_string()))?
            else {
                return Ok(None);
            };

            if !status.success() {
                return Err(CaptureError::StopFailed(describe_ffmpeg_failure(
                    &read_stderr_buffer(&self.stderr_buffer),
                )));
            }

            let artifact = self.build_artifact(SystemTime::now())?;
            self.finished_artifact = Some(artifact.clone());
            Ok(Some(artifact))
        }
    }

    pub fn list_capture_targets() -> Vec<CaptureTargetOption> {
        let mut targets = vec![full_desktop_target()];

        if let Ok(monitors) = query_monitors() {
            targets.extend(monitors.into_iter().map(|monitor| CaptureTargetOption {
                id: format!("{MONITOR_TARGET_PREFIX}{}", monitor.device_name),
                label: monitor.label,
                description: format!(
                    "{} x {} at {}, {}",
                    monitor.width, monitor.height, monitor.x, monitor.y
                ),
            }));
        }

        if let Ok(windows) = query_windows() {
            targets.extend(windows.into_iter().map(|window| CaptureTargetOption {
                id: format!("{WINDOW_TARGET_PREFIX}{}", window.id),
                label: format!("Window · {}", window.title),
                description: format!(
                    "{} · {} x {}",
                    window.process_name, window.width, window.height
                ),
            }));
        }

        targets
    }

    pub fn list_audio_inputs() -> Vec<AudioInputOption> {
        let mut inputs = vec![default_audio_input()];
        inputs.extend(discover_audio_inputs().unwrap_or_default());
        inputs
    }

    fn resolve_target(target_id: &str) -> Result<ResolvedTarget, CaptureError> {
        let monitors = query_monitors().unwrap_or_default();

        if target_id == FULL_DESKTOP_TARGET_ID {
            return Ok(resolve_full_desktop_target(&monitors));
        }

        if let Some(device_name) = target_id.strip_prefix(MONITOR_TARGET_PREFIX) {
            let monitor = monitors
                .into_iter()
                .find(|item| item.device_name == device_name)
                .ok_or_else(|| {
                    CaptureError::BackendUnavailable(format!(
                        "the selected monitor `{device_name}` is no longer available"
                    ))
                })?;

            return Ok(ResolvedTarget {
                label: monitor.label,
                source: "desktop".to_string(),
                offset_x: Some(monitor.x),
                offset_y: Some(monitor.y),
                video_size: Some((monitor.width, monitor.height)),
            });
        }

        if let Some(window_id) = target_id.strip_prefix(WINDOW_TARGET_PREFIX) {
            let window = query_windows()?
                .into_iter()
                .find(|item| item.id.to_string() == window_id)
                .ok_or_else(|| {
                    CaptureError::BackendUnavailable(format!(
                        "the selected window `{window_id}` is no longer available"
                    ))
                })?;

            return Ok(ResolvedTarget {
                label: format!("Window · {}", window.title),
                source: "desktop".to_string(),
                offset_x: Some(window.x),
                offset_y: Some(window.y),
                video_size: Some((window.width, window.height)),
            });
        }

        Err(CaptureError::BackendUnavailable(format!(
            "unknown Windows capture target: {target_id}"
        )))
    }

    fn resolve_full_desktop_target(monitors: &[MonitorDescriptor]) -> ResolvedTarget {
        if monitors.is_empty() {
            return ResolvedTarget {
                label: "Full desktop".to_string(),
                source: "desktop".to_string(),
                offset_x: None,
                offset_y: None,
                video_size: None,
            };
        }

        let min_x = monitors.iter().map(|monitor| monitor.x).min().unwrap_or(0);
        let min_y = monitors.iter().map(|monitor| monitor.y).min().unwrap_or(0);
        let max_x = monitors
            .iter()
            .map(|monitor| monitor.x + monitor.width as i32)
            .max()
            .unwrap_or(0);
        let max_y = monitors
            .iter()
            .map(|monitor| monitor.y + monitor.height as i32)
            .max()
            .unwrap_or(0);

        ResolvedTarget {
            label: "Full desktop".to_string(),
            source: "desktop".to_string(),
            offset_x: Some(min_x),
            offset_y: Some(min_y),
            video_size: Some(((max_x - min_x) as u32, (max_y - min_y) as u32)),
        }
    }

    fn spawn_ffmpeg(
        options: &RecordingOptions,
        target: &ResolvedTarget,
        stderr_buffer: Arc<Mutex<String>>,
    ) -> Result<(Child, Option<ChildStdin>), CaptureError> {
        let (width, height, fps) = quality_settings(&options.quality_preset);
        let mut command = Command::new("ffmpeg");
        command
            .arg("-y")
            .arg("-f")
            .arg("gdigrab")
            .arg("-draw_mouse")
            .arg("1")
            .arg("-framerate")
            .arg(fps.to_string())
            .arg("-thread_queue_size")
            .arg("1024");

        if let Some(offset_x) = target.offset_x {
            command.arg("-offset_x").arg(offset_x.to_string());
        }

        if let Some(offset_y) = target.offset_y {
            command.arg("-offset_y").arg(offset_y.to_string());
        }

        if let Some((source_width, source_height)) = target.video_size {
            command
                .arg("-video_size")
                .arg(format!("{source_width}x{source_height}"));
        }

        command.arg("-i").arg(&target.source);

        if options.mic_enabled {
            let device_name = discover_audio_device(&options.audio_input_id)?;
            command
                .arg("-f")
                .arg("dshow")
                .arg("-thread_queue_size")
                .arg("1024")
                .arg("-i")
                .arg(format!("audio={device_name}"))
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("192k");
        } else {
            command.arg("-an");
        }

        command
            .arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg("veryfast")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg("-vf")
            .arg(scale_filter(width, height))
            .arg("-movflags")
            .arg("+faststart")
            .arg(options.output_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?;

        let stdin = child.stdin.take();
        if let Some(mut stderr) = child.stderr.take() {
            let stderr_buffer = Arc::clone(&stderr_buffer);
            thread::spawn(move || {
                let mut buffer = String::new();
                let _ = stderr.read_to_string(&mut buffer);
                if let Ok(mut log) = stderr_buffer.lock() {
                    *log = buffer;
                }
            });
        }

        thread::sleep(Duration::from_millis(900));
        if child
            .try_wait()
            .map_err(|error| CaptureError::SpawnFailed(error.to_string()))?
            .is_some()
        {
            return Err(CaptureError::SpawnFailed(describe_ffmpeg_failure(
                &read_stderr_buffer(&stderr_buffer),
            )));
        }

        Ok((child, stdin))
    }

    fn query_monitors() -> Result<Vec<MonitorDescriptor>, CaptureError> {
        parse_json_command(
            r#"Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { [PSCustomObject]@{ deviceName = $_.DeviceName; label = if ($_.Primary) { "Display (Primary) · $($_.DeviceName)" } else { "Display · $($_.DeviceName)" }; width = $_.Bounds.Width; height = $_.Bounds.Height; x = $_.Bounds.X; y = $_.Bounds.Y; primary = $_.Primary } } | ConvertTo-Json -Compress"#,
        )
    }

    fn query_windows() -> Result<Vec<WindowDescriptor>, CaptureError> {
        parse_json_command(
            r#"$signature = @"
using System;
using System.Runtime.InteropServices;
public static class NativeWin {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
}
"@;
Add-Type $signature;
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle } | ForEach-Object {
  $rect = New-Object NativeWin+RECT
  if ([NativeWin]::GetWindowRect($_.MainWindowHandle, [ref]$rect)) {
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -gt 240 -and $height -gt 160 -and $_.MainWindowTitle -notlike 'Record Screen*') {
      [PSCustomObject]@{
        id = $_.MainWindowHandle.ToInt64()
        title = $_.MainWindowTitle
        processName = $_.ProcessName
        x = $rect.Left
        y = $rect.Top
        width = $width
        height = $height
      }
    }
  }
} | ConvertTo-Json -Compress"#,
        )
    }

    fn discover_audio_device(selected_audio_input_id: &str) -> Result<String, CaptureError> {
        let audio_inputs = discover_audio_inputs()?;
        if audio_inputs.is_empty() {
            return Err(CaptureError::BackendUnavailable(
                "ffmpeg did not expose any DirectShow audio input device".to_string(),
            ));
        }

        if selected_audio_input_id == DEFAULT_AUDIO_INPUT_ID {
            return resolve_audio_input_id(selected_audio_input_id, &audio_inputs).ok_or_else(
                || {
                    CaptureError::BackendUnavailable(
                        "ffmpeg did not expose any DirectShow audio input device".to_string(),
                    )
                },
            );
        }

        resolve_audio_input_id(selected_audio_input_id, &audio_inputs).ok_or_else(|| {
            CaptureError::BackendUnavailable(format!(
                "the selected microphone input `{selected_audio_input_id}` is no longer available"
            ))
        })
    }

    fn discover_audio_inputs() -> Result<Vec<AudioInputOption>, CaptureError> {
        let output = Command::new("ffmpeg")
            .args(["-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .output()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

        let listing = String::from_utf8_lossy(&output.stderr);
        let mut in_audio_section = false;
        let mut audio_inputs = Vec::new();

        for line in listing.lines() {
            if line.contains("DirectShow audio devices") {
                in_audio_section = true;
                continue;
            }

            if in_audio_section && line.contains("DirectShow video devices") {
                in_audio_section = false;
            }

            if !in_audio_section {
                continue;
            }

            let Some(device_name) = parse_ffmpeg_quoted_device(line) else {
                continue;
            };

            audio_inputs.push(AudioInputOption {
                id: device_name.clone(),
                label: device_name.clone(),
                description: format!("DirectShow input: {device_name}"),
            });
        }

        if audio_inputs.is_empty() {
            return Err(CaptureError::BackendUnavailable(
                "ffmpeg did not expose any DirectShow audio input device".to_string(),
            ));
        }

        Ok(audio_inputs)
    }

    fn parse_ffmpeg_quoted_device(line: &str) -> Option<String> {
        let start = line.find('"')? + 1;
        let end = line[start..].find('"')? + start;
        Some(line[start..end].to_string())
    }

    fn parse_json_command<T>(script: &str) -> Result<Vec<T>, CaptureError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))?;

        if !output.status.success() {
            return Err(CaptureError::BackendUnavailable(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }

        parse_json_array_or_single(&output.stdout)
            .map_err(|error| CaptureError::BackendUnavailable(error.to_string()))
    }

    fn parse_json_array_or_single<T>(bytes: &[u8]) -> Result<Vec<T>, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        if bytes.is_empty() {
            return Ok(Vec::new());
        }

        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        match value {
            serde_json::Value::Array(_) => serde_json::from_value(value),
            serde_json::Value::Null => Ok(Vec::new()),
            other => serde_json::from_value(other).map(|item| vec![item]),
        }
    }

    fn run_powershell(script: &str) -> Result<(), String> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|error| error.to_string())?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    fn quality_settings(preset: &str) -> (u32, u32, u32) {
        match preset {
            "720p / 30 fps" => (1280, 720, 30),
            "1440p / 60 fps" => (2560, 1440, 60),
            "4K / 60 fps" => (3840, 2160, 60),
            _ => (1920, 1080, 60),
        }
    }

    fn scale_filter(width: u32, height: u32) -> String {
        format!(
            "scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2"
        )
    }

    fn read_stderr_buffer(buffer: &Arc<Mutex<String>>) -> String {
        buffer.lock().map(|log| log.clone()).unwrap_or_default()
    }

    fn describe_ffmpeg_failure(stderr_log: &str) -> String {
        let stderr_lower = stderr_log.to_lowercase();

        if stderr_lower.contains("gdigrab") || stderr_lower.contains("desktop") {
            return "ffmpeg could not access the Windows desktop capture source. Make sure ffmpeg is installed and the selected target is still visible.".to_string();
        }

        if stderr_lower.contains("dshow") || stderr_lower.contains("audio") {
            return "ffmpeg could not open the Windows microphone source. Disable microphone capture and try again.".to_string();
        }

        if stderr_lower.contains("no such file or directory")
            || stderr_lower.contains("not recognized as an internal or external command")
        {
            return "ffmpeg is not available on this machine. Install ffmpeg first, then retry."
                .to_string();
        }

        stderr_log
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("ffmpeg exited before capture could start.")
            .trim()
            .to_string()
    }
}

#[cfg(target_os = "windows")]
pub use platform::{FfmpegWindowsCapture, list_audio_inputs, list_capture_targets};

#[cfg(not(target_os = "windows"))]
pub struct FfmpegWindowsCapture;

#[cfg(not(target_os = "windows"))]
pub fn list_capture_targets() -> Vec<capture::CaptureTargetOption> {
    vec![capture::full_desktop_target()]
}

#[cfg(not(target_os = "windows"))]
pub fn list_audio_inputs() -> Vec<capture::AudioInputOption> {
    vec![capture::default_audio_input()]
}

pub fn backend_name() -> &'static str {
    "Windows ffmpeg / gdigrab backend"
}
