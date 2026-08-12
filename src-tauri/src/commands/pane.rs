use crate::audit::{record_kill, tmux_counts};
use crate::commands::native::{
    create_native_slot, kill_native_slot, list_native_slots, rebuild_native_workspace,
    swap_native_slots as swap_native_slot_targets, visible_native_slot_numbers,
};
use crate::commands::session::{panel_agent_command, resolve_agent_command};
use crate::commands::utils::isolated_agent_command;
use crate::registry::detect_environment;
use crate::tmux::{
    check_tmux_installed, get_session_first_pane_dir, is_no_server_err, run_tmux,
    sanitize_session_name, strip_ansi, validate_pane_id,
};
use std::sync::Mutex;

static PANE_ADD_LOCK: Mutex<()> = Mutex::new(());
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
pub fn swap_native_slots(session_target_a: String, session_target_b: String) -> Result<(), String> {
    swap_native_slot_targets(&session_target_a, &session_target_b)
}

fn native_slot_numbers(slots: &[crate::commands::native::NativeSlot], count: usize) -> Vec<usize> {
    let first = slots
        .iter()
        .filter_map(|slot| slot.slot.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    (first..first + count).collect()
}

fn standard_split_args(session: &str, work_dir: &str, command: &str) -> Vec<String> {
    let mut args = vec![
        "split-window".to_string(),
        "-t".to_string(),
        session.to_string(),
    ];
    if !work_dir.is_empty() && work_dir != "~" {
        args.extend(["-c".to_string(), work_dir.to_string()]);
    }
    args.extend([
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
        command.to_string(),
    ]);
    args
}

fn standard_pane_metadata_args(
    pane_id: &str,
    agent_id: &str,
    intercom_id: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "set-option".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        pane_id.to_string(),
        "@tmuxdeck-agent".to_string(),
        agent_id.to_string(),
    ];
    if let Some(intercom_id) = intercom_id {
        args.extend([
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.to_string(),
            "@tmuxdeck-intercom-id".to_string(),
            intercom_id.to_string(),
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.to_string(),
            "@tmuxdeck-claude-adapter".to_string(),
            crate::claude_adapter::MANAGED_ADAPTER_MARKER.to_string(),
        ]);
    }
    args
}

fn rollback_targets(command: &str, targets: &[String]) -> Result<(), String> {
    let mut failures = Vec::new();
    for target in targets.iter().rev() {
        let exists = run_tmux(&["has-session", "-t", target]);
        if command == "kill-session" && exists.is_ok_and(|output| !output.status.success()) {
            continue;
        }
        match run_tmux(&[command, "-t", target]) {
            Ok(output) if output.status.success() => {}
            Ok(output) => failures.push(format!(
                "{}:{}",
                target,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(error) => failures.push(format!("{}:{}", target, error)),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join(";"))
    }
}

fn rollback_standard_panes(session: &str, pane_ids: &[String]) -> Result<(), String> {
    if pane_ids.is_empty() {
        return Ok(());
    }
    let kill_result = rollback_targets("kill-pane", pane_ids);
    let layout_result = run_tmux(&["select-layout", "-t", session, "tiled"]);
    let mut failures = Vec::new();
    if let Err(error) = kill_result {
        failures.push(error);
    }
    match layout_result {
        Ok(output) if output.status.success() => {}
        Ok(output) => failures.push(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Err(error) => failures.push(error.to_string()),
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join(";"))
    }
}

fn rollback_error(original: String, rollback: Result<(), String>) -> String {
    match rollback {
        Ok(()) => original,
        Err(detail) => format!("ERR_ADD_PANES_ROLLBACK|{}|{}", original, detail),
    }
}

fn add_native_panes(
    workspace: &str,
    native_slots: &[crate::commands::native::NativeSlot],
    work_dir: &str,
    agent_cmd: &str,
    agent_id: &str,
    count: usize,
) -> Result<usize, String> {
    let visible = visible_native_slot_numbers(workspace)?;
    let original_layout: Vec<_> = visible
        .iter()
        .filter_map(|number| {
            native_slots
                .iter()
                .find(|slot| slot.slot.parse::<usize>().ok() == Some(*number))
                .cloned()
        })
        .collect();
    let numbers = native_slot_numbers(native_slots, count);
    let mut attempted_targets = Vec::new();
    let mut created = Vec::new();
    for number in numbers {
        attempted_targets.push(crate::commands::native::slot_target(workspace, number));
        match create_native_slot(workspace, number, work_dir, agent_cmd, agent_id) {
            Ok(slot) => created.push(slot),
            Err(error) => {
                return Err(rollback_error(
                    error,
                    rollback_targets("kill-session", &attempted_targets),
                ));
            }
        }
    }

    let mut target_slots = original_layout.clone();
    target_slots.extend(created.iter().cloned());
    target_slots.sort_by_key(|slot| slot.slot.parse::<usize>().unwrap_or(usize::MAX));
    if let Err(error) = rebuild_native_workspace(workspace, &target_slots) {
        let mut rollback_failures = Vec::new();
        if let Err(detail) = rollback_targets("kill-session", &attempted_targets) {
            rollback_failures.push(detail);
        }
        if !original_layout.is_empty() {
            if let Err(detail) = rebuild_native_workspace(workspace, &original_layout) {
                rollback_failures.push(detail);
            }
        }
        return Err(if rollback_failures.is_empty() {
            error
        } else {
            format!(
                "ERR_ADD_PANES_ROLLBACK|{}|{}",
                error,
                rollback_failures.join(";")
            )
        });
    }
    Ok(count)
}

fn add_standard_panes(
    session: &str,
    work_dir: &str,
    agent_cmd: &str,
    agent_id: &str,
    count: usize,
) -> Result<usize, String> {
    let first_pane_number = crate::tmux::get_session_panes(session, false, None).len() + 1;
    let mut created = Vec::new();
    for offset in 0..count {
        let (pane_agent_cmd, intercom_id) =
            match panel_agent_command(agent_id, agent_cmd, session, first_pane_number + offset) {
                Ok(command) => command,
                Err(error) => {
                    return Err(rollback_error(
                        error,
                        rollback_standard_panes(session, &created),
                    ));
                }
            };
        let isolated_agent = isolated_agent_command(&pane_agent_cmd, agent_id != "shell");
        let split_args = standard_split_args(session, work_dir, &isolated_agent);
        let refs: Vec<&str> = split_args.iter().map(String::as_str).collect();
        let output = match run_tmux(&refs) {
            Ok(output) => output,
            Err(error) => {
                let original = format!("ERR_ADD_PANE_FAILED|{}", error);
                return Err(rollback_error(
                    original,
                    rollback_standard_panes(session, &created),
                ));
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let original = if is_no_server_err(&stderr) {
                "ERR_TMUX_NO_SERVER".to_string()
            } else {
                format!("ERR_ADD_PANE_OUTPUT_ERR|{}", stderr.trim())
            };
            return Err(rollback_error(
                original,
                rollback_standard_panes(session, &created),
            ));
        }
        let Some(pane_id) = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|value| value.starts_with('%'))
            .map(str::to_string)
        else {
            return Err(rollback_error(
                "ERR_ADD_PANE_OUTPUT_ERR|missing pane id".to_string(),
                rollback_standard_panes(session, &created),
            ));
        };
        created.push(pane_id.clone());
        let metadata_args = standard_pane_metadata_args(&pane_id, agent_id, intercom_id.as_deref());
        let refs: Vec<&str> = metadata_args.iter().map(String::as_str).collect();
        let metadata = match run_tmux(&refs) {
            Ok(output) => output,
            Err(error) => {
                return Err(rollback_error(
                    format!("ERR_ADD_PANE_FAILED|{}", error),
                    rollback_standard_panes(session, &created),
                ));
            }
        };
        if !metadata.status.success() {
            return Err(rollback_error(
                format!(
                    "ERR_ADD_PANE_OUTPUT_ERR|{}",
                    String::from_utf8_lossy(&metadata.stderr).trim()
                ),
                rollback_standard_panes(session, &created),
            ));
        }
    }

    let layout = run_tmux(&["select-layout", "-t", session, "tiled"])
        .map_err(|error| format!("ERR_ADD_PANE_FAILED|{}", error));
    match layout {
        Ok(output) if output.status.success() => Ok(count),
        Ok(output) => Err(rollback_error(
            format!(
                "ERR_ADD_PANE_OUTPUT_ERR|{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            rollback_standard_panes(session, &created),
        )),
        Err(error) => Err(rollback_error(
            error,
            rollback_standard_panes(session, &created),
        )),
    }
}

#[tauri::command]
pub fn add_panes(
    session_name: String,
    agent_id: Option<String>,
    count: u8,
) -> Result<usize, String> {
    if !(1..=6).contains(&count) {
        return Err(format!("ERR_ADD_PANES_COUNT|{}", count));
    }
    let sanitized = sanitize_session_name(&session_name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }
    let _guard = PANE_ADD_LOCK
        .lock()
        .map_err(|_| "ERR_ADD_PANE_FAILED|lock".to_string())?;
    let native_slots = list_native_slots(&sanitized)?;
    let work_dir_target = native_slots
        .first()
        .map(|slot| slot.target.as_str())
        .unwrap_or(sanitized.as_str());
    let work_dir = get_session_first_pane_dir(work_dir_target).unwrap_or_else(|| "~".to_string());
    let agent_id = agent_id.unwrap_or_else(|| "shell".to_string());
    let agent_cmd = resolve_agent_command(&agent_id, &detect_environment().agents)?;
    if native_slots.is_empty() {
        add_standard_panes(&sanitized, &work_dir, &agent_cmd, &agent_id, count as usize)
    } else {
        add_native_panes(
            &sanitized,
            &native_slots,
            &work_dir,
            &agent_cmd,
            &agent_id,
            count as usize,
        )
    }
}

#[tauri::command]
pub fn add_pane(session_name: String, agent_id: Option<String>) -> Result<(), String> {
    add_panes(session_name, agent_id, 1).map(|_| ())
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

    #[test]
    fn add_panes_rejects_counts_outside_one_through_six() {
        assert_eq!(
            add_panes("workspace".into(), None, 0),
            Err("ERR_ADD_PANES_COUNT|0".to_string())
        );
        assert_eq!(
            add_panes("workspace".into(), None, 7),
            Err("ERR_ADD_PANES_COUNT|7".to_string())
        );
    }

    #[test]
    fn native_batch_uses_contiguous_numbers_after_the_max_slot() {
        let slots = vec![
            crate::commands::native::NativeSlot {
                target: "deck__td_slot_01".into(),
                slot: "1".into(),
            },
            crate::commands::native::NativeSlot {
                target: "deck__td_slot_04".into(),
                slot: "4".into(),
            },
        ];
        assert_eq!(native_slot_numbers(&slots, 3), vec![5, 6, 7]);
    }

    #[test]
    fn standard_batch_split_command_preserves_target_workdir_and_output_id() {
        assert_eq!(
            standard_split_args("deck", "/tmp/project", "agent --flag"),
            [
                "split-window",
                "-t",
                "deck",
                "-c",
                "/tmp/project",
                "-P",
                "-F",
                "#{pane_id}",
                "agent --flag",
            ]
        );
        assert!(!standard_split_args("deck", "~", "shell")
            .iter()
            .any(|arg| arg == "-c"));
    }

    #[test]
    fn standard_metadata_includes_managed_identity_and_marker() {
        let args = standard_pane_metadata_args("%4", "claude", Some("tmuxdeck-random"));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["@tmuxdeck-agent", "claude"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["@tmuxdeck-intercom-id", "tmuxdeck-random"]));
        assert!(args.windows(2).any(|pair| {
            pair == [
                "@tmuxdeck-claude-adapter",
                crate::claude_adapter::MANAGED_ADAPTER_MARKER,
            ]
        }));
    }

    #[test]
    fn standard_metadata_omits_managed_fields_for_other_agents() {
        let args = standard_pane_metadata_args("%4", "pi", None);
        assert_eq!(
            args,
            ["set-option", "-p", "-t", "%4", "@tmuxdeck-agent", "pi"]
        );
    }
}
