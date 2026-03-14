use tauri::AppHandle;

#[derive(Debug, Clone, Copy)]
pub struct PreviewBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn preview_bounds_for_target(
    app: &AppHandle,
    capture_target_id: &str,
) -> Result<Option<PreviewBounds>, String> {
    let settings = crate::with_core(app, |core| core.settings())?;

    #[cfg(target_os = "windows")]
    {
        return Ok(capture_windows::preview_target_bounds(
            capture_target_id,
            settings.region_x,
            settings.region_y,
            settings.region_width,
            settings.region_height,
        )
        .ok()
        .map(|(x, y, width, height)| PreviewBounds { x, y, width, height }));
    }

    #[cfg(target_os = "linux")]
    {
        return Ok(capture_linux::preview_target_bounds(
            capture_target_id,
            settings.region_x,
            settings.region_y,
            settings.region_width,
            settings.region_height,
        )
        .ok()
        .map(|(x, y, width, height)| PreviewBounds { x, y, width, height }));
    }

    #[cfg(target_os = "macos")]
    {
        return macos_preview_bounds(app, capture_target_id, &settings.capture_target_id, settings.region_x, settings.region_y, settings.region_width, settings.region_height);
    }

    #[allow(unreachable_code)]
    Ok(None)
}

#[cfg(target_os = "macos")]
fn macos_preview_bounds(
    app: &AppHandle,
    capture_target_id: &str,
    _selected_target_id: &str,
    region_x: u32,
    region_y: u32,
    region_width: u32,
    region_height: u32,
) -> Result<Option<PreviewBounds>, String> {
    use capture::{CUSTOM_REGION_TARGET_ID, FULL_DESKTOP_TARGET_ID};

    if capture_target_id == CUSTOM_REGION_TARGET_ID {
        return Ok(Some(PreviewBounds {
            x: region_x as i32,
            y: region_y as i32,
            width: region_width.max(64),
            height: region_height.max(64),
        }));
    }

    let mut monitors = app.available_monitors().map_err(|error| error.to_string())?;
    if monitors.is_empty() {
        return Ok(None);
    }

    monitors.sort_by_key(|monitor| {
        let position = monitor.position();
        (position.y, position.x)
    });

    if capture_target_id == FULL_DESKTOP_TARGET_ID {
        let min_x = monitors.iter().map(|monitor| monitor.position().x).min().unwrap_or(0);
        let min_y = monitors.iter().map(|monitor| monitor.position().y).min().unwrap_or(0);
        let max_x = monitors
            .iter()
            .map(|monitor| monitor.position().x + monitor.size().width as i32)
            .max()
            .unwrap_or(0);
        let max_y = monitors
            .iter()
            .map(|monitor| monitor.position().y + monitor.size().height as i32)
            .max()
            .unwrap_or(0);

        return Ok(Some(PreviewBounds {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(64) as u32,
            height: (max_y - min_y).max(64) as u32,
        }));
    }

    let targets = capture_macos::list_capture_targets();
    let monitor_targets: Vec<_> = targets
        .into_iter()
        .filter(|target| target.id != FULL_DESKTOP_TARGET_ID)
        .collect();
    let Some(target_index) = monitor_targets
        .iter()
        .position(|target| target.id == capture_target_id)
    else {
        return Ok(None);
    };

    let Some(monitor) = monitors.get(target_index) else {
        return Ok(None);
    };

    Ok(Some(PreviewBounds {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    }))
}
