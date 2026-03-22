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
    pub detail: Option<String>,
    pub style: PreviewStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewStyle {
    Badge,
    RegionOutline,
}

pub fn preview_bounds_for_target_with_title(
    app: &AppHandle,
    capture_target_id: &str,
    title: String,
    style: PreviewStyle,
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
            detail: Some(format!("{} x {}", width, height)),
            style,
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
            detail: Some(format!("{} x {}", width, height)),
            style,
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
            settings.region_source_origin_x,
            settings.region_source_origin_y,
            settings.region_source_scale_factor_milli,
            style,
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
    region_source_origin_x: i32,
    region_source_origin_y: i32,
    region_source_scale_factor_milli: u32,
    style: PreviewStyle,
) -> Result<Option<PreviewPresentation>, String> {
    if capture_target_id == capture::CUSTOM_REGION_TARGET_ID {
        let scale_factor = (f64::from(region_source_scale_factor_milli.max(1)) / 1000.0).max(1.0);
        return Ok(Some(PreviewPresentation {
            bounds: PreviewBounds {
                x: region_source_origin_x + (f64::from(region_x) / scale_factor).round() as i32,
                y: region_source_origin_y + (f64::from(region_y) / scale_factor).round() as i32,
                width: (f64::from(region_width.max(64)) / scale_factor).round() as u32,
                height: (f64::from(region_height.max(64)) / scale_factor).round() as u32,
            },
            title: title.to_string(),
            detail: Some(format!(
                "{} x {}",
                region_width.max(64),
                region_height.max(64)
            )),
            style,
        }));
    }

    Ok(
        capture_macos::logical_preview_target_bounds(capture_target_id).map(
            |(x, y, width, height)| PreviewPresentation {
                bounds: PreviewBounds {
                    x,
                    y,
                    width,
                    height,
                },
                title: title.to_string(),
                detail: Some(format!("{} x {}", width, height)),
                style,
            },
        ),
    )
}
