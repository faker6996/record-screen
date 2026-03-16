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

pub fn preview_bounds_for_target(
    app: &AppHandle,
    capture_target_id: &str,
) -> Result<Option<PreviewPresentation>, String> {
    let settings = crate::with_core(app, |core| core.settings())?;
    let title = preview_title(capture_target_id, &settings);

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

fn preview_title(capture_target_id: &str, settings: &storage::AppSettings) -> String {
    crate::capture_targets::available_capture_targets(settings)
        .into_iter()
        .find(|target| target.id == capture_target_id)
        .map(|target| target.label)
        .unwrap_or_else(|| {
            if capture_target_id == capture::FULL_DESKTOP_TARGET_ID {
                "Full desktop".to_string()
            } else if capture_target_id == capture::CUSTOM_REGION_TARGET_ID {
                "Custom region".to_string()
            } else {
                "Display".to_string()
            }
        })
}
