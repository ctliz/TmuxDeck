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
    agent_cmd: &str,
) -> Result<Vec<NativeSlot>, String> {
    let workspace = sanitize_session_name(workspace)?;
    if !list_native_slots(&workspace)?.is_empty()
        || run_tmux(&["has-session", "-t", &workspace]).is_ok_and(|output| output.status.success())
    {
        return Err("ERR_CREATE_OUTPUT_ERR|workspace already exists".to_string());
    }
    let mut created = Vec::new();
    for slot in 1..=count {
        match create_native_slot(&workspace, slot, work_dir, agent_cmd) {
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

fn native_slot_command_args(
    workspace: &str,
    slot: usize,
    work_dir: &str,
    agent_cmd: &str,
) -> Vec<String> {
    let target = slot_target(workspace, slot);
    let slot_value = slot.to_string();
    let augmented_path = crate::commands::utils::build_augmented_path_for_command(agent_cmd);
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
    args.push(isolated_agent_command(agent_cmd));
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
    args
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
    let status = parts.get(1).copied().filter(|value| !value.is_empty()).unwrap_or("unknown");
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
) -> Result<NativeSlot, String> {
    let workspace = sanitize_session_name(workspace)?;
    let target = slot_target(&workspace, slot);
    let slot_value = slot.to_string();
    let args = native_slot_command_args(&workspace, slot, work_dir, agent_cmd);
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
    let remain_off = run_tmux(&[
        "set-option",
        "-w",
        "-t",
        &target,
        "remain-on-exit",
        "off",
    ])
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

pub(crate) fn open_native_workspace(workspace: &str, slots: &[NativeSlot]) -> Result<(), String> {
    if slots.is_empty() {
        return Err("ERR_TMUX_NO_SERVER".to_string());
    }
    let _layout_guard = GHOSTTY_LAYOUT_LOCK
        .lock()
        .map_err(|_| "ERR_GHOSTTY_LAYOUT_LOCK_POISONED".to_string())?;
    let tmux = check_tmux_installed().ok_or_else(|| "ERR_TMUX_NOT_FOUND".to_string())?;
    let script = ghostty_layout_script(workspace, slots, &tmux);
    let output = Command::new("osascript")
        .args(["-e", &script])
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
    format!("/tmp/tmuxdeck-{}-slot-{}.sh", workspace, slot)
}

fn title(workspace: &str, slot: &str) -> String {
    format!("TmuxDeck::{}::{}", workspace, slot)
}

fn ghostty_layout_script(workspace: &str, slots: &[NativeSlot], tmux: &str) -> String {
    let prefix = format!("TmuxDeck::{}::", workspace);
    let mut script = String::from("tell application \"Ghostty\"\nset deckWindow to missing value\nset anchorTerminal to missing value\nrepeat with w in windows\nrepeat with term in terminals of w\nif name of term starts with \"");
    script.push_str(&applescript_string(&prefix));
    script.push_str("\" then\nset deckWindow to w\nset anchorTerminal to term\nend if\nend repeat\nend repeat\n");
    script.push_str("if deckWindow is missing value then\n");
    script.push_str(&config_script("cfg1", workspace, &slots[0], tmux));
    script.push_str("set deckWindow to new window with configuration cfg1\ndelay 0.8\nset t1 to item 1 of terminals of deckWindow\nperform action \"set_surface_title:");
    script.push_str(&applescript_string(&title(workspace, &slots[0].slot)));
    script.push_str("\" on t1\nset anchorTerminal to t1\n");
    for (index, slot) in slots.iter().enumerate().skip(1) {
        let n = index + 1;
        script.push_str(&config_script(&format!("cfg{}", n), workspace, slot, tmux));
        let (base, direction) = match (slots.len(), n) {
            (4, 2) => ("t1", "right"),
            (4, 3) => ("t1", "down"),
            (4, 4) => ("t2", "down"),
            (6, 2) => ("t1", "right"),
            (6, 3) => ("t2", "right"),
            (6, 4) => ("t1", "down"),
            (6, 5) => ("t2", "down"),
            (6, 6) => ("t3", "down"),
            _ => ("anchorTerminal", if n % 2 == 0 { "right" } else { "down" }),
        };
        script.push_str(&format!("set t{n} to split {base} direction {direction} with configuration cfg{n}\ndelay 0.5\nperform action \"set_surface_title:{}\" on t{n}\nset anchorTerminal to t{n}\n", applescript_string(&title(workspace, &slot.slot))));
    }
    script.push_str("else\n");
    for (index, slot) in slots.iter().enumerate() {
        let n = index + 1;
        let slot_title = applescript_string(&title(workspace, &slot.slot));
        script.push_str(&format!("set slotFound to false\nrepeat with w in windows\nrepeat with term in terminals of w\nif name of term is \"{slot_title}\" then\nset slotFound to true\nset deckWindow to w\nset anchorTerminal to term\nend if\nend repeat\nend repeat\nif slotFound is false then\n"));
        script.push_str(&config_script(
            &format!("missingCfg{}", n),
            workspace,
            slot,
            tmux,
        ));
        script.push_str(&format!("set addedTerminal to split anchorTerminal direction right with configuration missingCfg{n}\ndelay 0.5\nperform action \"set_surface_title:{slot_title}\" on addedTerminal\nset anchorTerminal to addedTerminal\nend if\n"));
    }
    script.push_str("end if\nperform action \"equalize_splits\" on anchorTerminal\nfocus anchorTerminal\nactivate\nend tell\n");
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
    fn test_native_slot_uses_one_command_queue_and_isolated_agent_env() {
        let args = native_slot_command_args(
            "workspace",
            2,
            "/tmp/project",
            "custom-agent --model 'A B'",
        );
        assert_eq!(args.iter().filter(|arg| *arg == "new-session").count(), 1);
        assert_eq!(args.iter().filter(|arg| *arg == "set-option").count(), 6);
        assert_eq!(
            args.iter().filter(|arg| *arg == "set-environment").count(),
            crate::commands::utils::AGENT_IDENTITY_ENV_VARS.len()
        );
        let agent_index = args
            .iter()
            .position(|arg| arg.contains("env -u PI_SESSION_ID"))
            .unwrap();
        let first_option = args.iter().position(|arg| arg == "set-option").unwrap();
        let first_clear = args.iter().position(|arg| arg == "set-environment").unwrap();
        assert!(agent_index < first_option && first_option < first_clear);
        assert_eq!(args[first_option + 4], "remain-on-exit");
        assert_eq!(args[first_option + 5], "on");
        assert!(!args.iter().any(|arg| arg == "PI_CODING_AGENT_DIR"));
        assert!(args[agent_index].contains("custom-agent --model"));
        assert!(args.windows(2).any(|pair| pair == [";", "set-option"]));
        assert!(args.windows(2).any(|pair| pair == [TERMINAL_OPTION, "ghostty"]));
    }

    #[test]
    fn test_native_slot_error_classifies_early_agent_exit() {
        assert_eq!(
            native_slot_setup_error("workspace__td_slot_02", "no such session: workspace__td_slot_02"),
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
        let script = ghostty_layout_script("deck", &slots, "/opt/homebrew/bin/tmux");
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
        let script = ghostty_layout_script("compile-test", &slots, "/opt/homebrew/bin/tmux");
        let output_path = std::env::temp_dir().join(format!(
            "tmuxdeck-ghostty-script-{}.scpt",
            std::process::id()
        ));
        let output = Command::new("osacompile")
            .args(["-e", &script, "-o"])
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
}
