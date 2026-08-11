use std::collections::HashMap;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::audit::{observe_session_count, record_kill, tmux_counts};
use crate::commands::native::{
    create_native_workspace, destroy_native_workspace, ghostty_native_available, list_native_slots,
    open_native_workspace, TERMINAL_OPTION,
};
use crate::commands::utils::{
    append_identity_env_clears, isolated_agent_command, to_wsl_path,
};
use crate::config::{load_config, save_config};
use crate::models::{CreateOpts, TmuxSession};
use crate::registry::{detect_environment, ToolInfo};
use crate::tmux::{
    check_tmux_installed, get_session_panes, has_attached_clients, is_no_server_err,
    is_session_attached, run_tmux, sanitize_session_name,
};

#[tauri::command]
pub fn open_session(name: String, terminal_id: String) -> Result<(), String> {
    let sanitized_name = sanitize_session_name(&name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }
    let native_slots = list_native_slots(&sanitized_name)?;
    if !native_slots.is_empty() {
        return open_native_workspace(&sanitized_name, &native_slots);
    }

    if let Ok(out) = run_tmux(&["has-session", "-t", &sanitized_name]) {
        if !out.status.success() {
            let err_msg = String::from_utf8_lossy(&out.stderr);
            if is_no_server_err(&err_msg) {
                return Err("ERR_TMUX_NO_SERVER".to_string());
            }
        }
    }

    if is_session_attached(&sanitized_name) {
        #[cfg(target_os = "windows")]
        {
            let ps_script = format!(
                "(New-Object -ComObject WScript.Shell).AppActivate('{}')",
                sanitized_name
            );
            let _ = Command::new("powershell.exe")
                .args(["-Command", &ps_script])
                .status();
            return Ok(());
        }

        #[cfg(not(target_os = "windows"))]
        {
            let (proc_name, app_name) = match terminal_id.as_str() {
                "ghostty" => ("Ghostty", "Ghostty"),
                "iterm2" => ("iTerm2", "iTerm2"),
                "terminal" => ("Terminal", "Terminal"),
                "wezterm" => ("WezTerm", "WezTerm"),
                "kitty" => ("kitty", "kitty"),
                "alacritty" => ("Alacritty", "Alacritty"),
                _ => ("Terminal", "Terminal"),
            };

            // Strategy C: Precise window focus by session title via System Events
            let focus_script = format!(
                "tell application \"System Events\"\n\
                 tell process \"{}\"\n\
                 repeat with w in windows\n\
                 if name of w contains \"{}\" then\n\
                 set frontmost of process \"{}\" to true\n\
                 perform action \"AXRaise\" of w\n\
                 return\n\
                 end if\n\
                 end repeat\n\
                 end tell\n\
                 end tell",
                proc_name, sanitized_name, proc_name
            );

            let status = Command::new("osascript")
                .args(["-e", &focus_script])
                .status();

            if let Ok(st) = status {
                if st.success() {
                    return Ok(());
                }
            }

            // Strategy A: Fallback to activating App
            let activate_script = format!("tell application \"{}\" to activate", app_name);
            let status_a = Command::new("osascript")
                .args(["-e", &activate_script])
                .status();

            if let Ok(st) = status_a {
                if st.success() {
                    return Ok(());
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let status = match terminal_id.as_str() {
            "wt" => Command::new("wt.exe")
                .args([
                    "new-tab",
                    "--",
                    "wsl.exe",
                    "--",
                    "tmux",
                    "attach-session",
                    "-t",
                    &sanitized_name,
                ])
                .status(),
            "powershell" => Command::new("powershell.exe")
                .args([
                    "-NoExit",
                    "-Command",
                    &format!("wsl.exe -- tmux attach-session -t '{}'", sanitized_name),
                ])
                .status(),
            _ => Command::new("cmd.exe")
                .args([
                    "/c",
                    "start",
                    "cmd",
                    "/k",
                    "wsl.exe",
                    "--",
                    "tmux",
                    "attach-session",
                    "-t",
                    &sanitized_name,
                ])
                .status(),
        };
        return match status {
            Ok(st) if st.success() => Ok(()),
            Ok(st) => Err(format!("ERR_TERMINAL_RETURN_ERR|{}", st)),
            Err(e) => Err(format!("ERR_TERMINAL_LAUNCH_FAILED|{}", e)),
        };
    }

    #[cfg(not(target_os = "windows"))]
    {
        let tmux = check_tmux_installed().ok_or_else(|| "ERR_TMUX_NOT_FOUND".to_string())?;
        let script_path = format!("/tmp/tmuxdeck-{}.sh", sanitized_name);

        let script_content = format!(
            "#!/bin/bash\nexec '{}' attach-session -t '{}'\n",
            tmux, sanitized_name
        );
        std::fs::write(&script_path, script_content)
            .map_err(|e| format!("ERR_SCRIPT_WRITE_FAILED|{}", e))?;

        #[cfg(unix)]
        {
            if let Ok(meta) = std::fs::metadata(&script_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&script_path, perms);
            }
        }

        let status = match terminal_id.as_str() {
            "ghostty" => Command::new("osascript")
                .args([
                    "-e",
                    &format!(
                        "tell application \"Ghostty\" to new window with configuration {{command: \"{}\"}}",
                        script_path
                    ),
                ])
                .status(),
            "iterm2" => Command::new("osascript")
                .args([
                    "-e",
                    &format!(
                        "tell application \"iTerm\" to create window with default profile command \"{}\"",
                        script_path
                    ),
                ])
                .status(),
            "terminal" => Command::new("osascript")
                .args([
                    "-e",
                    &format!(
                        "tell application \"Terminal\" to do script \"{}\"",
                        script_path
                    ),
                    "-e",
                    "tell application \"Terminal\" to activate",
                ])
                .status(),
            "wezterm" => Command::new("/usr/bin/open")
                .args(["-na", "WezTerm", "--args", "start", "--", &script_path])
                .status(),
            "kitty" => Command::new("/usr/bin/open")
                .args(["-na", "kitty", "--args", &script_path])
                .status(),
            "alacritty" => Command::new("/usr/bin/open")
                .args(["-na", "Alacritty", "--args", "-e", &script_path])
                .status(),
            _ => Command::new("osascript")
                .args([
                    "-e",
                    &format!(
                        "tell application \"Terminal\" to do script \"{}\"",
                        script_path
                    ),
                    "-e",
                    "tell application \"Terminal\" to activate",
                ])
                .status(),
        };

        match status {
            Ok(st) if st.success() => Ok(()),
            Ok(st) => Err(format!("ERR_TERMINAL_RETURN_ERR|{}", st)),
            Err(e) => Err(format!("ERR_TERMINAL_LAUNCH_FAILED|{}", e)),
        }
    }
}

fn resolve_agent_command(agent_id: &str, agents: &[ToolInfo]) -> String {
    agents
        .iter()
        .find(|agent| agent.id == agent_id)
        .map(|agent| agent.path.clone())
        .unwrap_or_else(|| agent_id.to_string())
}

#[tauri::command]
pub fn create_session(opts: CreateOpts) -> Result<(), String> {
    let sanitized_name = sanitize_session_name(&opts.name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let env_info = detect_environment();

    let agent_cmd = resolve_agent_command(&opts.agent_id, &env_info.agents);

    let work_dir_clean = opts
        .dir
        .clone()
        .filter(|d| !d.trim().is_empty() && (d == "~" || std::path::Path::new(d).exists()))
        .map(|d| to_wsl_path(d))
        .unwrap_or_else(|| {
            if cfg!(target_os = "windows") {
                "~".to_string()
            } else {
                std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
            }
        });

    let count = match opts.panes {
        1 | 2 | 4 | 6 => opts.panes as usize,
        _ => 4,
    };

    if opts.terminal_id == "ghostty" && ghostty_native_available() {
        let slots = create_native_workspace(&sanitized_name, count, &work_dir_clean, &agent_cmd)?;
        if open_native_workspace(&sanitized_name, &slots).is_ok() {
            save_create_defaults(&opts);
            return Ok(());
        }
        for slot in &slots {
            let _ = run_tmux(&["kill-session", "-t", &slot.target]);
        }
    }

    let augmented_path = crate::commands::utils::build_augmented_path_for_command(&agent_cmd);
    let mut new_args = vec![
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        sanitized_name.clone(),
        "-e".to_string(),
        format!("PATH={}", augmented_path),
    ];
    if !work_dir_clean.is_empty() && work_dir_clean != "~" {
        new_args.extend(["-c".to_string(), work_dir_clean.clone()]);
    }
    new_args.push(isolated_agent_command(&agent_cmd));
    append_identity_env_clears(&mut new_args, &sanitized_name);
    new_args.extend([
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        sanitized_name.clone(),
        TERMINAL_OPTION.to_string(),
        opts.terminal_id.clone(),
    ]);
    let new_refs: Vec<&str> = new_args.iter().map(String::as_str).collect();

    let output = run_tmux(&new_refs).map_err(|e| format!("ERR_CREATE_FAILED|{}", e))?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_CREATE_OUTPUT_ERR|{}", err_msg));
    }

    for _ in 1..count {
        let mut split_args = vec!["split-window", "-t", &sanitized_name];
        if !work_dir_clean.is_empty() && work_dir_clean != "~" {
            split_args.push("-c");
            split_args.push(&work_dir_clean);
        }
        let isolated_agent = isolated_agent_command(&agent_cmd);
        split_args.push(&isolated_agent);

        let _ = run_tmux(&split_args);
        let _ = run_tmux(&["select-layout", "-t", &sanitized_name, "tiled"]);
    }

    save_create_defaults(&opts);
    open_session(sanitized_name, opts.terminal_id)
}

fn save_create_defaults(opts: &CreateOpts) {
    let mut cfg = load_config();
    cfg.default_terminal = opts.terminal_id.clone();
    cfg.default_agent = opts.agent_id.clone();
    cfg.default_panes = opts.panes;
    if let Some(dir) = &opts.dir {
        if !dir.trim().is_empty() {
            cfg.recent_dirs.retain(|item| item != dir);
            cfg.recent_dirs.insert(0, dir.clone());
            cfg.recent_dirs.truncate(5);
        }
    }
    let _ = save_config(cfg);
}

fn terminal_id_from_metadata(native: bool, value: &str) -> Option<String> {
    if !value.is_empty() {
        Some(value.to_string())
    } else if native {
        Some("ghostty".to_string())
    } else {
        None
    }
}

#[tauri::command]
pub fn get_tmux_sessions() -> Result<Vec<TmuxSession>, String> {
    if check_tmux_installed().is_none() {
        return Ok(Vec::new());
    }

    let output = run_tmux(&[
        "list-sessions",
        "-F",
        "#{session_id}|#{session_name}|#{session_windows}|#{session_attached}|#{session_created}|#{session_activity}|#{@tmuxdeck-native-split}|#{@tmuxdeck-workspace}|#{@tmuxdeck-slot}|#{@tmuxdeck-terminal}",
    ])
    .map_err(|e| format!("ERR_TMUX_LIST_FAILED|{}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            observe_session_count(0, true);
            return Ok(Vec::new());
        }
        return Err(format!("ERR_TMUX_GENERIC|{}", err_msg));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sessions = Vec::new();
    let mut native_groups: HashMap<String, TmuxSession> = HashMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 10 {
            continue;
        }
        let id = parts[0].to_string();
        let target = parts[1].to_string();
        let windows_count = parts[2].parse::<usize>().unwrap_or(1);
        let attached = has_attached_clients(parts[3]);
        let created_ts = parts[4].parse::<i64>().unwrap_or(0);
        let last_active_ts = parts[5].parse::<i64>().unwrap_or(0);
        let native = parts[6] == "1" && !parts[7].is_empty();
        let workspace = parts[7].to_string();
        let slot = (!parts[8].is_empty()).then_some(parts[8]);
        let terminal_id = terminal_id_from_metadata(native, parts[9]);
        let panes = get_session_panes(&target, attached, slot);

        if native {
            let entry = native_groups
                .entry(workspace.clone())
                .or_insert_with(|| TmuxSession {
                    id: format!("native:{}", workspace),
                    name: workspace.clone(),
                    windows_count: 1,
                    panes_count: 0,
                    attached: false,
                    created_at: created_age(created_ts),
                    last_active_ts: 0,
                    panes: Vec::new(),
                    native_split: true,
                    terminal_id: terminal_id.clone(),
                });
            entry.attached |= attached;
            entry.last_active_ts = entry.last_active_ts.max(last_active_ts);
            entry.panes.extend(panes);
            entry.panes_count = entry.panes.len();
        } else {
            let panes_count = panes.len();
            sessions.push(TmuxSession {
                id,
                name: target,
                windows_count,
                panes_count,
                attached,
                created_at: created_age(created_ts),
                last_active_ts,
                panes,
                native_split: false,
                terminal_id,
            });
        }
    }
    for mut group in native_groups.into_values() {
        group.panes.sort_by_key(|pane| {
            pane.slot
                .as_deref()
                .and_then(|slot| slot.parse::<usize>().ok())
                .unwrap_or(usize::MAX)
        });
        sessions.push(group);
    }

    observe_session_count(sessions.len(), false);
    Ok(sessions)
}

fn created_age(created_ts: i64) -> String {
    if created_ts <= 0 {
        return "-".to_string();
    }
    let datetime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(created_ts as u64);
    let Ok(elapsed) = datetime.elapsed() else {
        return "0s".to_string();
    };
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86400)
    }
}

#[tauri::command]
pub fn kill_session(session_name: String) -> Result<(), String> {
    let sanitized_name = sanitize_session_name(&session_name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }
    if destroy_native_workspace(&sanitized_name)? {
        return Ok(());
    }

    let before = tmux_counts();
    let output = match run_tmux(&["kill-session", "-t", &sanitized_name]) {
        Ok(output) => output,
        Err(e) => {
            record_kill(
                "kill_session",
                &sanitized_name,
                before,
                tmux_counts(),
                "spawn_error",
            );
            return Err(format!("ERR_KILL_FAILED|{}", e));
        }
    };
    let status = if output.status.success() {
        "success".to_string()
    } else {
        output
            .status
            .code()
            .map(|code| format!("exit_{}", code))
            .unwrap_or_else(|| "signal".to_string())
    };
    record_kill(
        "kill_session",
        &sanitized_name,
        before,
        tmux_counts(),
        &status,
    );

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_KILL_OUTPUT_ERR|{}", err_msg));
    }
    Ok(())
}

fn reject_native_rename(is_native: bool) -> Result<(), String> {
    if is_native {
        Err("ERR_NATIVE_WORKSPACE_RENAME_UNSUPPORTED".to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub fn rename_session(old_name: String, new_name: String) -> Result<(), String> {
    let sanitized_old = sanitize_session_name(&old_name)?;
    let sanitized_new = sanitize_session_name(&new_name)?;

    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }
    reject_native_rename(!list_native_slots(&sanitized_old)?.is_empty())?;

    let output = run_tmux(&["rename-session", "-t", &sanitized_old, &sanitized_new])
        .map_err(|e| format!("ERR_RENAME_FAILED|{}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_RENAME_OUTPUT_ERR|{}", err_msg));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_workspace_rename_is_rejected_before_mutation() {
        assert_eq!(
            reject_native_rename(true),
            Err("ERR_NATIVE_WORKSPACE_RENAME_UNSUPPORTED".to_string())
        );
        assert_eq!(reject_native_rename(false), Ok(()));
    }

    #[test]
    fn resolve_agent_uses_detected_shell_and_agent_paths() {
        let agents = vec![
            ToolInfo {
                id: "shell".to_string(),
                name: "Plain Shell".to_string(),
                path: "/bin/zsh".to_string(),
                icon_path: None,
            },
            ToolInfo {
                id: "pi".to_string(),
                name: "Pi".to_string(),
                path: "/opt/bin/pi".to_string(),
                icon_path: None,
            },
        ];
        assert_eq!(resolve_agent_command("shell", &agents), "/bin/zsh");
        assert_eq!(resolve_agent_command("pi", &agents), "/opt/bin/pi");
        assert_eq!(resolve_agent_command("unknown-agent", &agents), "unknown-agent");
    }

    #[test]
    fn terminal_metadata_preserves_legacy_compatibility() {
        assert_eq!(
            terminal_id_from_metadata(true, ""),
            Some("ghostty".to_string())
        );
        assert_eq!(terminal_id_from_metadata(false, ""), None);
        assert_eq!(
            terminal_id_from_metadata(false, "iterm2"),
            Some("iterm2".to_string())
        );
    }
}
