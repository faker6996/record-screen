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
