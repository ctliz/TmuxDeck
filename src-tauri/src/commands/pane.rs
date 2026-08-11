use crate::audit::{record_kill, tmux_counts};
use crate::commands::native::{
    create_native_slot, kill_native_slot, list_native_slots, open_native_workspace,
};
use crate::commands::utils::isolated_agent_command;
use crate::tmux::{
    check_tmux_installed, get_session_first_pane_dir, is_no_server_err, run_tmux,
    sanitize_session_name, strip_ansi, validate_pane_id,
};
use std::sync::Mutex;

static PANE_KILL_LOCK: Mutex<()> = Mutex::new(());

/// 向 pane 发送一段文本。桌面端此前也没有这个能力。
///
/// 自由文本走 tmux 的 `-l`（literal）通道，控制键走 `send_pane_key` 的白名单通道，
/// 两者不混用——否则消息里出现 "C-c" 会被 tmux 当成控制键执行。
#[tauri::command]
pub fn send_pane_text(pane_id: String, text: String, submit: bool) -> Result<(), String> {
    crate::tmux::send_keys(&pane_id, &text, submit)
}

/// 向 pane 发送一个具名控制键（Escape / C-c / 方向键等，白名单内）。
#[tauri::command]
pub fn send_pane_key(pane_id: String, key: String) -> Result<(), String> {
    crate::tmux::send_key_name(&pane_id, &key)
}

/// 列出所有 pane 及其归属、进程与工作目录。
#[tauri::command]
pub fn list_panes() -> Vec<crate::tmux::PaneDetail> {
    crate::tmux::list_all_panes()
}

/// 交换两个 pane 格的物理位置（同 session 内交换布局）。
#[tauri::command]
pub fn swap_pane(pane_id_a: String, pane_id_b: String) -> Result<(), String> {
    crate::tmux::swap_panes(&pane_id_a, &pane_id_b)
}

#[tauri::command]
pub fn add_pane(session_name: String) -> Result<(), String> {
    let sanitized = sanitize_session_name(&session_name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let native_slots = list_native_slots(&sanitized)?;
    let work_dir_target = native_slots
        .first()
        .map(|slot| slot.target.as_str())
        .unwrap_or(sanitized.as_str());
    let work_dir = get_session_first_pane_dir(work_dir_target).unwrap_or_else(|| "~".to_string());
    let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    if !native_slots.is_empty() {
        let next_slot = native_slots
            .iter()
            .filter_map(|slot| slot.slot.parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let created = create_native_slot(&sanitized, next_slot, &work_dir, &shell_path)?;
        let slots = list_native_slots(&sanitized)?;
        if let Err(error) = open_native_workspace(&sanitized, &slots) {
            let _ = run_tmux(&["kill-session", "-t", &created.target]);
            return Err(error);
        }
        return Ok(());
    }

    let mut split_args = vec!["split-window", "-t", &sanitized];
    if !work_dir.is_empty() && work_dir != "~" {
        split_args.push("-c");
        split_args.push(&work_dir);
    }
    let isolated_shell = isolated_agent_command(&shell_path);
    split_args.push(&isolated_shell);

    let output = run_tmux(&split_args).map_err(|e| format!("ERR_ADD_PANE_FAILED|{}", e))?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_ADD_PANE_OUTPUT_ERR|{}", err_msg));
    }

    let _ = run_tmux(&["select-layout", "-t", &sanitized, "tiled"]);
    Ok(())
}

fn pane_kill_context(stdout: &str) -> Result<(String, usize), String> {
    let mut session_name = None;
    let mut pane_count = 0;
    for line in stdout.lines() {
        if let Some((_, session)) = line.split_once('|') {
            pane_count += 1;
            session_name.get_or_insert_with(|| session.to_string());
        }
    }
    let session_name = session_name.ok_or_else(|| "ERR_KILL_PANE_NOT_FOUND".to_string())?;
    if pane_count <= 1 {
        return Err("ERR_KILL_PANE_LAST_IN_SESSION".to_string());
    }
    Ok((session_name, pane_count))
}

#[tauri::command]
pub fn kill_slot(session_target: String) -> Result<(), String> {
    kill_native_slot(&session_target)
}

#[tauri::command]
pub fn kill_pane(pane_id: String) -> Result<(), String> {
    let trimmed = pane_id.trim();
    if !validate_pane_id(trimmed) {
        return Err("ERR_KILL_PANE_INVALID".to_string());
    }
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }
    let _guard = PANE_KILL_LOCK
        .lock()
        .map_err(|_| "ERR_KILL_PANE_FAILED|lock".to_string())?;
    let before = tmux_counts();

    let lookup = match run_tmux(&[
        "list-panes",
        "-s",
        "-t",
        trimmed,
        "-F",
        "#{pane_id}|#{session_name}",
    ]) {
        Ok(output) => output,
        Err(e) => {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "guard_spawn_error",
            );
            return Err(format!("ERR_KILL_PANE_FAILED|{}", e));
        }
    };
    if !lookup.status.success() {
        let err_msg = String::from_utf8_lossy(&lookup.stderr);
        record_kill(
            "kill_pane",
            trimmed,
            before,
            tmux_counts(),
            "guard_query_failed",
        );
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_KILL_PANE_OUTPUT_ERR|{}", err_msg));
    }
    if let Err(error) = pane_kill_context(&String::from_utf8_lossy(&lookup.stdout)) {
        let status = if error == "ERR_KILL_PANE_LAST_IN_SESSION" {
            "rejected_last_pane"
        } else {
            "guard_invalid_output"
        };
        record_kill("kill_pane", trimmed, before, tmux_counts(), status);
        return Err(error);
    }

    let output = match run_tmux(&["kill-pane", "-t", trimmed]) {
        Ok(output) => output,
        Err(e) => {
            record_kill("kill_pane", trimmed, before, tmux_counts(), "spawn_error");
            return Err(format!("ERR_KILL_PANE_FAILED|{}", e));
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
    record_kill("kill_pane", trimmed, before, tmux_counts(), &status);
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_KILL_PANE_OUTPUT_ERR|{}", err_msg));
    }
    Ok(())
}

#[tauri::command]
pub fn capture_pane(pane_id: String, max_lines: usize) -> Result<String, String> {
    if check_tmux_installed().is_none() {
        return Ok(String::new());
    }

    let output = run_tmux(&["capture-pane", "-p", "-t", &pane_id]);
    let out = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Ok(String::new()),
    };

    let stripped = strip_ansi(&out);
    let mut lines: Vec<&str> = stripped
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("───") && !t.starts_with("---")
        })
        .collect();

    let limit = if max_lines == 0 { 5 } else { max_lines };
    if lines.len() > limit {
        lines = lines[lines.len() - limit..].to_vec();
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_pane_is_rejected() {
        assert_eq!(
            pane_kill_context("%3|workspace\n"),
            Err("ERR_KILL_PANE_LAST_IN_SESSION".to_string())
        );
    }

    #[test]
    fn test_multiple_panes_can_be_killed() {
        assert_eq!(
            pane_kill_context("%3|workspace\n%4|workspace\n"),
            Ok(("workspace".to_string(), 2))
        );
    }
}
