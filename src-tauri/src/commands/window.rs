use tauri::AppHandle;

#[tauri::command]
pub fn focus_launcher(app: AppHandle) -> Result<(), String> {
    crate::window::focus_launcher(&app)
}

#[tauri::command]
pub fn show_hud(app: AppHandle) -> Result<(), String> {
    crate::window::show_hud(&app)
}

#[tauri::command]
pub fn hide_hud(app: AppHandle) -> Result<(), String> {
    crate::window::hide_hud(&app)
}

#[tauri::command]
pub fn show_region_selector(app: AppHandle) -> Result<(), String> {
    crate::window::show_region_selector(&app)
}

#[tauri::command]
pub fn hide_region_selector(app: AppHandle) -> Result<(), String> {
    crate::window::hide_region_selector(&app)
}

#[tauri::command]
pub fn start_hud_drag(app: AppHandle) -> Result<(), String> {
    crate::window::start_hud_drag(&app)
}
