use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use capture::{
    CUSTOM_REGION_TARGET_ID, CaptureBackendAvailability, CaptureBackendDescriptor,
    CaptureBackendFactory, CaptureBackendFamily, CaptureBackendRuntimeReport, CaptureController,
    CaptureError, FULL_DESKTOP_TARGET_ID, RecordingOptions,
};
#[cfg(target_os = "macos")]
use screencapturekit::{
    shareable_content::{SCDisplay, SCShareableContent},
    stream::{
        SCStream,
        configuration::{PixelFormat, SCStreamConfiguration},
        content_filter::SCContentFilter,
        output_type::SCStreamOutputType,
    },
};

pub struct ScreenCaptureKitMacosBackend;

static SCREEN_CAPTURE_KIT_MACOS_BACKEND: ScreenCaptureKitMacosBackend =
    ScreenCaptureKitMacosBackend;

const SCREEN_CAPTURE_KIT_MINIMUM_MAJOR: u64 = 12;
const SCREEN_CAPTURE_KIT_MINIMUM_MINOR: u64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreenCaptureKitProbeReport {
    summary: String,
    preferred_target_label: Option<String>,
    display_count: usize,
    window_count: usize,
    application_count: usize,
    targets: Vec<ScreenCaptureKitNativeTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreenCaptureKitNativeTarget {
    target_id: String,
    display_index: usize,
    display_id: u32,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCaptureKitStartPlan {
    pub target_id: String,
    pub resolved_source_target_id: String,
    pub resolved_native_target_label: Option<String>,
    pub output_width: u32,
    pub output_height: u32,
    pub fps: u32,
    pub target_summary: String,
    pub stream_summary: String,
    pub region_summary: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCaptureKitExecutionPlan {
    pub target_label: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub shows_cursor: bool,
    pub captures_audio: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCaptureKitRuntimeFoundation {
    pub target_label: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub captures_audio: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCaptureKitPreparedRuntime {
    pub target_label: String,
    pub screen_handler_registered: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenCaptureKitSmokeLifecycle {
    pub target_label: String,
    pub observed_screen_frames: usize,
    pub observed_audio_samples: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ScreenCaptureKitBufferBridgeStats {
    screen_frames: usize,
    audio_samples: usize,
    first_screen_pts_millis: Option<i64>,
    first_audio_pts_millis: Option<i64>,
}

pub fn backend() -> &'static dyn CaptureBackendFactory {
    &SCREEN_CAPTURE_KIT_MACOS_BACKEND
}

pub fn start_plan(options: &RecordingOptions) -> ScreenCaptureKitStartPlan {
    enrich_start_plan(build_start_plan(options))
}

pub fn execution_plan(options: &RecordingOptions) -> Result<ScreenCaptureKitExecutionPlan, String> {
    build_execution_plan(options)
}

pub fn runtime_foundation_summary(options: &RecordingOptions) -> Option<String> {
    build_runtime_foundation(options)
        .ok()
        .map(|plan| plan.summary)
}

pub fn prepared_runtime_summary(options: &RecordingOptions) -> Option<String> {
    build_prepared_runtime(options)
        .ok()
        .map(|plan| plan.summary)
}

pub fn smoke_lifecycle_summary(options: &RecordingOptions) -> Option<String> {
    build_smoke_lifecycle(options).ok().map(|plan| plan.summary)
}

impl CaptureBackendFactory for ScreenCaptureKitMacosBackend {
    fn descriptor(&self) -> CaptureBackendDescriptor {
        CaptureBackendDescriptor {
            id: "macos-screencapturekit",
            label: "macOS ScreenCaptureKit",
            family: CaptureBackendFamily::Native,
        }
    }

    fn availability(&self) -> CaptureBackendAvailability {
        match macos_version() {
            Some((major, minor, _patch))
                if major < SCREEN_CAPTURE_KIT_MINIMUM_MAJOR
                    || (major == SCREEN_CAPTURE_KIT_MINIMUM_MAJOR
                        && minor < SCREEN_CAPTURE_KIT_MINIMUM_MINOR) =>
            {
                CaptureBackendAvailability::Unavailable {
                    reason: format!(
                        "ScreenCaptureKit requires macOS {}.{} or newer.",
                        SCREEN_CAPTURE_KIT_MINIMUM_MAJOR, SCREEN_CAPTURE_KIT_MINIMUM_MINOR
                    ),
                }
            }
            Some(_) => match screen_capture_kit_probe() {
                Ok(_report) => CaptureBackendAvailability::Available,
                Err(reason) => CaptureBackendAvailability::Unavailable {
                    reason: format!("ScreenCaptureKit shareable-content probe failed: {reason}"),
                },
            },
            None => CaptureBackendAvailability::Unavailable {
                reason:
                    "The app could not confirm the macOS version for ScreenCaptureKit readiness."
                        .to_string(),
            },
        }
    }

    fn runtime_report(&self) -> CaptureBackendRuntimeReport {
        match screen_capture_kit_probe() {
            Ok(report) => CaptureBackendRuntimeReport {
                summary: Some(format!(
                    "{} ScreenCaptureKit now has a hybrid recorder path for display capture; unsupported cases still fall back to the ffmpeg / AVFoundation runtime.",
                    report.summary
                )),
                preferred_target_label: report.preferred_target_label,
            },
            Err(probe_error) => CaptureBackendRuntimeReport {
                summary: Some(match macos_version() {
                    Some((major, minor, patch)) => format!(
                        "macOS ScreenCaptureKit candidate probed on {major}.{minor}.{patch}, but shareable-content probing failed: {probe_error}"
                    ),
                    None => "macOS ScreenCaptureKit candidate exists, but version and shareable-content probing both failed."
                        .to_string(),
                }),
                preferred_target_label: None,
            },
        }
    }

    fn start(&self, options: RecordingOptions) -> Result<Box<dyn CaptureController>, CaptureError> {
        super::start_native_capture_bridge(options)
    }
}

fn macos_version() -> Option<(u64, u64, u64)> {
    let output = Command::new("sw_vers")
        .args(["-productVersion"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_version(String::from_utf8_lossy(&output.stdout).trim())
}

#[cfg(target_os = "macos")]
fn screen_capture_kit_probe() -> Result<ScreenCaptureKitProbeReport, String> {
    use screencapturekit::shareable_content::SCShareableContent;

    let content = SCShareableContent::create()
        .with_on_screen_windows_only(true)
        .with_exclude_desktop_windows(true)
        .get()
        .map_err(|error| error.to_string())?;

    let displays = content.displays();
    let windows = content.windows();
    let applications = content.applications();

    Ok(ScreenCaptureKitProbeReport {
        summary: build_probe_summary(displays.len(), windows.len(), applications.len()),
        preferred_target_label: displays.first().map(|display| {
            format_display_label(display.display_id(), display.width(), display.height())
        }),
        display_count: displays.len(),
        window_count: windows.len(),
        application_count: applications.len(),
        targets: displays
            .iter()
            .enumerate()
            .map(|(index, display)| ScreenCaptureKitNativeTarget {
                target_id: format!("monitor:{}", display.display_id()),
                display_index: index,
                display_id: display.display_id(),
                label: format_display_label(
                    display.display_id(),
                    display.width(),
                    display.height(),
                ),
            })
            .collect(),
    })
}

#[cfg(not(target_os = "macos"))]
fn screen_capture_kit_probe() -> Result<ScreenCaptureKitProbeReport, String> {
    Err("ScreenCaptureKit probing only runs on macOS hosts.".to_string())
}

fn build_probe_summary(
    display_count: usize,
    window_count: usize,
    application_count: usize,
) -> String {
    format!(
        "ScreenCaptureKit shareable-content probe found {display_count} display(s), {window_count} window(s), and {application_count} application(s)."
    )
}

#[cfg(target_os = "macos")]
fn pts_millis(sample: &screencapturekit::cm::CMSampleBuffer) -> Option<i64> {
    let pts = sample.presentation_timestamp();
    if pts.timescale == 0 {
        return None;
    }

    Some((pts.value.saturating_mul(1000)) / i64::from(pts.timescale))
}

fn format_display_label(display_id: u32, width: u32, height: u32) -> String {
    format!("Display {display_id} ({width}x{height})")
}

fn build_start_plan(options: &RecordingOptions) -> ScreenCaptureKitStartPlan {
    let (output_width, output_height, fps) = stream_settings(&options.quality_preset);
    let target_id = options.capture_target_id.clone();
    let resolved_source_target_id = if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
        if options.region_source_capture_target_id.trim().is_empty() {
            FULL_DESKTOP_TARGET_ID.to_string()
        } else {
            options.region_source_capture_target_id.clone()
        }
    } else {
        options.capture_target_id.clone()
    };

    let target_summary = if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
        format!(
            "ScreenCaptureKit would target custom region on source `{}`.",
            resolved_source_target_id
        )
    } else if resolved_source_target_id == FULL_DESKTOP_TARGET_ID {
        "ScreenCaptureKit would target the full desktop capture source.".to_string()
    } else {
        format!(
            "ScreenCaptureKit would target capture source `{}`.",
            resolved_source_target_id
        )
    };

    let region_summary = (options.capture_target_id == CUSTOM_REGION_TARGET_ID).then(|| {
        format!(
            "Region x={}, y={}, width={}, height={}, origin=({}, {}), scale={}‰.",
            options.region_x,
            options.region_y,
            options.region_width,
            options.region_height,
            options.region_source_origin_x,
            options.region_source_origin_y,
            options.region_source_scale_factor_milli
        )
    });

    let stream_summary = format!(
        "Stream config would request {}x{} at {} fps.",
        output_width, output_height, fps
    );

    let summary = match &region_summary {
        Some(region_summary) => format!("{target_summary} {stream_summary} {region_summary}"),
        None => format!("{target_summary} {stream_summary}"),
    };

    ScreenCaptureKitStartPlan {
        target_id,
        resolved_source_target_id,
        resolved_native_target_label: None,
        output_width,
        output_height,
        fps,
        target_summary,
        stream_summary,
        region_summary,
        summary,
    }
}

fn enrich_start_plan(mut plan: ScreenCaptureKitStartPlan) -> ScreenCaptureKitStartPlan {
    let Ok(probe_report) = screen_capture_kit_probe() else {
        return plan;
    };

    let Some(native_target) = resolve_native_target(&probe_report, &plan.resolved_source_target_id)
    else {
        return plan;
    };

    plan.resolved_native_target_label = Some(native_target.label.clone());
    plan.summary = format!(
        "{} Native candidate resolves to {}.",
        plan.summary, native_target.label
    );
    plan
}

#[cfg(target_os = "macos")]
fn build_execution_plan(
    options: &RecordingOptions,
) -> Result<ScreenCaptureKitExecutionPlan, String> {
    let start_plan = build_start_plan(options);
    let content = SCShareableContent::create()
        .with_on_screen_windows_only(true)
        .with_exclude_desktop_windows(true)
        .get()
        .map_err(|error| error.to_string())?;

    let display = resolve_native_display(&content, &start_plan.resolved_source_target_id)
        .ok_or_else(|| {
            format!(
                "ScreenCaptureKit could not resolve native display for `{}`.",
                start_plan.resolved_source_target_id
            )
        })?;

    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();
    let config = SCStreamConfiguration::new()
        .with_width(start_plan.output_width)
        .with_height(start_plan.output_height)
        .with_fps(start_plan.fps)
        .with_pixel_format(PixelFormat::BGRA)
        .with_shows_cursor(true)
        .with_captures_audio(options.system_audio_enabled);

    let target_label =
        format_display_label(display.display_id(), display.width(), display.height());
    let mut summary = format!(
        "ScreenCaptureKit native execution plan built filter for {target_label} with {}x{} at {} fps, pixel-format=BGRA, cursor={}, system-audio={}.",
        config.width(),
        config.height(),
        config.fps(),
        config.shows_cursor(),
        config.captures_audio()
    );
    if options.capture_target_id == CUSTOM_REGION_TARGET_ID {
        summary.push_str(
            " Custom-region crop is still not wired into the native ScreenCaptureKit filter path.",
        );
    }

    drop(filter);

    Ok(ScreenCaptureKitExecutionPlan {
        target_label,
        width: config.width(),
        height: config.height(),
        fps: config.fps(),
        shows_cursor: config.shows_cursor(),
        captures_audio: config.captures_audio(),
        summary,
    })
}

#[cfg(not(target_os = "macos"))]
fn build_execution_plan(
    _options: &RecordingOptions,
) -> Result<ScreenCaptureKitExecutionPlan, String> {
    Err("ScreenCaptureKit execution planning only runs on macOS hosts.".to_string())
}

#[cfg(target_os = "macos")]
fn build_runtime_foundation(
    options: &RecordingOptions,
) -> Result<ScreenCaptureKitRuntimeFoundation, String> {
    let execution_plan = build_execution_plan(options)?;
    let start_plan = build_start_plan(options);
    let content = SCShareableContent::create()
        .with_on_screen_windows_only(true)
        .with_exclude_desktop_windows(true)
        .get()
        .map_err(|error| error.to_string())?;

    let display = resolve_native_display(&content, &start_plan.resolved_source_target_id)
        .ok_or_else(|| {
            format!(
                "ScreenCaptureKit could not resolve native display for `{}` while building stream foundation.",
                start_plan.resolved_source_target_id
            )
        })?;

    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();
    let config = SCStreamConfiguration::new()
        .with_width(start_plan.output_width)
        .with_height(start_plan.output_height)
        .with_fps(start_plan.fps)
        .with_pixel_format(PixelFormat::BGRA)
        .with_shows_cursor(true)
        .with_captures_audio(options.system_audio_enabled);
    let stream = SCStream::new(&filter, &config);

    let summary = format!(
        "ScreenCaptureKit runtime foundation constructed SCStream for {} at {}x{} {} fps, pixel-format=BGRA, cursor={}, system-audio={}.",
        execution_plan.target_label,
        execution_plan.width,
        execution_plan.height,
        execution_plan.fps,
        execution_plan.shows_cursor,
        execution_plan.captures_audio
    );

    drop(stream);

    Ok(ScreenCaptureKitRuntimeFoundation {
        target_label: execution_plan.target_label,
        width: execution_plan.width,
        height: execution_plan.height,
        fps: execution_plan.fps,
        captures_audio: execution_plan.captures_audio,
        summary,
    })
}

#[cfg(not(target_os = "macos"))]
fn build_runtime_foundation(
    _options: &RecordingOptions,
) -> Result<ScreenCaptureKitRuntimeFoundation, String> {
    Err("ScreenCaptureKit runtime foundation only runs on macOS hosts.".to_string())
}

#[cfg(target_os = "macos")]
fn build_prepared_runtime(
    options: &RecordingOptions,
) -> Result<ScreenCaptureKitPreparedRuntime, String> {
    let foundation = build_runtime_foundation(options)?;
    let start_plan = build_start_plan(options);
    let content = SCShareableContent::create()
        .with_on_screen_windows_only(true)
        .with_exclude_desktop_windows(true)
        .get()
        .map_err(|error| error.to_string())?;

    let display = resolve_native_display(&content, &start_plan.resolved_source_target_id)
        .ok_or_else(|| {
            format!(
                "ScreenCaptureKit could not resolve native display for `{}` while preparing stream handlers.",
                start_plan.resolved_source_target_id
            )
        })?;

    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();
    let config = SCStreamConfiguration::new()
        .with_width(start_plan.output_width)
        .with_height(start_plan.output_height)
        .with_fps(start_plan.fps)
        .with_pixel_format(PixelFormat::BGRA)
        .with_shows_cursor(true)
        .with_captures_audio(options.system_audio_enabled);

    let mut stream = SCStream::new(&filter, &config);
    let handler_registered = stream
        .add_output_handler(|_sample, _type| {}, SCStreamOutputType::Screen)
        .is_some();

    let summary = format!(
        "ScreenCaptureKit prepared runtime registered screen handler={} for {}.",
        handler_registered, foundation.target_label
    );

    drop(stream);

    Ok(ScreenCaptureKitPreparedRuntime {
        target_label: foundation.target_label,
        screen_handler_registered: handler_registered,
        summary,
    })
}

#[cfg(not(target_os = "macos"))]
fn build_prepared_runtime(
    _options: &RecordingOptions,
) -> Result<ScreenCaptureKitPreparedRuntime, String> {
    Err("ScreenCaptureKit prepared runtime only runs on macOS hosts.".to_string())
}

#[cfg(target_os = "macos")]
fn build_smoke_lifecycle(
    options: &RecordingOptions,
) -> Result<ScreenCaptureKitSmokeLifecycle, String> {
    let prepared = build_prepared_runtime(options)?;
    let start_plan = build_start_plan(options);
    let content = SCShareableContent::create()
        .with_on_screen_windows_only(true)
        .with_exclude_desktop_windows(true)
        .get()
        .map_err(|error| error.to_string())?;

    let display = resolve_native_display(&content, &start_plan.resolved_source_target_id)
        .ok_or_else(|| {
            format!(
                "ScreenCaptureKit could not resolve native display for `{}` while running smoke lifecycle.",
                start_plan.resolved_source_target_id
            )
        })?;

    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();
    let config = SCStreamConfiguration::new()
        .with_width(start_plan.output_width)
        .with_height(start_plan.output_height)
        .with_fps(start_plan.fps)
        .with_pixel_format(PixelFormat::BGRA)
        .with_shows_cursor(true)
        .with_captures_audio(options.system_audio_enabled);

    let observed_screen_frames = Arc::new(AtomicUsize::new(0));
    let observed_audio_samples = Arc::new(AtomicUsize::new(0));
    let bridge_stats = Arc::new(std::sync::Mutex::new(
        ScreenCaptureKitBufferBridgeStats::default(),
    ));
    let observed_screen_frames_for_handler = Arc::clone(&observed_screen_frames);
    let observed_audio_samples_for_handler = Arc::clone(&observed_audio_samples);
    let bridge_stats_for_screen = Arc::clone(&bridge_stats);
    let bridge_stats_for_audio = Arc::clone(&bridge_stats);

    let mut stream = SCStream::new(&filter, &config);
    let handler_registered = stream
        .add_output_handler(
            move |sample, _type| {
                observed_screen_frames_for_handler.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut stats) = bridge_stats_for_screen.lock() {
                    stats.screen_frames += 1;
                    if stats.first_screen_pts_millis.is_none() {
                        stats.first_screen_pts_millis = pts_millis(&sample);
                    }
                }
            },
            SCStreamOutputType::Screen,
        )
        .is_some();

    if !handler_registered {
        return Err(
            "ScreenCaptureKit smoke lifecycle could not register a screen output handler."
                .to_string(),
        );
    }

    if options.system_audio_enabled {
        let _ = stream.add_output_handler(
            move |sample, _type| {
                observed_audio_samples_for_handler.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut stats) = bridge_stats_for_audio.lock() {
                    stats.audio_samples += 1;
                    if stats.first_audio_pts_millis.is_none() {
                        stats.first_audio_pts_millis = pts_millis(&sample);
                    }
                }
            },
            SCStreamOutputType::Audio,
        );
    }

    stream.start_capture().map_err(|error| error.to_string())?;
    thread::sleep(Duration::from_millis(150));
    stream.stop_capture().map_err(|error| error.to_string())?;

    let frame_count = observed_screen_frames.load(Ordering::Relaxed);
    let audio_count = observed_audio_samples.load(Ordering::Relaxed);
    let bridge_summary = bridge_stats
        .lock()
        .ok()
        .map(|stats| {
            format!(
                "bridge(screen_frames={}, audio_samples={}, first_screen_pts_ms={:?}, first_audio_pts_ms={:?})",
                stats.screen_frames,
                stats.audio_samples,
                stats.first_screen_pts_millis,
                stats.first_audio_pts_millis
            )
        })
        .unwrap_or_else(|| "bridge(unavailable)".to_string());
    let summary = format!(
        "ScreenCaptureKit smoke lifecycle started and stopped capture for {} with {} observed screen frame(s) and {} observed audio sample(s). {}",
        prepared.target_label, frame_count, audio_count, bridge_summary
    );

    Ok(ScreenCaptureKitSmokeLifecycle {
        target_label: prepared.target_label,
        observed_screen_frames: frame_count,
        observed_audio_samples: audio_count,
        summary,
    })
}

#[cfg(not(target_os = "macos"))]
fn build_smoke_lifecycle(
    _options: &RecordingOptions,
) -> Result<ScreenCaptureKitSmokeLifecycle, String> {
    Err("ScreenCaptureKit smoke lifecycle only runs on macOS hosts.".to_string())
}

fn resolve_native_target<'a>(
    probe_report: &'a ScreenCaptureKitProbeReport,
    resolved_source_target_id: &str,
) -> Option<&'a ScreenCaptureKitNativeTarget> {
    if resolved_source_target_id == FULL_DESKTOP_TARGET_ID {
        return probe_report.targets.first();
    }

    if let Some(target) = probe_report
        .targets
        .iter()
        .find(|target| target.target_id == resolved_source_target_id)
    {
        return Some(target);
    }

    let parsed_monitor_index = resolved_source_target_id
        .strip_prefix("monitor:")
        .and_then(|value| value.parse::<usize>().ok())?;

    probe_report
        .targets
        .get(parsed_monitor_index.saturating_sub(1))
        .or_else(|| probe_report.targets.get(parsed_monitor_index))
        .or_else(|| {
            probe_report
                .targets
                .iter()
                .find(|target| target.display_index == parsed_monitor_index)
        })
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_native_display(
    content: &SCShareableContent,
    resolved_source_target_id: &str,
) -> Option<SCDisplay> {
    let displays = content.displays();

    if resolved_source_target_id == FULL_DESKTOP_TARGET_ID {
        return displays.into_iter().next();
    }

    if let Some(display_id) = resolved_source_target_id
        .strip_prefix("monitor:")
        .and_then(|value| value.parse::<u32>().ok())
    {
        if let Some(display) = displays
            .iter()
            .find(|display| display.display_id() == display_id)
        {
            return Some(display.clone());
        }
    }

    let parsed_monitor_index = resolved_source_target_id
        .strip_prefix("monitor:")
        .and_then(|value| value.parse::<usize>().ok())?;

    displays
        .get(parsed_monitor_index.saturating_sub(1))
        .cloned()
        .or_else(|| displays.get(parsed_monitor_index).cloned())
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn stream_settings(preset: &str) -> (u32, u32, u32) {
    match preset {
        "720p / 30 fps" => (1280, 720, 30),
        "1080p / 30 fps" => (1920, 1080, 30),
        "1440p / 60 fps" => (2560, 1440, 60),
        "4K / 60 fps" => (3840, 2160, 60),
        _ => (1920, 1080, 60),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ScreenCaptureKitExecutionPlan, ScreenCaptureKitNativeTarget,
        ScreenCaptureKitPreparedRuntime, ScreenCaptureKitProbeReport,
        ScreenCaptureKitRuntimeFoundation, ScreenCaptureKitSmokeLifecycle,
        ScreenCaptureKitStartPlan, build_probe_summary, build_start_plan, enrich_start_plan,
        format_display_label, parse_version, resolve_native_target, stream_settings,
    };
    use capture::{CUSTOM_REGION_TARGET_ID, FULL_DESKTOP_TARGET_ID, RecordingOptions};
    use std::path::PathBuf;

    #[test]
    fn parses_semver_like_macos_versions() {
        assert_eq!(parse_version("14.4.1"), Some((14, 4, 1)));
        assert_eq!(parse_version("13.6"), Some((13, 6, 0)));
    }

    #[test]
    fn rejects_invalid_versions() {
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("ventura"), None);
    }

    #[test]
    fn builds_probe_summary() {
        assert_eq!(
            build_probe_summary(2, 14, 6),
            "ScreenCaptureKit shareable-content probe found 2 display(s), 14 window(s), and 6 application(s)."
        );
    }

    #[test]
    fn formats_display_label() {
        assert_eq!(
            format_display_label(69733248, 3024, 1964),
            "Display 69733248 (3024x1964)"
        );
    }

    #[test]
    fn resolves_stream_settings_from_quality_preset() {
        assert_eq!(stream_settings("720p / 30 fps"), (1280, 720, 30));
        assert_eq!(stream_settings("4K / 60 fps"), (3840, 2160, 60));
    }

    #[test]
    fn resolves_full_desktop_to_first_native_target() {
        let report = ScreenCaptureKitProbeReport {
            summary: String::new(),
            preferred_target_label: None,
            display_count: 2,
            window_count: 0,
            application_count: 0,
            targets: vec![
                ScreenCaptureKitNativeTarget {
                    target_id: "monitor:69733248".to_string(),
                    display_index: 0,
                    display_id: 69733248,
                    label: "Display 69733248 (3024x1964)".to_string(),
                },
                ScreenCaptureKitNativeTarget {
                    target_id: "monitor:1002".to_string(),
                    display_index: 1,
                    display_id: 1002,
                    label: "Display 1002 (1920x1080)".to_string(),
                },
            ],
        };

        let target = resolve_native_target(&report, FULL_DESKTOP_TARGET_ID).unwrap();
        assert_eq!(target.display_id, 69733248);
    }

    #[test]
    fn resolves_monitor_targets_by_exact_or_index_match() {
        let report = ScreenCaptureKitProbeReport {
            summary: String::new(),
            preferred_target_label: None,
            display_count: 2,
            window_count: 0,
            application_count: 0,
            targets: vec![
                ScreenCaptureKitNativeTarget {
                    target_id: "monitor:69733248".to_string(),
                    display_index: 0,
                    display_id: 69733248,
                    label: "Display 69733248 (3024x1964)".to_string(),
                },
                ScreenCaptureKitNativeTarget {
                    target_id: "monitor:1002".to_string(),
                    display_index: 1,
                    display_id: 1002,
                    label: "Display 1002 (1920x1080)".to_string(),
                },
            ],
        };

        let exact_target = resolve_native_target(&report, "monitor:69733248").unwrap();
        assert_eq!(exact_target.display_id, 69733248);

        let index_target = resolve_native_target(&report, "monitor:1").unwrap();
        assert_eq!(index_target.display_id, 69733248);

        let second_index_target = resolve_native_target(&report, "monitor:2").unwrap();
        assert_eq!(second_index_target.display_id, 1002);
    }

    #[test]
    fn builds_custom_region_start_plan() {
        let plan = build_start_plan(&RecordingOptions {
            output_path: PathBuf::from("/tmp/out.mp4"),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: true,
            system_audio_enabled: false,
            capture_target_id: CUSTOM_REGION_TARGET_ID.to_string(),
            audio_input_id: "default".to_string(),
            region_x: 100,
            region_y: 120,
            region_width: 1280,
            region_height: 720,
            region_source_capture_target_id: "monitor:main".to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 24,
            region_source_scale_factor_milli: 2000,
        });

        assert_eq!(plan.target_id, CUSTOM_REGION_TARGET_ID);
        assert_eq!(plan.resolved_source_target_id, "monitor:main");
        assert!(plan.summary.contains("custom region"));
        assert!(plan.summary.contains("monitor:main"));
        assert!(plan.summary.contains("scale=2000"));
        assert_eq!(plan.output_width, 1920);
        assert_eq!(plan.output_height, 1080);
        assert_eq!(plan.fps, 30);
    }

    #[test]
    fn builds_full_desktop_start_plan() {
        let plan = build_start_plan(&RecordingOptions {
            output_path: PathBuf::from("/tmp/out.mp4"),
            quality_preset: "1080p / 30 fps".to_string(),
            mic_enabled: true,
            system_audio_enabled: false,
            capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            audio_input_id: "default".to_string(),
            region_x: 0,
            region_y: 0,
            region_width: 0,
            region_height: 0,
            region_source_capture_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            region_source_origin_x: 0,
            region_source_origin_y: 0,
            region_source_scale_factor_milli: 1000,
        });

        assert_eq!(plan.resolved_source_target_id, FULL_DESKTOP_TARGET_ID);
        assert!(plan.summary.contains("full desktop"));
        assert!(plan.region_summary.is_none());
    }

    #[test]
    fn enrich_start_plan_keeps_shape_stable() {
        let plan = ScreenCaptureKitStartPlan {
            target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            resolved_source_target_id: FULL_DESKTOP_TARGET_ID.to_string(),
            resolved_native_target_label: None,
            output_width: 1920,
            output_height: 1080,
            fps: 30,
            target_summary: "ScreenCaptureKit would target the full desktop capture source."
                .to_string(),
            stream_summary: "Stream config would request 1920x1080 at 30 fps.".to_string(),
            region_summary: None,
            summary: "ScreenCaptureKit would target the full desktop capture source. Stream config would request 1920x1080 at 30 fps.".to_string(),
        };

        let enriched = enrich_start_plan(plan);
        if let Some(label) = enriched.resolved_native_target_label {
            assert!(enriched.summary.contains(&label));
        }
    }

    #[test]
    fn execution_plan_shape_is_constructible() {
        let plan = ScreenCaptureKitExecutionPlan {
            target_label: "Display 1 (1920x1080)".to_string(),
            width: 1920,
            height: 1080,
            fps: 30,
            shows_cursor: true,
            captures_audio: false,
            summary: "native execution plan".to_string(),
        };

        assert_eq!(plan.width, 1920);
        assert_eq!(plan.fps, 30);
    }

    #[test]
    fn runtime_foundation_shape_is_constructible() {
        let plan = ScreenCaptureKitRuntimeFoundation {
            target_label: "Display 1 (1920x1080)".to_string(),
            width: 1920,
            height: 1080,
            fps: 30,
            captures_audio: false,
            summary: "runtime foundation".to_string(),
        };

        assert_eq!(plan.height, 1080);
        assert!(!plan.captures_audio);
    }

    #[test]
    fn prepared_runtime_shape_is_constructible() {
        let plan = ScreenCaptureKitPreparedRuntime {
            target_label: "Display 1 (1920x1080)".to_string(),
            screen_handler_registered: true,
            summary: "prepared runtime".to_string(),
        };

        assert!(plan.screen_handler_registered);
    }

    #[test]
    fn smoke_lifecycle_shape_is_constructible() {
        let plan = ScreenCaptureKitSmokeLifecycle {
            target_label: "Display 1 (1920x1080)".to_string(),
            observed_screen_frames: 4,
            observed_audio_samples: 2,
            summary: "smoke lifecycle".to_string(),
        };

        assert_eq!(plan.observed_screen_frames, 4);
        assert_eq!(plan.observed_audio_samples, 2);
    }
}
