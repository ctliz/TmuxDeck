use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TmuxPane {
    pub id: String,
    pub command: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TmuxSession {
    pub id: String,
    pub name: String,
    pub windows_count: usize,
    pub panes_count: usize,
    pub attached: bool,
    pub created_at: String,
    pub panes: Vec<TmuxPane>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnvStatus {
    pub tmux_installed: bool,
    pub tmux_path: String,
    pub ghostty_installed: bool,
    pub ghostty_path: String,
    pub pi_installed: bool,
    pub pi_path: String,
}

fn find_binary(bin_name: &str, candidate_paths: &[&str]) -> Option<String> {
    for path in candidate_paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    // Try `which`
    if let Ok(output) = Command::new("which").arg(bin_name).output() {
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    None
}

fn get_tmux_bin() -> String {
    find_binary(
        "tmux",
        &["/opt/homebrew/bin/tmux", "/usr/local/bin/tmux", "/usr/bin/tmux"],
    )
    .unwrap_or_else(|| "tmux".to_string())
}

fn get_ghostty_bin() -> String {
    find_binary(
        "ghostty",
        &["/Applications/Ghostty.app/Contents/MacOS/ghostty", "/usr/local/bin/ghostty", "/opt/homebrew/bin/ghostty"],
    )
    .unwrap_or_else(|| "/Applications/Ghostty.app/Contents/MacOS/ghostty".to_string())
}

fn get_pi_bin() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let nvm_pi = format!("{}/.nvm/versions/node/v24.14.0/bin/pi", home);
    find_binary(
        "pi",
        &[
            "/opt/homebrew/bin/pi",
            "/usr/local/bin/pi",
            &nvm_pi,
        ],
    )
    .unwrap_or_else(|| "pi".to_string())
}

#[tauri::command]
fn check_env() -> EnvStatus {
    let tmux_p = get_tmux_bin();
    let ghostty_p = get_ghostty_bin();
    let pi_p = get_pi_bin();

    EnvStatus {
        tmux_installed: std::path::Path::new(&tmux_p).exists(),
        tmux_path: tmux_p,
        ghostty_installed: std::path::Path::new(&ghostty_p).exists(),
        ghostty_path: ghostty_p,
        pi_installed: std::path::Path::new(&pi_p).exists(),
        pi_path: pi_p,
    }
}

#[tauri::command]
fn get_tmux_sessions() -> Result<Vec<TmuxSession>, String> {
    let tmux = get_tmux_bin();
    let output = Command::new(&tmux)
        .args([
            "list-sessions",
            "-F",
            "#{session_id}|#{session_name}|#{session_windows}|#{session_attached}|#{session_created}",
        ])
        .output()
        .map_err(|e| format!("无法运行 tmux list-sessions: {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if err_msg.contains("no server running") {
            return Ok(Vec::new());
        }
        return Err(format!("tmux 错误: {}", err_msg));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sessions = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 5 {
            let id = parts[0].to_string();
            let name = parts[1].to_string();
            let windows_count = parts[2].parse::<usize>().unwrap_or(1);
            let attached = parts[3] == "1";
            let created_ts = parts[4].parse::<i64>().unwrap_or(0);

            let created_at = if created_ts > 0 {
                let datetime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(created_ts as u64);
                if let Ok(elapsed) = datetime.elapsed() {
                    let secs = elapsed.as_secs();
                    if secs < 60 {
                        format!("{} 秒前", secs)
                    } else if secs < 3600 {
                        format!("{} 分钟前", secs / 60)
                    } else if secs < 86400 {
                        format!("{} 小时前", secs / 3600)
                    } else {
                        format!("{} 天前", secs / 86400)
                    }
                } else {
                    "刚刚".to_string()
                }
            } else {
                "未知".to_string()
            };

            let panes = get_session_panes(&tmux, &name);
            let panes_count = panes.len();

            sessions.push(TmuxSession {
                id,
                name,
                windows_count,
                panes_count,
                attached,
                created_at,
                panes,
            });
        }
    }

    Ok(sessions)
}

fn get_session_panes(tmux: &str, session_name: &str) -> Vec<TmuxPane> {
    let output = Command::new(tmux)
        .args([
            "list-panes",
            "-s",
            "-t",
            session_name,
            "-F",
            "#{pane_id}|#{pane_current_command}|#{pane_active}",
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut panes = Vec::new();
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 3 {
                    panes.push(TmuxPane {
                        id: parts[0].to_string(),
                        command: parts[1].to_string(),
                        active: parts[2] == "1",
                    });
                }
            }
            return panes;
        }
    }
    Vec::new()
}

#[tauri::command]
fn attach_session(session_name: String) -> Result<(), String> {
    let tmux = get_tmux_bin();
    let cmd = format!("{} attach-session -t '{}'", tmux, session_name);

    let output = Command::new("/usr/bin/open")
        .args(["-na", "Ghostty", "--args", &format!("--command={}", cmd)])
        .output()
        .map_err(|e| format!("拉起 Ghostty 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!("Ghostty 启动失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

#[tauri::command]
fn create_4pi_session(session_name: String, working_dir: Option<String>) -> Result<(), String> {
    let tmux = get_tmux_bin();
    let pi = get_pi_bin();

    let work_dir = working_dir.unwrap_or_else(|| {
        std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
    });

    let cd_arg = format!("-c '{}'", work_dir);

    let cmd1 = format!("{} new-session -d -s '{}' {} '{}'", tmux, session_name, cd_arg, pi);
    let cmd2 = format!("{} split-window -h -t '{}' {} '{}'", tmux, session_name, cd_arg, pi);
    let cmd3 = format!("{} split-window -v -t '{}:0.0' {} '{}'", tmux, session_name, cd_arg, pi);
    let cmd4 = format!("{} split-window -v -t '{}:0.1' {} '{}'", tmux, session_name, cd_arg, pi);
    let cmd5 = format!("{} select-layout -t '{}' tiled", tmux, session_name);

    let script = format!("{}; {}; {}; {}; {}", cmd1, cmd2, cmd3, cmd4, cmd5);

    let output = Command::new("/bin/bash")
        .args(["-c", &script])
        .output()
        .map_err(|e| format!("创建 tmux session 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!("创建会话报错: {}", String::from_utf8_lossy(&output.stderr)));
    }

    attach_session(session_name)
}

#[tauri::command]
fn kill_session(session_name: String) -> Result<(), String> {
    let tmux = get_tmux_bin();
    let output = Command::new(&tmux)
        .args(["kill-session", "-t", &session_name])
        .output()
        .map_err(|e| format!("销毁 session 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!("销毁会话失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

#[tauri::command]
fn rename_session(old_name: String, new_name: String) -> Result<(), String> {
    let tmux = get_tmux_bin();
    let output = Command::new(&tmux)
        .args(["rename-session", "-t", &old_name, &new_name])
        .output()
        .map_err(|e| format!("重命名 session 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!("重命名会话失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            check_env,
            get_tmux_sessions,
            attach_session,
            create_4pi_session,
            kill_session,
            rename_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
