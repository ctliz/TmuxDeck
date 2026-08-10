use std::process::Command;
use crate::registry::detect_environment;

#[tauri::command]
pub fn get_terminal_icon(terminal_id: String) -> Result<Vec<u8>, String> {
    let env_info = detect_environment();
    let term = env_info
        .terminals
        .iter()
        .find(|t| t.id == terminal_id)
        .ok_or_else(|| "ERR_TERMINAL_NOT_FOUND".to_string())?;

    let icns_path = match &term.icon_path {
        Some(p) => p,
        None => return Err("ERR_ICON_NOT_FOUND".to_string()),
    };

    let out_png = format!("/tmp/tmuxdeck-icon-{}.png", terminal_id);
    let status = Command::new("sips")
        .args(["-s", "format", "png", icns_path, "--out", &out_png])
        .output();

    match status {
        Ok(out) if out.status.success() => {
            let bytes = std::fs::read(&out_png).map_err(|e| format!("ERR_READ_ICON|{}", e))?;
            let _ = std::fs::remove_file(&out_png);
            Ok(bytes)
        }
        _ => Err("ERR_ICON_CONVERT_FAILED".to_string()),
    }
}

#[tauri::command]
pub fn to_wsl_path(path: String) -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = Command::new("wsl.exe").args(["wslpath", "-u", &path]).output() {
            if out.status.success() {
                let converted = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !converted.is_empty() {
                    return converted;
                }
            }
        }
    }
    path
}
