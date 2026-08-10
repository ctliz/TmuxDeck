use crate::tmux::{
    check_tmux_installed, get_session_first_pane_dir, is_no_server_err, run_tmux,
    sanitize_session_name, strip_ansi, validate_pane_id,
};

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

#[tauri::command]
pub fn add_pane(session_name: String) -> Result<(), String> {
    let sanitized = sanitize_session_name(&session_name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let work_dir = get_session_first_pane_dir(&sanitized).unwrap_or_else(|| "~".to_string());
    let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());

    let mut split_args = vec!["split-window", "-t", &sanitized];
    if !work_dir.is_empty() && work_dir != "~" {
        split_args.push("-c");
        split_args.push(&work_dir);
    }
    split_args.push(&shell_path);

    let output = run_tmux(&split_args).map_err(|e| format!("ERR_ADD_PANE_FAILED|{}", e))?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!(
            "ERR_ADD_PANE_OUTPUT_ERR|{}",
            err_msg
        ));
    }

    let _ = run_tmux(&["select-layout", "-t", &sanitized, "tiled"]);
    Ok(())
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

    let output = run_tmux(&["kill-pane", "-t", trimmed]).map_err(|e| format!("ERR_KILL_PANE_FAILED|{}", e))?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!(
            "ERR_KILL_PANE_OUTPUT_ERR|{}",
            err_msg
        ));
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
