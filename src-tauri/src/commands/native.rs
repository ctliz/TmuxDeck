use crate::audit::{record_kill, tmux_counts};
use crate::commands::utils::{append_identity_env_clears, isolated_agent_command};
use crate::tmux::{check_tmux_installed, is_no_server_err, run_tmux, sanitize_session_name};
use std::process::Command;
use std::sync::Mutex;

static GHOSTTY_LAYOUT_LOCK: Mutex<()> = Mutex::new(());

pub(crate) const NATIVE_OPTION: &str = "@tmuxdeck-native-split";
pub(crate) const WORKSPACE_OPTION: &str = "@tmuxdeck-workspace";
pub(crate) const SLOT_OPTION: &str = "@tmuxdeck-slot";
pub(crate) const TERMINAL_OPTION: &str = "@tmuxdeck-terminal";

#[derive(Debug, Clone)]
pub(crate) struct NativeSlot {
    pub target: String,
    pub slot: String,
}

pub(crate) fn slot_target(workspace: &str, slot: usize) -> String {
    format!("{}__td_slot_{:02}", workspace, slot)
}

fn validate_native_slot_target(target: &str) -> Result<&str, String> {
    let Some((workspace, slot)) = target.rsplit_once("__td_slot_") else {
        return Err("ERR_KILL_SLOT_INVALID_TARGET".to_string());
    };
    let safe_workspace = !workspace.is_empty()
        && workspace.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        });
    if !safe_workspace
        || slot.is_empty()
        || !slot.chars().all(|character| character.is_ascii_digit())
    {
        return Err("ERR_KILL_SLOT_INVALID_TARGET".to_string());
    }
    Ok(target)
}

pub(crate) fn ghostty_native_available() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    let Ok(output) = Command::new("/Applications/Ghostty.app/Contents/MacOS/ghostty")
        .arg("+version")
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.lines().find_map(|line| line.strip_prefix("Ghostty "));
    version
        .and_then(|value| {
            let mut parts = value.split('.').filter_map(|part| part.parse::<u32>().ok());
            Some((parts.next()?, parts.next()?))
        })
        .is_some_and(|version| version >= (1, 3))
}

pub(crate) fn list_native_slots(workspace: &str) -> Result<Vec<NativeSlot>, String> {
    let workspace = sanitize_session_name(workspace)?;
    let output = run_tmux(&[
        "list-sessions",
        "-F",
        "#{session_name}|#{session_attached}|#{@tmuxdeck-native-split}|#{@tmuxdeck-workspace}|#{@tmuxdeck-slot}",
    ])
    .map_err(|e| format!("ERR_TMUX_LIST_FAILED|{}", e))?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&error) {
            return Ok(Vec::new());
        }
        return Err(format!("ERR_TMUX_GENERIC|{}", error));
    }
    let mut slots = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 5 && parts[2] == "1" && parts[3] == workspace {
            slots.push(NativeSlot {
                target: parts[0].to_string(),
                slot: parts[4].to_string(),
            });
        }
    }
    slots.sort_by_key(|slot| slot.slot.parse::<usize>().unwrap_or(usize::MAX));
    Ok(slots)
}

pub(crate) fn create_native_workspace(
    workspace: &str,
    count: usize,
    work_dir: &str,
    agent_commands: &[String],
    agent_ids: &[String],
) -> Result<Vec<NativeSlot>, String> {
    if agent_commands.len() != count || agent_ids.len() != count {
        return Err("ERR_PANE_AGENT_COUNT".to_string());
    }
    let workspace = sanitize_session_name(workspace)?;
    if !list_native_slots(&workspace)?.is_empty()
        || run_tmux(&["has-session", "-t", &workspace]).is_ok_and(|output| output.status.success())
    {
        return Err("ERR_CREATE_OUTPUT_ERR|workspace already exists".to_string());
    }
    let mut created = Vec::new();
    for slot in 1..=count {
        match create_native_slot(
            &workspace,
            slot,
            work_dir,
            &agent_commands[slot - 1],
            &agent_ids[slot - 1],
        ) {
            Ok(info) => created.push(info),
            Err(error) => {
                for info in &created {
                    let _ = run_tmux(&["kill-session", "-t", &info.target]);
                }
                return Err(error);
            }
        }
    }
    Ok(created)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn native_agent_command(
    agent_id: &str,
    command: &str,
    workspace: &str,
    slot: usize,
) -> Result<(String, Option<String>), String> {
    let is_managed_cci = std::path::Path::new(command) == crate::claude_adapter::managed_cci_path();
    if agent_id != "claude" || !is_managed_cci {
        return Ok((command.to_string(), None));
    }
    let id = crate::claude_adapter::random_intercom_id()?;
    let name = format!("{} · Claude {:02}", workspace, slot);
    Ok((
        format!(
            "{} --tui --safe --id {} --name {}",
            shell_quote(command),
            shell_quote(&id),
            shell_quote(&name)
        ),
        Some(id),
    ))
}

fn native_slot_command_args(
    workspace: &str,
    slot: usize,
    work_dir: &str,
    agent_cmd: &str,
    agent_id: &str,
) -> Result<Vec<String>, String> {
    let target = slot_target(workspace, slot);
    let slot_value = slot.to_string();
    let (agent_cmd, intercom_id) = native_agent_command(agent_id, agent_cmd, workspace, slot)?;
    let augmented_path = crate::commands::utils::build_augmented_path_for_command(&agent_cmd);
    let mut args = vec![
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        target.clone(),
        "-e".to_string(),
        format!("PATH={}", augmented_path),
        "-e".to_string(),
        format!("TMUXDECK_WORKSPACE={}", workspace),
        "-e".to_string(),
        format!("TMUXDECK_SLOT={}", slot_value),
    ];
    if !work_dir.is_empty() && work_dir != "~" {
        args.extend(["-c".to_string(), work_dir.to_string()]);
    }
    args.push(isolated_agent_command(&agent_cmd, agent_id != "shell"));
    args.extend([
        ";".to_string(),
        "set-option".to_string(),
        "-w".to_string(),
        "-t".to_string(),
        target.clone(),
        "remain-on-exit".to_string(),
        "on".to_string(),
    ]);
    append_identity_env_clears(&mut args, &target);
    for (option, value) in [
        (NATIVE_OPTION, "1"),
        (WORKSPACE_OPTION, workspace),
        (SLOT_OPTION, slot_value.as_str()),
        (TERMINAL_OPTION, "ghostty"),
        ("@tmuxdeck-agent", agent_id),
        ("status", "off"),
    ] {
        args.extend([
            ";".to_string(),
            "set-option".to_string(),
            "-t".to_string(),
            target.clone(),
            option.to_string(),
            value.to_string(),
        ]);
    }
    if let Some(intercom_id) = intercom_id {
        for (option, value) in [
            ("@tmuxdeck-intercom-id", intercom_id.as_str()),
            (
                "@tmuxdeck-claude-adapter",
                crate::claude_adapter::MANAGED_ADAPTER_MARKER,
            ),
        ] {
            args.extend([
                ";".to_string(),
                "set-option".to_string(),
                "-p".to_string(),
                "-t".to_string(),
                target.clone(),
                option.to_string(),
                value.to_string(),
            ]);
        }
    }
    Ok(args)
}

fn native_slot_setup_error(target: &str, stderr: &str) -> String {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("no such session") || is_no_server_err(stderr) {
        format!("ERR_NATIVE_SLOT_AGENT_EXITED|{}", target)
    } else {
        format!("ERR_CREATE_NATIVE_SLOT_SETUP|{}|{}", target, stderr.trim())
    }
}

fn startup_exit_error(target: &str, pane_status: &str) -> Option<String> {
    let parts: Vec<&str> = pane_status.trim().split('|').collect();
    if parts.first().copied() != Some("1") {
        return None;
    }
    if let Some(signal) = parts.get(2).filter(|value| !value.is_empty()) {
        return Some(format!(
            "ERR_NATIVE_SLOT_AGENT_EXITED|{}|signal|{}",
            target, signal
        ));
    }
    let status = parts
        .get(1)
        .copied()
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    Some(format!(
        "ERR_NATIVE_SLOT_AGENT_EXITED|{}|status|{}",
        target, status
    ))
}

pub(crate) fn create_native_slot(
    workspace: &str,
    slot: usize,
    work_dir: &str,
    agent_cmd: &str,
    agent_id: &str,
) -> Result<NativeSlot, String> {
    let workspace = sanitize_session_name(workspace)?;
    let target = slot_target(&workspace, slot);
    let slot_value = slot.to_string();
    let args = native_slot_command_args(&workspace, slot, work_dir, agent_cmd, agent_id)?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = run_tmux(&refs).map_err(|e| format!("ERR_CREATE_FAILED|{}", e))?;
    if !output.status.success() {
        return Err(native_slot_setup_error(
            &target,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }
    std::thread::sleep(std::time::Duration::from_millis(250));
    let pane = run_tmux(&[
        "list-panes",
        "-t",
        &target,
        "-F",
        "#{pane_dead}|#{pane_dead_status}|#{pane_dead_signal}",
    ])
    .map_err(|e| format!("ERR_CREATE_FAILED|{}", e))?;
    if !pane.status.success() {
        return Err(native_slot_setup_error(
            &target,
            &String::from_utf8_lossy(&pane.stderr),
        ));
    }
    if let Some(error) = startup_exit_error(&target, &String::from_utf8_lossy(&pane.stdout)) {
        let _ = run_tmux(&["kill-session", "-t", &target]);
        return Err(error);
    }
    let remain_off = run_tmux(&["set-option", "-w", "-t", &target, "remain-on-exit", "off"])
        .map_err(|e| format!("ERR_CREATE_FAILED|{}", e))?;
    if !remain_off.status.success() {
        return Err(native_slot_setup_error(
            &target,
            &String::from_utf8_lossy(&remain_off.stderr),
        ));
    }
    Ok(NativeSlot {
        target,
        slot: slot_value,
    })
}

fn run_ghostty_script(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| format!("ERR_TERMINAL_LAUNCH_FAILED|{}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ERR_TERMINAL_RETURN_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub(crate) fn open_native_workspace(
    workspace: &str,
    slots: &[NativeSlot],
    target_count: usize,
) -> Result<(), String> {
    if slots.is_empty() {
        return Err("ERR_TMUX_NO_SERVER".to_string());
    }
    let _layout_guard = GHOSTTY_LAYOUT_LOCK
        .lock()
        .map_err(|_| "ERR_GHOSTTY_LAYOUT_LOCK_POISONED".to_string())?;
    let tmux = check_tmux_installed().ok_or_else(|| "ERR_TMUX_NOT_FOUND".to_string())?;
    let current = ghostty_terminal_count().unwrap_or(slots.len());
    let add_direction = add_direction_for(current, crate::tmux::get_attach_client_size());
    let script = ghostty_layout_script(workspace, slots, &tmux, add_direction, target_count);
    run_ghostty_script(&script)
}

/// 返回当前可见的 native slot 编号（按标题 `TmuxDeck::<workspace>::<slot>`）。
/// 查询发生在创建新 slot 之前，供 add_pane 生成「可见 slots + 新 slot」目标集。
fn parse_visible_slot_numbers(stdout: &str, prefix: &str) -> Vec<usize> {
    let mut numbers: Vec<usize> = stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix(prefix)?.parse().ok())
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

fn validate_visible_slot_swap(
    visible: &[usize],
    slot_a: usize,
    slot_b: usize,
) -> Result<(), String> {
    if visible.contains(&slot_a) && visible.contains(&slot_b) {
        Ok(())
    } else {
        Err("ERR_PANE_INVALID".to_string())
    }
}

pub(crate) fn visible_native_slot_numbers(workspace: &str) -> Result<Vec<usize>, String> {
    let prefix = format!("TmuxDeck::{}::", workspace);
    let script = format!(
        "tell application \"Ghostty\"\nset resultText to \"\"\nrepeat with w in windows\nrepeat with term in terminals of w\nset termName to name of term\nif termName starts with \"{}\" then\nset resultText to resultText & termName & linefeed\nend if\nend repeat\nend repeat\nreturn resultText\nend tell\n",
        applescript_string(&prefix)
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("ERR_TERMINAL_LAUNCH_FAILED|{}", e))?;
    if !output.status.success() {
        return Err(format!(
            "ERR_TERMINAL_RETURN_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(parse_visible_slot_numbers(
        &String::from_utf8_lossy(&output.stdout),
        &prefix,
    ))
}

/// 关闭该 workspace 当前所有 surfaces，再按目标 slots 重建面板式网格。
/// tmux slot sessions 不动，agent 持续运行；视觉上只闪一次。
pub(crate) fn rebuild_native_workspace(
    workspace: &str,
    slots: &[NativeSlot],
) -> Result<(), String> {
    if slots.is_empty() {
        return Err("ERR_TMUX_NO_SERVER".to_string());
    }
    let _layout_guard = GHOSTTY_LAYOUT_LOCK
        .lock()
        .map_err(|_| "ERR_GHOSTTY_LAYOUT_LOCK_POISONED".to_string())?;
    let tmux = check_tmux_installed().ok_or_else(|| "ERR_TMUX_NOT_FOUND".to_string())?;
    let script = ghostty_rebuild_script(workspace, workspace, slots, &tmux);
    run_ghostty_script(&script)
}

fn rebuild_renamed_native_workspace(
    old_workspace: &str,
    new_workspace: &str,
    slots: &[NativeSlot],
) -> Result<(), String> {
    let _layout_guard = GHOSTTY_LAYOUT_LOCK
        .lock()
        .map_err(|_| "ERR_GHOSTTY_LAYOUT_LOCK_POISONED".to_string())?;
    let tmux = check_tmux_installed().ok_or_else(|| "ERR_TMUX_NOT_FOUND".to_string())?;
    let script = ghostty_rebuild_script(old_workspace, new_workspace, slots, &tmux);
    run_ghostty_script(&script)
}

pub(crate) fn swap_native_slots(target_a: &str, target_b: &str) -> Result<(), String> {
    let target_a = validate_native_slot_target(target_a)?;
    let target_b = validate_native_slot_target(target_b)?;
    let workspace_a = target_a
        .rsplit_once("__td_slot_")
        .map(|(workspace, _)| workspace)
        .ok_or_else(|| "ERR_PANE_INVALID".to_string())?;
    let workspace_b = target_b
        .rsplit_once("__td_slot_")
        .map(|(workspace, _)| workspace)
        .ok_or_else(|| "ERR_PANE_INVALID".to_string())?;
    if workspace_a != workspace_b {
        return Err("ERR_PANE_INVALID".to_string());
    }

    let slots = list_native_slots(workspace_a)?;
    let visible = visible_native_slot_numbers(workspace_a)?;
    let slot_a = slots
        .iter()
        .find(|slot| slot.target == target_a)
        .map(|slot| slot.slot.clone())
        .ok_or_else(|| "ERR_PANE_INVALID".to_string())?;
    let slot_b = slots
        .iter()
        .find(|slot| slot.target == target_b)
        .map(|slot| slot.slot.clone())
        .ok_or_else(|| "ERR_PANE_INVALID".to_string())?;
    let number_a = slot_a
        .parse::<usize>()
        .map_err(|_| "ERR_PANE_INVALID".to_string())?;
    let number_b = slot_b
        .parse::<usize>()
        .map_err(|_| "ERR_PANE_INVALID".to_string())?;
    validate_visible_slot_swap(&visible, number_a, number_b)?;

    for (target, slot) in [(target_a, slot_b.as_str()), (target_b, slot_a.as_str())] {
        let output = run_tmux(&[
            "set-option",
            "-t",
            target,
            SLOT_OPTION,
            slot,
            ";",
            "set-environment",
            "-t",
            target,
            "TMUXDECK_SLOT",
            slot,
        ])
        .map_err(|error| format!("ERR_SWAP_PANE_FAILED|{}", error))?;
        if !output.status.success() {
            return Err(format!(
                "ERR_SWAP_PANE_FAILED|{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    let swapped = list_native_slots(workspace_a)?;
    // 元数据交换后按 slot 升序重建；get_tmux_sessions 同样按 slot 排序，
    // 因此 Ghostty 视觉位置与刷新后的卡片 pane 顺序保持一致。
    let target_slots: Vec<_> = visible
        .iter()
        .filter_map(|number| {
            swapped
                .iter()
                .find(|slot| slot.slot.parse::<usize>().ok() == Some(*number))
                .cloned()
        })
        .collect();
    rebuild_native_workspace(workspace_a, &target_slots)
}

pub(crate) fn rename_native_workspace(old_name: &str, new_name: &str) -> Result<bool, String> {
    let old_name = sanitize_session_name(old_name)?;
    let new_name = sanitize_session_name(new_name)?;
    let slots = list_native_slots(&old_name)?;
    if slots.is_empty() {
        return Ok(false);
    }
    if !list_native_slots(&new_name)?.is_empty()
        || run_tmux(&["has-session", "-t", &new_name]).is_ok_and(|output| output.status.success())
    {
        return Err("ERR_RENAME_OUTPUT_ERR|workspace already exists".to_string());
    }

    let mut renamed: Vec<NativeSlot> = Vec::new();
    for slot in &slots {
        let slot_number = slot
            .slot
            .parse::<usize>()
            .map_err(|_| "ERR_RENAME_FAILED|invalid native slot".to_string())?;
        let new_target = slot_target(&new_name, slot_number);
        let output = run_tmux(&["rename-session", "-t", &slot.target, &new_target])
            .map_err(|error| format!("ERR_RENAME_FAILED|{}", error))?;
        if !output.status.success() {
            for previous in &renamed {
                if let Some(original) = slots.iter().find(|item| item.slot == previous.slot) {
                    let _ = run_tmux(&["rename-session", "-t", &previous.target, &original.target]);
                }
            }
            return Err(format!(
                "ERR_RENAME_OUTPUT_ERR|{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        renamed.push(NativeSlot {
            target: new_target,
            slot: slot.slot.clone(),
        });
    }

    for slot in &renamed {
        for (option, value) in [
            (WORKSPACE_OPTION, new_name.as_str()),
            (NATIVE_OPTION, "1"),
            (SLOT_OPTION, slot.slot.as_str()),
            (TERMINAL_OPTION, "ghostty"),
        ] {
            let output = run_tmux(&["set-option", "-t", &slot.target, option, value])
                .map_err(|error| format!("ERR_RENAME_FAILED|{}", error))?;
            if !output.status.success() {
                return Err(format!(
                    "ERR_RENAME_OUTPUT_ERR|{}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
        let environment = run_tmux(&[
            "set-environment",
            "-t",
            &slot.target,
            "TMUXDECK_WORKSPACE",
            &new_name,
        ])
        .map_err(|error| format!("ERR_RENAME_FAILED|{}", error))?;
        if !environment.status.success() {
            return Err(format!(
                "ERR_RENAME_OUTPUT_ERR|{}",
                String::from_utf8_lossy(&environment.stderr)
            ));
        }
    }
    rebuild_renamed_native_workspace(&old_name, &new_name, &renamed)?;
    Ok(true)
}

pub(crate) fn kill_native_slot(target: &str) -> Result<(), String> {
    let target = validate_native_slot_target(target)?;
    let marker = run_tmux(&["show-options", "-v", "-t", &target, NATIVE_OPTION])
        .map_err(|e| format!("ERR_KILL_FAILED|{}", e))?;
    if !marker.status.success() || String::from_utf8_lossy(&marker.stdout).trim() != "1" {
        return Err("ERR_KILL_SLOT_NOT_NATIVE".to_string());
    }
    let before = tmux_counts();
    let output =
        run_tmux(&["kill-session", "-t", &target]).map_err(|e| format!("ERR_KILL_FAILED|{}", e))?;
    let status = if output.status.success() {
        "success"
    } else {
        "failed"
    };
    record_kill("kill_slot", &target, before, tmux_counts(), status);
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ERR_KILL_OUTPUT_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

pub(crate) fn destroy_native_workspace(workspace: &str) -> Result<bool, String> {
    let slots = list_native_slots(workspace)?;
    if slots.is_empty() {
        return Ok(false);
    }
    let before = tmux_counts();
    let mut failed = None;
    for slot in &slots {
        match run_tmux(&["kill-session", "-t", &slot.target]) {
            Ok(output) if output.status.success() => {}
            Ok(output) => failed = Some(String::from_utf8_lossy(&output.stderr).to_string()),
            Err(error) => failed = Some(error.to_string()),
        }
    }
    record_kill(
        "kill_workspace",
        workspace,
        before,
        tmux_counts(),
        if failed.is_none() {
            "success"
        } else {
            "failed"
        },
    );
    failed.map_or(Ok(true), |error| {
        Err(format!("ERR_KILL_OUTPUT_ERR|{}", error))
    })
}

fn applescript_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn config_script(var: &str, workspace: &str, slot: &NativeSlot, tmux: &str) -> String {
    // Ghostty 用 `exec -l <command>` 包装 surface 的 command——多行 shell 逻辑
    // （if/then/else）内联进去会被语法破坏（b18a22c 回归：所有 native workspace
    // 打不开）。防御必须走脚本文件：脚本有 shebang，exec 脚本路径语法正确。
    let script_path = native_slot_script_path(workspace, &slot.slot);
    let tmux_q = shell_single_quote(tmux);
    let target_q = shell_single_quote(&slot.target);
    let script_content = format!(
        "#!/bin/bash\nif {tmux_q} has-session -t {target_q} 2>/dev/null; then\n  exec {tmux_q} attach-session -t {target_q}\nelse\n  echo \"Session {target_q} no longer exists. Starting a shell instead.\"\n  exec \"$SHELL\"\nfi\n"
    );
    let _ = std::fs::write(&script_path, &script_content);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&script_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&script_path, perms);
        }
    }
    format!(
        "set {var} to new surface configuration\nset command of {var} to \"{}\"\nset environment variables of {var} to {{\"TMUXDECK_WORKSPACE={}\", \"TMUXDECK_SLOT={}\"}}\nset wait after command of {var} to false\n",
        applescript_string(&script_path),
        applescript_string(workspace),
        applescript_string(&slot.slot)
    )
}

fn native_slot_script_path(workspace: &str, slot: &str) -> String {
    std::env::temp_dir()
        .join(format!("tmuxdeck-{}-slot-{}.sh", workspace, slot))
        .to_string_lossy()
        .into_owned()
}

fn title(workspace: &str, slot: &str) -> String {
    format!("TmuxDeck::{}::{}", workspace, slot)
}

/// 数当前可见的 Ghostty surface（方案 A 的「当前格子数」）。
///
/// AppleScript `count of terminals of front window`——注意这数的是**前窗口**的
/// terminals。查询失败（Ghostty 未开 / 无窗口）返回 None，调用方降级。
pub(crate) fn ghostty_terminal_count() -> Option<usize> {
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"Ghostty\" to count of terminals of front window",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    s.parse::<usize>().ok()
}

/// 新增 surface 的方向决策（方案 A，用户拍板）。
///
/// - `current_surfaces <= 1` → `"down"`（首个新增：当前格子下方）；
/// - 否则**只有明显横条**（宽 ≥ 2×高，确属上下排列）才往右（right）平衡，
///   其余（竖条、方形、接近方形如 2x2 缺格）一律往下（down）。
///   实测教训：2x2 删一格后 pane 接近方形（如 79x55，ratio 1.44），
///   按 w>=h 判定会一直往右——方形必须默认 down。
pub(crate) fn add_direction_for(
    current_surfaces: usize,
    client: Option<(u32, u32)>,
) -> &'static str {
    if current_surfaces <= 1 {
        return "down";
    }
    match client {
        Some((w, h)) if h > 0 && w >= h * 2 => "right",
        _ => "down",
    }
}

/// 面板式网格：6 格固定 3 列 2 行；其余按 2 列逐行填充。
/// 返回 `(base_terminal_index, direction)`，n 从 2 开始。
fn grid_split_for(total: usize, n: usize) -> (usize, &'static str) {
    if total == 6 {
        if n <= 3 {
            (n - 1, "right")
        } else {
            (n - 3, "down")
        }
    } else if n == 2 {
        (1, "right")
    } else {
        (n - 2, "down")
    }
}

/// 向 AppleScript 追加「新建窗口 + 按面板网格创建 slots」逻辑。
fn append_grid_creation(script: &mut String, workspace: &str, slots: &[NativeSlot], tmux: &str) {
    script.push_str(&config_script("cfg1", workspace, &slots[0], tmux));
    script.push_str("set deckWindow to new window with configuration cfg1\ndelay 0.8\nset t1 to item 1 of terminals of deckWindow\nperform action \"set_surface_title:");
    script.push_str(&applescript_string(&title(workspace, &slots[0].slot)));
    script.push_str("\" on t1\nset anchorTerminal to t1\n");
    for (index, slot) in slots.iter().enumerate().skip(1) {
        let n = index + 1;
        let (base, direction) = grid_split_for(slots.len(), n);
        script.push_str(&config_script(&format!("cfg{}", n), workspace, slot, tmux));
        script.push_str(&format!("set t{n} to split t{base} direction {direction} with configuration cfg{n}\ndelay 0.5\nperform action \"set_surface_title:{}\" on t{n}\nset anchorTerminal to t{n}\n", applescript_string(&title(workspace, &slot.slot))));
    }
}

fn ghostty_layout_script(
    workspace: &str,
    slots: &[NativeSlot],
    tmux: &str,
    add_direction: &str,
    target_count: usize,
) -> String {
    let prefix = format!("TmuxDeck::{}::", workspace);
    let mut script = String::from("tell application \"Ghostty\"\nset deckWindow to missing value\nset anchorTerminal to missing value\nrepeat with w in windows\nrepeat with term in terminals of w\nif name of term starts with \"");
    script.push_str(&applescript_string(&prefix));
    script.push_str("\" then\nset deckWindow to w\nset anchorTerminal to term\nend if\nend repeat\nend repeat\n");
    script.push_str("if deckWindow is missing value then\n");
    append_grid_creation(&mut script, workspace, slots, tmux);
    script.push_str("else\n");
    // else 分支（已有窗口）：只补到 target_count，不无条件补全（方案 A）。
    // openCount = 本 workspace 当前可见 surface 数（按前缀统计，跨所有窗口）；
    // needed = target_count - openCount；反向遍历 slots（新 slot 优先——add_pane
    // 刚建的 slot 才是「新增的那个」，先补它，关闭的旧 slot 保持后台不复活），
    // 补满 needed 即停。
    script.push_str(&format!("set openCount to 0\nrepeat with w in windows\nrepeat with term in terminals of w\nif name of term starts with \"{}\" then set openCount to openCount + 1\nend repeat\nend repeat\nset needed to {} - openCount\nif needed < 0 then set needed to 0\nset addedCount to 0\n", applescript_string(&prefix), target_count));
    for (index, slot) in slots.iter().rev().enumerate() {
        let n = slots.len() - index;
        let slot_title = applescript_string(&title(workspace, &slot.slot));
        script.push_str(&format!("set slotFound to false\nrepeat with w in windows\nrepeat with term in terminals of w\nif name of term is \"{slot_title}\" then\nset slotFound to true\nset deckWindow to w\nset anchorTerminal to term\nend if\nend repeat\nend repeat\nif slotFound is false and addedCount < needed then\n"));
        script.push_str(&config_script(
            &format!("missingCfg{}", n),
            workspace,
            slot,
            tmux,
        ));
        script.push_str(&format!("set addedTerminal to split anchorTerminal direction {add_direction} with configuration missingCfg{n}\ndelay 0.5\nperform action \"set_surface_title:{slot_title}\" on addedTerminal\nset anchorTerminal to addedTerminal\nset addedCount to addedCount + 1\nend if\n"));
    }
    script.push_str("end if\nperform action \"equalize_splits\" on anchorTerminal\nfocus anchorTerminal\nactivate\nend tell\n");
    script
}

fn ghostty_rebuild_script(
    close_workspace: &str,
    workspace: &str,
    slots: &[NativeSlot],
    tmux: &str,
) -> String {
    let prefix = applescript_string(&format!("TmuxDeck::{}::", close_workspace));
    // Ghostty 在最后一个窗口被关掉时会退出：不能 close-old 后再 new-window。
    // native workspace 本来就是独立窗口：先保存含该 workspace surface 的 window id，
    // 创建新网格后按 id 关闭旧窗口。tmux 只短暂双 attach，agent 不受影响。
    let mut script = format!(
        "tell application \"Ghostty\"\nset oldWindowIds to {{}}\nrepeat with w in windows\nset isWorkspaceWindow to false\nrepeat with term in terminals of w\nif name of term starts with \"{prefix}\" then\nset isWorkspaceWindow to true\nexit repeat\nend if\nend repeat\nif isWorkspaceWindow then set end of oldWindowIds to id of w\nend repeat\n"
    );
    append_grid_creation(&mut script, workspace, slots, tmux);
    script.push_str("delay 0.5\nrepeat with oldWindowId in oldWindowIds\nset oldWindow to missing value\nrepeat with w in windows\nif id of w is contents of oldWindowId then\nset oldWindow to contents of w\nexit repeat\nend if\nend repeat\nif oldWindow is not missing value then\nclose window oldWindow\ndelay 0.3\nend if\nend repeat\nperform action \"equalize_splits\" on anchorTerminal\nfocus anchorTerminal\nactivate\nend tell\n");
    script
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_target_is_deterministic_and_safe() {
        assert_eq!(slot_target("workspace", 1), "workspace__td_slot_01");
        assert_eq!(
            validate_native_slot_target("workspace__td_slot_06"),
            Ok("workspace__td_slot_06")
        );
        assert!(validate_native_slot_target("workspace/unsafe__td_slot_01").is_err());
    }

    #[test]
    fn test_long_workspace_slot_target_is_not_truncated() {
        let workspace = "w".repeat(60);
        let target = slot_target(&workspace, 42);
        assert!(target.len() > 60);
        assert_eq!(validate_native_slot_target(&target), Ok(target.as_str()));
    }

    #[test]
    fn test_parse_visible_slot_numbers_filters_sorts_and_deduplicates() {
        let stdout = "TmuxDeck::deck::5\nTmuxDeck::other::9\nTmuxDeck::deck::2\nTmuxDeck::deck::5\nTmuxDeck::deck::bad\n";
        assert_eq!(
            parse_visible_slot_numbers(stdout, "TmuxDeck::deck::"),
            vec![2, 5]
        );
    }

    #[test]
    fn test_native_slot_swap_requires_two_visible_slots() {
        assert_eq!(validate_visible_slot_swap(&[1, 2, 5], 1, 5), Ok(()));
        assert_eq!(
            validate_visible_slot_swap(&[1, 2, 5], 1, 3),
            Err("ERR_PANE_INVALID".to_string())
        );
    }

    #[test]
    fn test_grid_split_rules() {
        // 非 6 格：2 列逐行填充
        assert_eq!(grid_split_for(2, 2), (1, "right"));
        assert_eq!(grid_split_for(3, 3), (1, "down"));
        assert_eq!(grid_split_for(4, 4), (2, "down"));
        assert_eq!(grid_split_for(5, 5), (3, "down"));
        // 6 格：3 列 2 行
        assert_eq!(grid_split_for(6, 2), (1, "right"));
        assert_eq!(grid_split_for(6, 3), (2, "right"));
        assert_eq!(grid_split_for(6, 4), (1, "down"));
        assert_eq!(grid_split_for(6, 5), (2, "down"));
        assert_eq!(grid_split_for(6, 6), (3, "down"));
    }

    #[test]
    fn test_rebuild_three_slot_grid_uses_visible_plus_new_only() {
        // 4 格关掉 03/04，再新增 05 → 重建目标 01/02/05（2+1 面板布局）
        let slots = vec![
            NativeSlot {
                target: slot_target("deck", 1),
                slot: "1".into(),
            },
            NativeSlot {
                target: slot_target("deck", 2),
                slot: "2".into(),
            },
            NativeSlot {
                target: slot_target("deck", 5),
                slot: "5".into(),
            },
        ];
        let script = ghostty_rebuild_script("deck", "deck", &slots, "/opt/homebrew/bin/tmux");
        assert!(script.contains("set end of oldWindowIds to id of w"));
        assert!(script.contains("if id of w is contents of oldWindowId then"));
        assert!(script.contains("close window oldWindow\ndelay 0.3"));
        assert!(script.contains("set_surface_title:TmuxDeck::deck::1"));
        assert!(script.contains("set_surface_title:TmuxDeck::deck::2"));
        assert!(script.contains("set_surface_title:TmuxDeck::deck::5"));
        assert!(!script.contains("set_surface_title:TmuxDeck::deck::3"));
        assert!(!script.contains("set_surface_title:TmuxDeck::deck::4"));
        assert!(script.contains("set t2 to split t1 direction right"));
        assert!(script.contains("set t3 to split t1 direction down"));
        for slot in ["1", "2", "5"] {
            let _ = std::fs::remove_file(native_slot_script_path("deck", slot));
        }
    }

    #[test]
    fn test_add_direction_for() {
        // current_surfaces <= 1 → down（首个新增：当前格子下方）
        assert_eq!(add_direction_for(0, None), "down");
        assert_eq!(add_direction_for(1, None), "down");
        assert_eq!(add_direction_for(1, Some((114, 55))), "down");
        assert_eq!(add_direction_for(1, Some((55, 114))), "down");
        // 2+ 竖条（w<h，纵向分割过）→ down
        assert_eq!(add_direction_for(2, Some((55, 114))), "down");
        assert_eq!(add_direction_for(3, Some((40, 90))), "down");
        // 2+ 明显横条（宽 >= 2*高，上下排列）→ right
        assert_eq!(add_direction_for(2, Some((114, 27))), "right");
        assert_eq!(add_direction_for(3, Some((220, 55))), "right");
        // 接近方形（2x2 缺格，如 79x55 / 100x100）→ down（避免一直 right）
        assert_eq!(add_direction_for(3, Some((79, 55))), "down");
        assert_eq!(add_direction_for(3, Some((100, 100))), "down");
        // 2+ 拿不到尺寸 → down（默认横向，避免一直往右）
        assert_eq!(add_direction_for(2, None), "down");
        assert_eq!(add_direction_for(5, None), "down");
    }

    #[test]
    fn test_else_branch_adds_only_up_to_target_count_newest_first() {
        let slots: Vec<NativeSlot> = (1..=4)
            .map(|slot| NativeSlot {
                target: slot_target("deck", slot),
                slot: slot.to_string(),
            })
            .collect();
        // target=3：只补到 3 个；反向遍历（新 slot 优先）
        let script = ghostty_layout_script("deck", &slots, "/opt/homebrew/bin/tmux", "down", 3);
        assert!(script.contains("set needed to 3 - openCount"));
        assert!(script.contains("repeat with w in windows"));
        // 反向：先处理 slot 4，再 3、2、1
        let pos4 = script.find("missingCfg4").unwrap();
        let pos3 = script.find("missingCfg3").unwrap();
        let pos1 = script.find("missingCfg1").unwrap();
        assert!(pos4 < pos3 && pos3 < pos1);
        // 上限：addedCount < needed 才补
        assert!(script.contains("if slotFound is false and addedCount < needed then"));
        assert!(script.contains("set addedCount to addedCount + 1"));
        // 创建分支（新窗口 2x2 布局）不受影响
        assert!(script.contains("set t2 to split t1 direction right"));
        assert!(script.contains("set t4 to split t2 direction down"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_config_uses_workspace_without_parsing_target() {
        let workspace = "alpha__td_slot_inside";
        let slot = NativeSlot {
            target: slot_target(workspace, 1),
            slot: "1".to_string(),
        };
        let script = config_script("cfg", workspace, &slot, "/opt/homebrew/bin/tmux");
        assert!(script.contains("TMUXDECK_WORKSPACE=alpha__td_slot_inside"));
        // command 必须是脚本文件路径（多行 shell 逻辑内联进 Ghostty 的
        // `exec -l` 包装会被语法破坏——b18a22c 回归，必须走脚本文件）。
        let expected_path = native_slot_script_path(workspace, "1");
        assert!(script.contains(&format!("set command of cfg to \"{}\"", expected_path)));
        assert!(!script.contains("has-session"));
        let written =
            std::fs::read_to_string(&expected_path).expect("script file should be written");
        assert!(written.contains("has-session -t 'alpha__td_slot_inside__td_slot_01'"));
        assert!(written.contains("attach-session -t 'alpha__td_slot_inside__td_slot_01'"));
        let _ = std::fs::remove_file(&expected_path);
    }

    #[test]
    fn test_native_workspace_validates_per_slot_agents() {
        assert!(matches!(
            create_native_workspace("workspace", 2, "/tmp", &["/bin/pi".into()], &["pi".into()]),
            Err(error) if error == "ERR_PANE_AGENT_COUNT"
        ));
    }

    #[test]
    fn test_native_claude_uses_random_cci_slot_identity() {
        let managed = crate::claude_adapter::managed_cci_path()
            .to_string_lossy()
            .to_string();
        let (command, id) = native_agent_command("claude", &managed, "alpha", 2).unwrap();
        let id = id.unwrap();
        assert!(id.starts_with("tmuxdeck-"));
        assert_eq!(
            command,
            format!(
                "'{}' --tui --safe --id '{}' --name 'alpha · Claude 02'",
                managed, id
            )
        );
        assert_eq!(
            native_agent_command("claude", "/opt/bin/claude", "alpha", 2).unwrap(),
            ("/opt/bin/claude".into(), None)
        );
        assert_eq!(
            native_agent_command("custom", "cci --custom", "alpha", 2).unwrap(),
            ("cci --custom".into(), None)
        );
    }

    #[test]
    fn test_native_slot_uses_one_command_queue_and_isolated_agent_env() {
        let args = native_slot_command_args(
            "workspace",
            2,
            "/tmp/project",
            "custom-agent --model 'A B'",
            "custom",
        )
        .unwrap();
        assert_eq!(args.iter().filter(|arg| *arg == "new-session").count(), 1);
        assert_eq!(args.iter().filter(|arg| *arg == "set-option").count(), 7);
        assert_eq!(
            args.iter().filter(|arg| *arg == "set-environment").count(),
            crate::commands::utils::AGENT_IDENTITY_ENV_VARS.len()
        );
        let agent_index = args
            .iter()
            .position(|arg| arg.contains("env -u PI_SESSION_ID"))
            .unwrap();
        let first_option = args.iter().position(|arg| arg == "set-option").unwrap();
        let first_clear = args
            .iter()
            .position(|arg| arg == "set-environment")
            .unwrap();
        assert!(agent_index < first_option && first_option < first_clear);
        assert_eq!(args[first_option + 4], "remain-on-exit");
        assert_eq!(args[first_option + 5], "on");
        assert!(!args.iter().any(|arg| arg == "PI_CODING_AGENT_DIR"));
        assert!(args[agent_index].contains("custom-agent --model"));
        assert!(args.windows(2).any(|pair| pair == [";", "set-option"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == [TERMINAL_OPTION, "ghostty"]));
    }

    #[test]
    fn test_native_slot_error_classifies_early_agent_exit() {
        assert_eq!(
            native_slot_setup_error(
                "workspace__td_slot_02",
                "no such session: workspace__td_slot_02"
            ),
            "ERR_NATIVE_SLOT_AGENT_EXITED|workspace__td_slot_02"
        );
        assert_eq!(
            native_slot_setup_error("workspace__td_slot_02", "bad option"),
            "ERR_CREATE_NATIVE_SLOT_SETUP|workspace__td_slot_02|bad option"
        );
    }

    #[test]
    fn test_startup_exit_status_and_signal_are_reported() {
        assert_eq!(
            startup_exit_error("workspace__td_slot_01", "1|127|"),
            Some("ERR_NATIVE_SLOT_AGENT_EXITED|workspace__td_slot_01|status|127".to_string())
        );
        assert_eq!(
            startup_exit_error("workspace__td_slot_01", "1||9"),
            Some("ERR_NATIVE_SLOT_AGENT_EXITED|workspace__td_slot_01|signal|9".to_string())
        );
        assert_eq!(startup_exit_error("workspace__td_slot_01", "0||"), None);
    }

    #[test]
    fn test_four_slot_layout_is_two_by_two() {
        let slots: Vec<NativeSlot> = (1..=4)
            .map(|slot| NativeSlot {
                target: slot_target("deck", slot),
                slot: slot.to_string(),
            })
            .collect();
        let script = ghostty_layout_script("deck", &slots, "/opt/homebrew/bin/tmux", "down", 4);
        assert!(script.contains("set t2 to split t1 direction right"));
        assert!(script.contains("set t3 to split t1 direction down"));
        assert!(script.contains("set t4 to split t2 direction down"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_generated_ghostty_script_compiles() {
        if !std::path::Path::new("/Applications/Ghostty.app").exists() {
            return;
        }
        let slots: Vec<NativeSlot> = (1..=4)
            .map(|slot| NativeSlot {
                target: slot_target("compile-test", slot),
                slot: slot.to_string(),
            })
            .collect();
        let scripts = [
            ghostty_layout_script("compile-test", &slots, "/opt/homebrew/bin/tmux", "down", 4),
            ghostty_rebuild_script(
                "compile-test",
                "compile-test",
                &slots,
                "/opt/homebrew/bin/tmux",
            ),
        ];
        for (index, script) in scripts.iter().enumerate() {
            let output_path = std::env::temp_dir().join(format!(
                "tmuxdeck-ghostty-script-{}-{index}.scpt",
                std::process::id()
            ));
            let output = Command::new("osacompile")
                .args(["-e", script, "-o"])
                .arg(&output_path)
                .output()
                .expect("osacompile should be available on macOS");
            let _ = std::fs::remove_file(output_path);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        for slot in 1..=4 {
            let _ =
                std::fs::remove_file(native_slot_script_path("compile-test", &slot.to_string()));
        }
    }
}
