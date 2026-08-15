use std::collections::HashMap;
use std::process::Command;

use crate::audit::{observe_session_count, record_kill, tmux_counts};
use crate::commands::native::{
    create_native_workspace, destroy_native_workspace_unlocked, ghostty_native_available,
    list_native_slots, open_native_workspace, rename_native_workspace, NativeSlot, TERMINAL_OPTION,
};
use crate::commands::utils::{append_identity_env_clears, shell_single_quote, to_wsl_path};
use crate::config::{load_config, save_config};
use crate::models::{CreateOpts, TmuxSession};
use crate::registry::{detect_environment, ToolInfo};
use crate::tmux::{
    check_tmux_installed, get_session_panes, has_attached_clients, is_no_server_err,
    is_session_attached, is_session_missing_err, run_tmux, sanitize_session_name,
};

/// 写出防御版 attach 脚本（has-session 守卫 + shell 降级）。
/// 脚本文件内容是运行时读取的——即使旧 Ghostty 窗口的 command 快照
/// 指向此路径，重写后也能优雅降级，不再弹 "failed to launch" 窗口。
fn write_attach_script(sanitized_name: &str) -> Result<String, String> {
    let tmux = check_tmux_installed().ok_or_else(|| "ERR_TMUX_NOT_FOUND".to_string())?;
    let script_path = format!("/tmp/tmuxdeck-{}.sh", sanitized_name);
    let script_content = format!(
        "#!/bin/bash\nif '{}' has-session -t '{}' 2>/dev/null; then\n  exec '{}' attach-session -t '{}'\nelse\n  echo \"Session '{}' no longer exists. Starting a shell instead.\"\n  exec \"$SHELL\"\nfi\n",
        tmux, sanitized_name, tmux, sanitized_name, sanitized_name
    );
    std::fs::write(&script_path, script_content)
        .map_err(|e| format!("ERR_SCRIPT_WRITE_FAILED|{}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&script_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&script_path, perms);
        }
    }
    Ok(script_path)
}

#[tauri::command]
pub fn open_session(name: String, terminal_id: String) -> Result<(), String> {
    let sanitized_name = sanitize_session_name(&name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }
    let native_slots = list_native_slots(&sanitized_name)?;
    if !native_slots.is_empty() {
        // 打开 workspace：补全到全部 slot（行为不变）
        return open_native_workspace(&sanitized_name, &native_slots, native_slots.len());
    }

    if let Ok(out) = run_tmux(&["has-session", "-t", &sanitized_name]) {
        if !out.status.success() {
            let err_msg = String::from_utf8_lossy(&out.stderr);
            if is_no_server_err(&err_msg) {
                return Err("ERR_TMUX_NO_SERVER".to_string());
            }
            // session 已消失：不要继续生成脚本启动终端——attach 必败，
            // 脚本 39ms 退出会让 Ghostty 弹 "failed to launch" 窗口。
            if is_session_missing_err(&err_msg) {
                // 自愈：仍把脚本文件重写为防御版（has-session 守卫 + shell 降级）。
                // 遗留 Ghostty 窗口的 surface command 是创建时的脚本路径快照，
                // 但脚本内容运行时读取——重写后旧窗口按 Cmd+D split 继承的也是
                // 防御脚本，attach 失败时优雅降级而非弹 "failed to launch" 窗口。
                let _ = write_attach_script(&sanitized_name);
                return Err("ERR_SESSION_NOT_FOUND".to_string());
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
        let script_path = write_attach_script(&sanitized_name)
            .map_err(|e| format!("ERR_SCRIPT_WRITE_FAILED|{}", e))?;

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

pub(crate) fn resolve_agent_command(agent_id: &str, agents: &[ToolInfo]) -> Result<String, String> {
    agents
        .iter()
        .find(|agent| agent.id == agent_id)
        .map(|agent| agent.path.clone())
        .ok_or_else(|| format!("ERR_AGENT_NOT_FOUND|{}", agent_id))
}

fn resolve_pane_agents(
    default_agent_id: &str,
    pane_agent_ids: &[String],
    count: usize,
    agents: &[ToolInfo],
) -> Result<(Vec<String>, Vec<String>), String> {
    let ids = if pane_agent_ids.is_empty() {
        vec![default_agent_id.to_string(); count]
    } else {
        if pane_agent_ids.len() != count {
            return Err(format!(
                "ERR_PANE_AGENT_COUNT|{}|{}",
                count,
                pane_agent_ids.len()
            ));
        }
        pane_agent_ids.to_vec()
    };
    let commands = ids
        .iter()
        .map(|agent_id| resolve_agent_command(agent_id, agents))
        .collect::<Result<_, _>>()?;
    Ok((ids, commands))
}

fn is_managed_cci_command(command: &str) -> bool {
    std::path::Path::new(command) == crate::claude_adapter::managed_cci_path()
}

pub(crate) fn panel_agent_command(
    agent_id: &str,
    command: &str,
    workspace: &str,
    pane: usize,
    assigned_session_id: &str,
) -> Result<(String, String), String> {
    let id = assigned_session_id.to_string();
    if agent_id != "claude" || !is_managed_cci_command(command) {
        return Ok((command.to_string(), id));
    }
    let name = format!("{} · Claude {:02}", workspace, pane);
    Ok((
        format!(
            "{} --tui --safe --id {} --name {}",
            shell_single_quote(command),
            shell_single_quote(&id),
            shell_single_quote(&name)
        ),
        id,
    ))
}

fn rollback_create_session(
    session_name: Option<&str>,
    team_run_id: Option<&str>,
    original_err: String,
) -> String {
    let mut rollback_errors = Vec::new();
    if let Some(s) = session_name {
        if let Ok(out) = run_tmux(&["kill-session", "-t", s]) {
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                if !crate::tmux::is_no_server_err(&err)
                    && !crate::tmux::is_session_missing_err(&err)
                {
                    rollback_errors.push(format!("kill_session: {}", err.trim()));
                }
            }
        } else {
            rollback_errors.push(format!("kill_session spawn failed for {}", s));
        }
    }
    if let Some(run_id) = team_run_id {
        if let Err(e) = crate::team::delete_team_manifest(run_id) {
            rollback_errors.push(format!("delete_manifest: {}", e));
        }
    }
    if rollback_errors.is_empty() {
        original_err
    } else {
        format!(
            "ERR_CREATE_FAILED|rollback_failed|{}|{}",
            original_err,
            rollback_errors.join(", ")
        )
    }
}

fn rollback_create_native_workspace(
    slots: &[NativeSlot],
    team_run_id: Option<&str>,
    original_err: String,
) -> String {
    let mut rollback_errors = Vec::new();
    for slot in slots {
        if let Ok(out) = run_tmux(&["kill-session", "-t", &slot.target]) {
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                if !crate::tmux::is_no_server_err(&err)
                    && !crate::tmux::is_session_missing_err(&err)
                {
                    rollback_errors.push(format!("kill_slot {}: {}", slot.target, err.trim()));
                }
            }
        } else {
            rollback_errors.push(format!("kill_slot spawn failed for {}", slot.target));
        }
    }
    if let Some(run_id) = team_run_id {
        if let Err(e) = crate::team::delete_team_manifest(run_id) {
            rollback_errors.push(format!("delete_manifest: {}", e));
        }
    }
    if rollback_errors.is_empty() {
        original_err
    } else {
        format!(
            "ERR_CREATE_FAILED|rollback_failed|{}|{}",
            original_err,
            rollback_errors.join(", ")
        )
    }
}

#[tauri::command]
pub fn create_session(opts: CreateOpts) -> Result<(), String> {
    let _lock = crate::team::TEAM_MUTATION_LOCK
        .lock()
        .map_err(|_| "ERR_CREATE_FAILED|lock".to_string())?;
    let sanitized_name = sanitize_session_name(&opts.name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let env_info = detect_environment();
    let panel_bypass_permissions = load_config().panel_bypass_permissions;
    let count = match opts.panes {
        1 | 2 | 4 | 6 => opts.panes as usize,
        _ => 4,
    };

    let (pane_agent_ids, pane_agent_commands) = resolve_pane_agents(
        &opts.agent_id,
        &opts.pane_agent_ids,
        count,
        &env_info.agents,
    )?;

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

    let scope_id = crate::scope::generate_workspace_scope_id()?;
    let team_run_id = crate::team::generate_team_run_id()?;

    let mut pane_session_ids = Vec::with_capacity(count);
    let mut seen_ids = std::collections::HashSet::new();
    for _ in 0..count {
        let mut id = crate::team::generate_session_id()?;
        while !seen_ids.insert(id.clone()) {
            id = crate::team::generate_session_id()?;
        }
        pane_session_ids.push(id);
    }
    let lead_session_id = pane_session_ids[0].clone();

    let mut members = Vec::with_capacity(count);
    for (idx, s_id) in pane_session_ids.iter().enumerate() {
        members.push(crate::team::TeamMember {
            session_id: s_id.clone(),
            role: if idx == 0 {
                crate::team::ROLE_LEAD.to_string()
            } else {
                crate::team::ROLE_WORKER.to_string()
            },
        });
    }
    let manifest =
        crate::team::create_team_manifest(team_run_id.clone(), lead_session_id.clone(), members)?;
    let manifest_path = crate::team::write_team_manifest(&manifest)?;
    let manifest_path_str = manifest_path.to_string_lossy().to_string();

    if opts.terminal_id == "ghostty" && ghostty_native_available() {
        let slots_result = create_native_workspace(
            &sanitized_name,
            count,
            &work_dir_clean,
            &pane_agent_commands,
            &pane_agent_ids,
            &scope_id,
            &team_run_id,
            &manifest_path_str,
            &pane_session_ids,
        );
        match slots_result {
            Ok(slots) => {
                if let Err(e) = open_native_workspace(&sanitized_name, &slots, slots.len()) {
                    return Err(rollback_create_native_workspace(
                        &slots,
                        Some(&team_run_id),
                        e,
                    ));
                }
                save_create_defaults(&opts);
                return Ok(());
            }
            Err(e) => {
                return Err(rollback_create_session(None, Some(&team_run_id), e));
            }
        }
    }

    let (first_agent_cmd, first_intercom_id) = match panel_agent_command(
        &pane_agent_ids[0],
        &pane_agent_commands[0],
        &sanitized_name,
        1,
        &pane_session_ids[0],
    ) {
        Ok((command, id)) => (
            crate::commands::utils::panel_agent_command(
                &pane_agent_ids[0],
                &command,
                panel_bypass_permissions,
            ),
            id,
        ),
        Err(e) => {
            return Err(rollback_create_session(None, Some(&team_run_id), e));
        }
    };
    let first_team_envs = crate::team::build_pane_team_env(&crate::team::PaneTeamEnvOpts {
        workspace_name: &sanitized_name,
        pane_index: 1,
        agent_id: &pane_agent_ids[0],
        scope_id: &scope_id,
        team_manifest_path: &manifest_path_str,
        session_id: &pane_session_ids[0],
        role: crate::team::ROLE_LEAD,
        lead_session_id: &lead_session_id,
    });
    let augmented_path = crate::commands::utils::build_augmented_path_for_command(&first_agent_cmd);
    let mut new_args = vec![
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        sanitized_name.clone(),
        "-e".to_string(),
        format!("PATH={}", augmented_path),
    ];
    new_args.extend(crate::commands::utils::terminal_capability_envs(Some(
        &opts.terminal_id,
    )));
    new_args.extend([
        "-e".to_string(),
        format!("{}={}", crate::scope::SCOPE_ENV_VAR, scope_id),
        "-e".to_string(),
        format!("AGENT_INTERCOM_TEAM_MANIFEST={}", manifest_path_str),
    ]);
    if !work_dir_clean.is_empty() && work_dir_clean != "~" {
        new_args.extend(["-c".to_string(), work_dir_clean.clone()]);
    }
    new_args.push(
        crate::commands::utils::isolated_agent_command_with_team_env(
            &first_agent_cmd,
            pane_agent_ids[0] != "shell",
            &first_team_envs,
        ),
    );
    append_identity_env_clears(&mut new_args, &sanitized_name);
    new_args.extend([
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        sanitized_name.clone(),
        TERMINAL_OPTION.to_string(),
        opts.terminal_id.clone(),
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        sanitized_name.clone(),
        crate::team::OPTION_LEAD_ID.to_string(),
        lead_session_id.clone(),
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        sanitized_name.clone(),
        crate::team::OPTION_TEAM_RUN_ID.to_string(),
        team_run_id.clone(),
    ]);
    new_args.extend(crate::commands::utils::session_terminal_options(
        &sanitized_name,
    ));
    let new_refs: Vec<&str> = new_args.iter().map(String::as_str).collect();

    let output = match run_tmux(&new_refs) {
        Ok(out) => out,
        Err(e) => {
            return Err(rollback_create_session(
                Some(&sanitized_name),
                Some(&team_run_id),
                format!("ERR_CREATE_FAILED|{}", e),
            ));
        }
    };
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        let err = if is_no_server_err(&err_msg) {
            "ERR_TMUX_NO_SERVER".to_string()
        } else {
            format!("ERR_CREATE_OUTPUT_ERR|{}", err_msg)
        };
        return Err(rollback_create_session(
            Some(&sanitized_name),
            Some(&team_run_id),
            err,
        ));
    }

    let first_pane = run_tmux(&["list-panes", "-t", &sanitized_name, "-F", "#{pane_id}"])
        .ok()
        .filter(|result| result.status.success())
        .and_then(|result| String::from_utf8(result.stdout).ok())
        .and_then(|stdout| stdout.lines().next().map(String::from));
    let Some(first_pane) = first_pane else {
        return Err(rollback_create_session(
            Some(&sanitized_name),
            Some(&team_run_id),
            "ERR_CREATE_OUTPUT_ERR|missing first pane id".to_string(),
        ));
    };

    let mut pane1_opts = vec![
        "set-option".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        first_pane.clone(),
        "@tmuxdeck-agent".to_string(),
        pane_agent_ids[0].clone(),
        ";".to_string(),
        "set-option".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        first_pane.clone(),
        crate::team::OPTION_INTERCOM_ID.to_string(),
        first_intercom_id.clone(),
        ";".to_string(),
        "set-option".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        first_pane.clone(),
        crate::team::OPTION_ROLE.to_string(),
        crate::team::ROLE_LEAD.to_string(),
    ];
    if pane_agent_ids[0] == "claude" && is_managed_cci_command(&pane_agent_commands[0]) {
        pane1_opts.extend([
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            first_pane.clone(),
            "@tmuxdeck-claude-adapter".to_string(),
            crate::claude_adapter::MANAGED_ADAPTER_MARKER.to_string(),
        ]);
    }
    let refs: Vec<&str> = pane1_opts.iter().map(String::as_str).collect();
    let pane1_res = run_tmux(&refs);
    match pane1_res {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            return Err(rollback_create_session(
                Some(&sanitized_name),
                Some(&team_run_id),
                format!(
                    "ERR_CREATE_OUTPUT_ERR|{}",
                    String::from_utf8_lossy(&out.stderr)
                ),
            ));
        }
        Err(e) => {
            return Err(rollback_create_session(
                Some(&sanitized_name),
                Some(&team_run_id),
                format!("ERR_CREATE_FAILED|{}", e),
            ));
        }
    }

    for pane in 2..=count {
        let mut split_args = vec![
            "split-window".to_string(),
            "-P".to_string(),
            "-F".to_string(),
            "#{pane_id}".to_string(),
            "-t".to_string(),
            sanitized_name.clone(),
        ];
        split_args.extend(crate::commands::utils::terminal_capability_envs(Some(
            &opts.terminal_id,
        )));
        split_args.extend([
            "-e".to_string(),
            format!("{}={}", crate::scope::SCOPE_ENV_VAR, scope_id),
            "-e".to_string(),
            format!("AGENT_INTERCOM_TEAM_MANIFEST={}", manifest_path_str),
        ]);
        if !work_dir_clean.is_empty() && work_dir_clean != "~" {
            split_args.push("-c".to_string());
            split_args.push(work_dir_clean.clone());
        }
        let (pane_agent_cmd, intercom_id) = match panel_agent_command(
            &pane_agent_ids[pane - 1],
            &pane_agent_commands[pane - 1],
            &sanitized_name,
            pane,
            &pane_session_ids[pane - 1],
        ) {
            Ok((command, id)) => (
                crate::commands::utils::panel_agent_command(
                    &pane_agent_ids[pane - 1],
                    &command,
                    panel_bypass_permissions,
                ),
                id,
            ),
            Err(e) => {
                return Err(rollback_create_session(
                    Some(&sanitized_name),
                    Some(&team_run_id),
                    e,
                ));
            }
        };
        let pane_team_envs = crate::team::build_pane_team_env(&crate::team::PaneTeamEnvOpts {
            workspace_name: &sanitized_name,
            pane_index: pane,
            agent_id: &pane_agent_ids[pane - 1],
            scope_id: &scope_id,
            team_manifest_path: &manifest_path_str,
            session_id: &pane_session_ids[pane - 1],
            role: crate::team::ROLE_WORKER,
            lead_session_id: &lead_session_id,
        });
        let isolated_agent = crate::commands::utils::isolated_agent_command_with_team_env(
            &pane_agent_cmd,
            pane_agent_ids[pane - 1] != "shell",
            &pane_team_envs,
        );
        split_args.push(isolated_agent);

        let split_refs: Vec<&str> = split_args.iter().map(String::as_str).collect();
        let split_output = match run_tmux(&split_refs) {
            Ok(out) if out.status.success() => out,
            Ok(out) => {
                return Err(rollback_create_session(
                    Some(&sanitized_name),
                    Some(&team_run_id),
                    format!(
                        "ERR_CREATE_OUTPUT_ERR|{}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                ));
            }
            Err(e) => {
                return Err(rollback_create_session(
                    Some(&sanitized_name),
                    Some(&team_run_id),
                    format!("ERR_CREATE_FAILED|{}", e),
                ));
            }
        };

        let Some(pane_id) = String::from_utf8_lossy(&split_output.stdout)
            .lines()
            .find(|value| value.starts_with('%'))
            .map(str::to_string)
        else {
            return Err(rollback_create_session(
                Some(&sanitized_name),
                Some(&team_run_id),
                "ERR_CREATE_OUTPUT_ERR|missing pane id".to_string(),
            ));
        };

        let mut pane_opts = vec![
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.clone(),
            "@tmuxdeck-agent".to_string(),
            pane_agent_ids[pane - 1].clone(),
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.clone(),
            crate::team::OPTION_INTERCOM_ID.to_string(),
            intercom_id.clone(),
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.clone(),
            crate::team::OPTION_ROLE.to_string(),
            crate::team::ROLE_WORKER.to_string(),
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.clone(),
            crate::team::OPTION_MANAGER_TARGET.to_string(),
            lead_session_id.clone(),
        ];
        if pane_agent_ids[pane - 1] == "claude"
            && is_managed_cci_command(&pane_agent_commands[pane - 1])
        {
            pane_opts.extend([
                ";".to_string(),
                "set-option".to_string(),
                "-p".to_string(),
                "-t".to_string(),
                pane_id.clone(),
                "@tmuxdeck-claude-adapter".to_string(),
                crate::claude_adapter::MANAGED_ADAPTER_MARKER.to_string(),
            ]);
        }
        let refs: Vec<&str> = pane_opts.iter().map(String::as_str).collect();
        let pane_opt_res = run_tmux(&refs);
        match pane_opt_res {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                return Err(rollback_create_session(
                    Some(&sanitized_name),
                    Some(&team_run_id),
                    format!(
                        "ERR_CREATE_OUTPUT_ERR|{}",
                        String::from_utf8_lossy(&out.stderr)
                    ),
                ));
            }
            Err(e) => {
                return Err(rollback_create_session(
                    Some(&sanitized_name),
                    Some(&team_run_id),
                    format!("ERR_CREATE_FAILED|{}", e),
                ));
            }
        }

        let _ = run_tmux(&["select-layout", "-t", &sanitized_name, "tiled"]);
    }

    if let Err(e) = open_session(sanitized_name.clone(), opts.terminal_id.clone()) {
        return Err(rollback_create_session(
            Some(&sanitized_name),
            Some(&team_run_id),
            e,
        ));
    }
    save_create_defaults(&opts);
    Ok(())
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

    let _lock = crate::team::TEAM_MUTATION_LOCK
        .lock()
        .map_err(|_| "ERR_KILL_FAILED|lock".to_string())?;

    let team_run_id = run_tmux(&[
        "show-options",
        "-t",
        &sanitized_name,
        "-v",
        crate::team::OPTION_TEAM_RUN_ID,
    ])
    .ok()
    .filter(|out| out.status.success())
    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());

    if destroy_native_workspace_unlocked(&sanitized_name)? {
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
    if let Some(run_id) = team_run_id.filter(|s| !s.is_empty()) {
        crate::team::delete_team_manifest(&run_id)
            .map_err(|e| format!("ERR_KILL_OUTPUT_ERR|manifest_delete|{}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn rename_session(old_name: String, new_name: String) -> Result<(), String> {
    let sanitized_old = sanitize_session_name(&old_name)?;
    let sanitized_new = sanitize_session_name(&new_name)?;

    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let old_native_slots = list_native_slots(&sanitized_old)?;
    if !old_native_slots.is_empty() {
        let old_targets: Vec<&str> = old_native_slots.iter().map(|s| s.target.as_str()).collect();
        let old_scope = crate::scope::read_targets_scope(&old_targets)?;
        rename_native_workspace(&sanitized_old, &sanitized_new)?;
        let new_native_slots = list_native_slots(&sanitized_new)?;
        let new_targets: Vec<&str> = new_native_slots.iter().map(|s| s.target.as_str()).collect();
        let new_scope = crate::scope::read_targets_scope(&new_targets)
            .map_err(|_| "ERR_SCOPE_REATTACH".to_string())?;
        if new_scope != old_scope {
            return Err("ERR_SCOPE_REATTACH".to_string());
        }
        return Ok(());
    }

    let old_scope = crate::scope::read_session_scope(&sanitized_old)?
        .ok_or_else(|| "ERR_SCOPE_UNAVAILABLE".to_string())?;

    let output = run_tmux(&["rename-session", "-t", &sanitized_old, &sanitized_new])
        .map_err(|e| format!("ERR_RENAME_FAILED|{}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err_msg) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_RENAME_OUTPUT_ERR|{}", err_msg));
    }

    let new_scope = crate::scope::read_session_scope(&sanitized_new)
        .map_err(|_| "ERR_SCOPE_REATTACH".to_string())?
        .ok_or_else(|| "ERR_SCOPE_REATTACH".to_string())?;

    if new_scope != old_scope {
        return Err("ERR_SCOPE_REATTACH".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(resolve_agent_command("shell", &agents).unwrap(), "/bin/zsh");
        assert_eq!(resolve_agent_command("pi", &agents).unwrap(), "/opt/bin/pi");
        assert_eq!(
            resolve_agent_command("unknown-agent", &agents),
            Err("ERR_AGENT_NOT_FOUND|unknown-agent".to_string())
        );
    }

    #[test]
    fn pane_agent_resolution_preserves_legacy_and_validates_mixed_lists() {
        let agents = vec![
            ToolInfo {
                id: "pi".into(),
                name: "Pi".into(),
                path: "/bin/pi".into(),
                icon_path: None,
            },
            ToolInfo {
                id: "shell".into(),
                name: "Shell".into(),
                path: "/bin/zsh".into(),
                icon_path: None,
            },
        ];
        let (legacy_ids, legacy_commands) = resolve_pane_agents("pi", &[], 2, &agents).unwrap();
        assert_eq!(legacy_ids, ["pi", "pi"]);
        assert_eq!(legacy_commands, ["/bin/pi", "/bin/pi"]);

        let mixed = vec!["pi".to_string(), "shell".to_string()];
        assert_eq!(
            resolve_pane_agents("pi", &mixed, 2, &agents).unwrap().1,
            ["/bin/pi", "/bin/zsh"]
        );
        assert_eq!(
            resolve_pane_agents("pi", &mixed, 4, &agents),
            Err("ERR_PANE_AGENT_COUNT|4|2".to_string())
        );
        assert_eq!(
            resolve_pane_agents("pi", &["missing".to_string()], 1, &agents),
            Err("ERR_AGENT_NOT_FOUND|missing".to_string())
        );
    }

    #[test]
    fn claude_panel_uses_random_cci_identity_with_plain_claude_fallback() {
        let managed = crate::claude_adapter::managed_cci_path()
            .to_string_lossy()
            .to_string();
        let valid_session_id = "tmuxdeck-a0000000-0000-4000-8000-000000000001";
        let (command, id) =
            panel_agent_command("claude", &managed, "alpha", 1, valid_session_id).unwrap();
        assert_eq!(id, valid_session_id);
        assert_eq!(
            command,
            format!(
                "'{}' --tui --safe --id '{}' --name 'alpha · Claude 01'",
                managed, valid_session_id
            )
        );
        assert_eq!(
            panel_agent_command("claude", "/opt/bin/claude", "alpha", 1, valid_session_id).unwrap(),
            ("/opt/bin/claude".into(), valid_session_id.into())
        );
        assert_eq!(
            panel_agent_command("custom", "cci --custom", "alpha", 1, valid_session_id).unwrap(),
            ("cci --custom".into(), valid_session_id.into())
        );
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
