use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use capture::{AudioInputOption, CaptureTargetOption};

const DEVICE_DISCOVERY_TTL: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct DeviceDiscoverySnapshot {
    pub capture_targets: Vec<CaptureTargetOption>,
    pub audio_inputs: Vec<AudioInputOption>,
}

#[derive(Clone)]
struct CachedDeviceDiscovery {
    snapshot: DeviceDiscoverySnapshot,
    refreshed_at: Instant,
}

fn cache() -> &'static Mutex<Option<CachedDeviceDiscovery>> {
    static CACHE: OnceLock<Mutex<Option<CachedDeviceDiscovery>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub fn initial_snapshot() -> DeviceDiscoverySnapshot {
    current_snapshot()
}

pub fn current_snapshot() -> DeviceDiscoverySnapshot {
    load_snapshot(false)
}

pub fn refreshed_snapshot() -> DeviceDiscoverySnapshot {
    load_snapshot(true)
}

fn load_snapshot(force_refresh: bool) -> DeviceDiscoverySnapshot {
    let mut cache = cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if !force_refresh {
        if let Some(entry) = cache.as_ref() {
            if entry.refreshed_at.elapsed() < DEVICE_DISCOVERY_TTL {
                return entry.snapshot.clone();
            }
        }
    }

    let snapshot = discover_devices();
    *cache = Some(CachedDeviceDiscovery {
        snapshot: snapshot.clone(),
        refreshed_at: Instant::now(),
    });

    snapshot
}

#[cfg(target_os = "macos")]
fn discover_devices() -> DeviceDiscoverySnapshot {
    let (capture_targets, audio_inputs) = capture_macos::list_device_options();
    DeviceDiscoverySnapshot {
        capture_targets,
        audio_inputs,
    }
}

#[cfg(target_os = "linux")]
fn discover_devices() -> DeviceDiscoverySnapshot {
    DeviceDiscoverySnapshot {
        capture_targets: capture_linux::list_capture_targets(),
        audio_inputs: capture_linux::list_audio_inputs(),
    }
}

#[cfg(target_os = "windows")]
fn discover_devices() -> DeviceDiscoverySnapshot {
    DeviceDiscoverySnapshot {
        capture_targets: capture_windows::list_capture_targets(),
        audio_inputs: capture_windows::list_audio_inputs(),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn discover_devices() -> DeviceDiscoverySnapshot {
    DeviceDiscoverySnapshot {
        capture_targets: vec![capture::full_desktop_target()],
        audio_inputs: vec![capture::default_audio_input()],
    }
}
