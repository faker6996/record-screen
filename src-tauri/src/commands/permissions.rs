use permissions::PermissionCheck;

use crate::bootstrap;

#[tauri::command]
pub fn get_permissions() -> Result<Vec<PermissionCheck>, String> {
    Ok(permissions::probe_permissions(bootstrap::platform_name()))
}

#[tauri::command]
pub fn request_permission(permission_name: String) -> Result<Vec<PermissionCheck>, String> {
    permissions::request_permission(bootstrap::platform_name(), &permission_name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_permission_settings(permission_name: String) -> Result<(), String> {
    permissions::open_permission_settings(bootstrap::platform_name(), &permission_name)
        .map_err(|error| error.to_string())
}
