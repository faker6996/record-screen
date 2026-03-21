use tauri::AppHandle;

#[derive(Debug, Clone, Copy)]
pub struct PreviewBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct PreviewPresentation {
    pub bounds: PreviewBounds,
    pub title: String,
}

pub fn preview_bounds_for_target_with_title(
    app: &AppHandle,
    capture_target_id: &str,
    title: String,
) -> Result<Option<PreviewPresentation>, String> {
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
        .map(|(x, y, width, height)| PreviewPresentation {
            bounds: PreviewBounds {
                x,
                y,
                width,
                height,
            },
            title,
        }));
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
        .map(|(x, y, width, height)| PreviewPresentation {
            bounds: PreviewBounds {
                x,
                y,
                width,
                height,
            },
            title,
        }));
    }

    #[cfg(target_os = "macos")]
    {
        return macos_preview_bounds(
            app,
            capture_target_id,
            &title,
            &settings.capture_target_id,
            settings.region_x,
            settings.region_y,
            settings.region_width,
            settings.region_height,
        );
    }

    #[allow(unreachable_code)]
    Ok(None)
}

#[cfg(target_os = "macos")]
fn macos_preview_bounds(
    _app: &AppHandle,
    capture_target_id: &str,
    title: &str,
    _selected_target_id: &str,
    region_x: u32,
    region_y: u32,
    region_width: u32,
    region_height: u32,
) -> Result<Option<PreviewPresentation>, String> {
    Ok(capture_macos::preview_target_bounds(
        capture_target_id,
        region_x,
        region_y,
        region_width,
        region_height,
    )
    .ok()
    .map(|(x, y, width, height)| PreviewPresentation {
        bounds: PreviewBounds {
            x,
            y,
            width,
            height,
        },
        title: title.to_string(),
    }))
}
