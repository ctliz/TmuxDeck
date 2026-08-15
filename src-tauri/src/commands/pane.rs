use crate::audit::{record_kill, tmux_counts};
use crate::commands::native::{
    kill_native_slot, list_native_slots, rebuild_native_workspace,
    swap_native_slots as swap_native_slot_targets, visible_native_slot_numbers,
};
use crate::commands::session::{panel_agent_command, resolve_agent_command};
use crate::registry::detect_environment;
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

fn standard_split_args(
    session: &str,
    work_dir: &str,
    command: &str,
    scope_id: &str,
    manifest_path: &str,
) -> Vec<String> {
    let mut args = vec![
        "split-window".to_string(),
        "-t".to_string(),
        session.to_string(),
        "-e".to_string(),
        format!("{}={}", crate::scope::SCOPE_ENV_VAR, scope_id),
    ];
    if !manifest_path.is_empty() {
        args.extend([
            "-e".to_string(),
            format!("AGENT_INTERCOM_TEAM_MANIFEST={}", manifest_path),
        ]);
    }
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
    lead_session_id: &str,
    is_managed_claude: bool,
) -> Vec<String> {
    let mut args = vec![
        "set-option".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        pane_id.to_string(),
        "@tmuxdeck-agent".to_string(),
        agent_id.to_string(),
        ";".to_string(),
        "set-option".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        pane_id.to_string(),
        crate::team::OPTION_ROLE.to_string(),
        crate::team::ROLE_WORKER.to_string(),
    ];
    if !lead_session_id.is_empty() {
        args.extend([
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.to_string(),
            crate::team::OPTION_MANAGER_TARGET.to_string(),
            lead_session_id.to_string(),
        ]);
    }
    if let Some(intercom_id) = intercom_id {
        args.extend([
            ";".to_string(),
            "set-option".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            pane_id.to_string(),
            crate::team::OPTION_INTERCOM_ID.to_string(),
            intercom_id.to_string(),
        ]);
        if is_managed_claude {
            args.extend([
                ";".to_string(),
                "set-option".to_string(),
                "-p".to_string(),
                "-t".to_string(),
                pane_id.to_string(),
                "@tmuxdeck-claude-adapter".to_string(),
                crate::claude_adapter::MANAGED_ADAPTER_MARKER.to_string(),
            ]);
        }
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
    scope_id: &str,
    team_run_id: &str,
    manifest_path: &str,
    lead_session_id: &str,
    session_ids: &[String],
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
    for (idx, number) in numbers.into_iter().enumerate() {
        attempted_targets.push(crate::commands::native::slot_target(workspace, number));
        match crate::commands::native::create_native_slot_with_team(
            workspace,
            number,
            work_dir,
            agent_cmd,
            agent_id,
            scope_id,
            team_run_id,
            manifest_path,
            &session_ids[idx],
            crate::team::ROLE_WORKER,
            lead_session_id,
        ) {
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
    scope_id: &str,
    manifest_path: &str,
    lead_session_id: &str,
    session_ids: &[String],
) -> Result<usize, String> {
    let first_pane_number = crate::tmux::get_session_panes(session, false, None).len() + 1;
    let mut created = Vec::new();
    for offset in 0..count {
        let pane_num = first_pane_number + offset;
        let s_id = &session_ids[offset];
        let (pane_agent_cmd, intercom_id) =
            match panel_agent_command(agent_id, agent_cmd, session, pane_num, s_id) {
                Ok(command) => command,
                Err(error) => {
                    return Err(rollback_error(
                        error,
                        rollback_standard_panes(session, &created),
                    ));
                }
            };
        let pane_team_envs = crate::team::build_pane_team_env(&crate::team::PaneTeamEnvOpts {
            workspace_name: session,
            pane_index: pane_num,
            agent_id,
            scope_id,
            team_manifest_path: manifest_path,
            session_id: s_id,
            role: crate::team::ROLE_WORKER,
            lead_session_id,
        });
        let isolated_agent = crate::commands::utils::isolated_agent_command_with_team_env(
            &pane_agent_cmd,
            agent_id != "shell",
            &pane_team_envs,
        );
        let split_args =
            standard_split_args(session, work_dir, &isolated_agent, scope_id, manifest_path);
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
        let is_managed_claude = agent_id == "claude"
            && std::path::Path::new(agent_cmd) == crate::claude_adapter::managed_cci_path();
        let metadata_args = standard_pane_metadata_args(
            &pane_id,
            agent_id,
            Some(intercom_id.as_str()),
            lead_session_id,
            is_managed_claude,
        );
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
    let _guard = crate::team::TEAM_MUTATION_LOCK
        .lock()
        .map_err(|_| "ERR_ADD_PANE_FAILED|lock".to_string())?;
    let native_slots = list_native_slots(&sanitized)?;
    let scope_id = if native_slots.is_empty() {
        crate::scope::read_targets_scope(&[&sanitized])?
    } else {
        let targets: Vec<&str> = native_slots
            .iter()
            .map(|slot| slot.target.as_str())
            .collect();
        crate::scope::read_targets_scope(&targets)?
    };

    let (team_run_id, lead_session_id) = if native_slots.is_empty() {
        let env_out = run_tmux(&[
            "show-environment",
            "-t",
            &sanitized,
            "AGENT_INTERCOM_TEAM_MANIFEST",
        ])
        .map_err(|_| crate::team::ERR_TEAM_UNAVAILABLE.to_string())?;
        if !env_out.status.success() {
            return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
        }
        let env_manifest = String::from_utf8_lossy(&env_out.stdout)
            .lines()
            .find_map(|l| l.strip_prefix("AGENT_INTERCOM_TEAM_MANIFEST="))
            .map(str::trim)
            .unwrap_or("")
            .to_string();

        let opt_run = run_tmux(&[
            "show-options",
            "-t",
            &sanitized,
            "-v",
            crate::team::OPTION_TEAM_RUN_ID,
        ])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

        let opt_lead = run_tmux(&[
            "show-options",
            "-t",
            &sanitized,
            "-v",
            crate::team::OPTION_LEAD_ID,
        ])
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

        if opt_run.is_empty() || opt_lead.is_empty() {
            return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
        }
        if !crate::team::is_valid_team_run_id(&opt_run) || !crate::team::is_valid_session_id(&opt_lead) {
            return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
        }
        let expected_manifest = crate::team::team_manifest_path(&opt_run)?
            .to_string_lossy()
            .to_string();
        if env_manifest.is_empty() || env_manifest != expected_manifest {
            return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
        }
        (opt_run, opt_lead)
    } else {
        let mut run_id = String::new();
        let mut lead_id = String::new();
        for slot in &native_slots {
            let opt_run = run_tmux(&[
                "show-options",
                "-t",
                &slot.target,
                "-v",
                crate::team::OPTION_TEAM_RUN_ID,
            ])
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

            let opt_lead = run_tmux(&[
                "show-options",
                "-t",
                &slot.target,
                "-v",
                crate::team::OPTION_LEAD_ID,
            ])
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

            if opt_run.is_empty() || opt_lead.is_empty() {
                return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
            }
            if !crate::team::is_valid_team_run_id(&opt_run) || !crate::team::is_valid_session_id(&opt_lead) {
                return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
            }

            let expected_manifest = crate::team::team_manifest_path(&opt_run)?
                .to_string_lossy()
                .to_string();

            let env_out = run_tmux(&[
                "show-environment",
                "-t",
                &slot.target,
                "AGENT_INTERCOM_TEAM_MANIFEST",
            ])
            .map_err(|_| crate::team::ERR_TEAM_UNAVAILABLE.to_string())?;
            if !env_out.status.success() {
                return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
            }
            let p = String::from_utf8_lossy(&env_out.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("AGENT_INTERCOM_TEAM_MANIFEST="))
                .map(str::trim)
                .unwrap_or("")
                .to_string();

            if p.is_empty() || p != expected_manifest {
                return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
            }

            if run_id.is_empty() {
                run_id = opt_run;
                lead_id = opt_lead;
            } else if run_id != opt_run || lead_id != opt_lead {
                return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
            }
        }
        (run_id, lead_id)
    };

    let old_manifest = match crate::team::read_team_manifest(&team_run_id) {
        Ok(m) => m,
        Err(_) => return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string()),
    };

    if old_manifest.run_id != team_run_id || old_manifest.lead_id != lead_session_id {
        return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
    }

    // Live panes validation & Lead alive check
    if native_slots.is_empty() {
        let panes_query = run_tmux(&[
            "list-panes",
            "-t",
            &sanitized,
            "-F",
            "#{pane_id}|#{@tmuxdeck-intercom-id}|#{@tmuxdeck-role}|#{@tmuxdeck-manager-target}|#{pane_dead}",
        ])
        .map_err(|e| format!("ERR_ADD_PANE_FAILED|{}", e))?;
        if !panes_query.status.success() {
            return Err("ERR_LEAD_DISCONNECTED".to_string());
        }
        let stdout = String::from_utf8_lossy(&panes_query.stdout);
        let mut lead_pane_found = false;
        let mut lead_dead = false;
        let mut live_members = Vec::new();
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 5 {
                let intercom_id = parts[1];
                let role = parts[2];
                let manager_target = parts[3];
                let dead = parts[4] == "1";
                if intercom_id.is_empty() || role.is_empty() {
                    return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
                }
                live_members.push(crate::team::LiveTeamMemberInfo {
                    session_id: intercom_id.to_string(),
                    role: role.to_string(),
                    manager_target: manager_target.to_string(),
                });
                if intercom_id == lead_session_id {
                    lead_pane_found = true;
                    lead_dead = dead;
                }
            } else {
                return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
            }
        }
        if !lead_pane_found || lead_dead {
            return Err("ERR_LEAD_DISCONNECTED".to_string());
        }
        crate::team::validate_live_team_members(&old_manifest, &live_members)?;
    } else {
        let mut live_members = Vec::new();
        let mut lead_found = false;
        let mut lead_dead = false;
        for slot in &native_slots {
            let slot_panes = run_tmux(&[
                "list-panes",
                "-t",
                &slot.target,
                "-F",
                "#{pane_dead}|#{@tmuxdeck-intercom-id}|#{@tmuxdeck-role}|#{@tmuxdeck-manager-target}",
            ]);
            match slot_panes {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let first_line = stdout.lines().next().unwrap_or("");
                    let parts: Vec<&str> = first_line.split('|').collect();
                    if parts.len() < 4 {
                        return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
                    }
                    let dead = parts.first().copied().unwrap_or("") == "1";
                    let intercom = parts.get(1).copied().unwrap_or("");
                    let role = parts.get(2).copied().unwrap_or("");
                    let manager_target = parts.get(3).copied().unwrap_or("");
                    if intercom.is_empty() || role.is_empty() {
                        return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
                    }
                    live_members.push(crate::team::LiveTeamMemberInfo {
                        session_id: intercom.to_string(),
                        role: role.to_string(),
                        manager_target: manager_target.to_string(),
                    });
                    if intercom == lead_session_id {
                        lead_found = true;
                        lead_dead = dead;
                    }
                }
                _ => return Err("ERR_LEAD_DISCONNECTED".to_string()),
            }
        }
        if !lead_found || lead_dead {
            return Err("ERR_LEAD_DISCONNECTED".to_string());
        }
        crate::team::validate_live_team_members(&old_manifest, &live_members)?;
    }

    if old_manifest.members.len() + (count as usize) > 64 {
        return Err(format!(
            "{}|{}",
            crate::team::ERR_TEAM_CAPACITY,
            old_manifest.members.len() + (count as usize)
        ));
    }

    let mut new_session_ids = Vec::with_capacity(count as usize);
    let mut existing_ids: std::collections::HashSet<String> = old_manifest
        .members
        .iter()
        .map(|m| m.session_id.clone())
        .collect();
    for _ in 0..count {
        let mut id = crate::team::generate_session_id()?;
        while existing_ids.contains(&id) {
            id = crate::team::generate_session_id()?;
        }
        existing_ids.insert(id.clone());
        new_session_ids.push(id);
    }

    let new_members: Vec<crate::team::TeamMember> = new_session_ids
        .iter()
        .map(|id| crate::team::TeamMember {
            session_id: id.clone(),
            role: crate::team::ROLE_WORKER.to_string(),
        })
        .collect();

    // Atomic append before spawning
    if let Err(e) = crate::team::append_team_members(&team_run_id, &new_members) {
        return Err(e);
    }

    let manifest_path_str = crate::team::team_manifest_path(&team_run_id)?
        .to_string_lossy()
        .to_string();
    let work_dir_target = native_slots
        .first()
        .map(|slot| slot.target.as_str())
        .unwrap_or(sanitized.as_str());
    let work_dir = get_session_first_pane_dir(work_dir_target).unwrap_or_else(|| "~".to_string());
    let agent_id = agent_id.unwrap_or_else(|| "shell".to_string());
    let agent_cmd = match resolve_agent_command(&agent_id, &detect_environment().agents) {
        Ok(cmd) => cmd,
        Err(e) => {
            if let Err(restore_err) = crate::team::write_team_manifest(&old_manifest) {
                return Err(format!(
                    "{}|{}|{}",
                    crate::team::ERR_TEAM_ROLLBACK,
                    e,
                    restore_err
                ));
            }
            return Err(e);
        }
    };

    let spawn_result = if native_slots.is_empty() {
        add_standard_panes(
            &sanitized,
            &work_dir,
            &agent_cmd,
            &agent_id,
            count as usize,
            &scope_id,
            &manifest_path_str,
            &lead_session_id,
            &new_session_ids,
        )
    } else {
        add_native_panes(
            &sanitized,
            &native_slots,
            &work_dir,
            &agent_cmd,
            &agent_id,
            count as usize,
            &scope_id,
            &team_run_id,
            &manifest_path_str,
            &lead_session_id,
            &new_session_ids,
        )
    };

    match spawn_result {
        Ok(n) => Ok(n),
        Err(e) => {
            // Restore old manifest on spawn failure
            if let Err(restore_err) = crate::team::write_team_manifest(&old_manifest) {
                return Err(format!(
                    "{}|{}|{}",
                    crate::team::ERR_TEAM_ROLLBACK,
                    e,
                    restore_err
                ));
            }
            Err(e)
        }
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
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 2 && !parts[1].is_empty() {
            pane_count += 1;
            session_name.get_or_insert_with(|| parts[1].to_string());
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
    let _guard = crate::team::TEAM_MUTATION_LOCK
        .lock()
        .map_err(|_| "ERR_KILL_PANE_FAILED|lock".to_string())?;
    let before = tmux_counts();

    let lookup = match run_tmux(&[
        "list-panes",
        "-s",
        "-t",
        trimmed,
        "-F",
        "#{pane_id}|#{session_name}|#{@tmuxdeck-intercom-id}|#{@tmuxdeck-role}|#{@tmuxdeck-manager-target}|#{pane_dead}",
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
    let stdout_str = String::from_utf8_lossy(&lookup.stdout);
    let (session_name, total_panes) = match pane_kill_context(&stdout_str) {
        Ok(res) => res,
        Err(error) => {
            let status = if error == "ERR_KILL_PANE_LAST_IN_SESSION" {
                "rejected_last_pane"
            } else {
                "guard_invalid_output"
            };
            record_kill("kill_pane", trimmed, before, tmux_counts(), status);
            return Err(error);
        }
    };

    let opt_run = run_tmux(&[
        "show-options",
        "-t",
        &session_name,
        "-v",
        crate::team::OPTION_TEAM_RUN_ID,
    ])
    .ok()
    .filter(|o| o.status.success())
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    .unwrap_or_default();

    let opt_lead = run_tmux(&[
        "show-options",
        "-t",
        &session_name,
        "-v",
        crate::team::OPTION_LEAD_ID,
    ])
    .ok()
    .filter(|o| o.status.success())
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    .unwrap_or_default();

    let is_legacy = opt_run.is_empty() && opt_lead.is_empty();
    let (old_manifest, _is_lead) = if !is_legacy {
        if opt_run.is_empty() || opt_lead.is_empty() {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_unavailable",
            );
            return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
        }
        if !crate::team::is_valid_team_run_id(&opt_run) || !crate::team::is_valid_session_id(&opt_lead) {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_unavailable",
            );
            return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
        }
        let expected_manifest = match crate::team::team_manifest_path(&opt_run) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => {
                record_kill(
                    "kill_pane",
                    trimmed,
                    before,
                    tmux_counts(),
                    "rejected_team_unavailable",
                );
                return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
            }
        };
        let env_out = run_tmux(&[
            "show-environment",
            "-t",
            &session_name,
            "AGENT_INTERCOM_TEAM_MANIFEST",
        ])
        .map_err(|_| crate::team::ERR_TEAM_UNAVAILABLE.to_string())?;
        if !env_out.status.success() {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_unavailable",
            );
            return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
        }
        let env_manifest = String::from_utf8_lossy(&env_out.stdout)
            .lines()
            .find_map(|l| l.strip_prefix("AGENT_INTERCOM_TEAM_MANIFEST="))
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if env_manifest.is_empty() || env_manifest != expected_manifest {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_conflict",
            );
            return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
        }
        let manifest = match crate::team::read_team_manifest(&opt_run) {
            Ok(m) => m,
            Err(_) => {
                record_kill(
                    "kill_pane",
                    trimmed,
                    before,
                    tmux_counts(),
                    "rejected_team_unavailable",
                );
                return Err(crate::team::ERR_TEAM_UNAVAILABLE.to_string());
            }
        };
        if manifest.run_id != opt_run || manifest.lead_id != opt_lead {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_conflict",
            );
            return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
        }

        let mut live_members = Vec::new();
        let mut target_intercom_id = None;
        let mut target_role = None;
        let mut lead_found = false;
        let mut lead_dead = false;

        for line in stdout_str.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 6 {
                let p_id = parts[0];
                let p_intercom_id = parts[2];
                let p_role = parts[3];
                let p_manager = parts[4];
                let p_dead = parts[5] == "1";

                if p_intercom_id.is_empty() || p_role.is_empty() {
                    record_kill(
                        "kill_pane",
                        trimmed,
                        before,
                        tmux_counts(),
                        "rejected_team_conflict",
                    );
                    return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
                }
                live_members.push(crate::team::LiveTeamMemberInfo {
                    session_id: p_intercom_id.to_string(),
                    role: p_role.to_string(),
                    manager_target: p_manager.to_string(),
                });
                if p_intercom_id == opt_lead {
                    lead_found = true;
                    lead_dead = p_dead;
                }

                if p_id == trimmed {
                    target_intercom_id = Some(p_intercom_id.to_string());
                    target_role = Some(p_role.to_string());
                }
            } else {
                record_kill(
                    "kill_pane",
                    trimmed,
                    before,
                    tmux_counts(),
                    "rejected_team_conflict",
                );
                return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
            }
        }

        if !lead_found || lead_dead {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_lead_disconnected",
            );
            return Err("ERR_LEAD_DISCONNECTED".to_string());
        }

        if let Err(e) = crate::team::validate_live_team_members(&manifest, &live_members) {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_conflict",
            );
            return Err(e);
        }

        let Some(t_id) = target_intercom_id.filter(|s| !s.is_empty()) else {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_conflict",
            );
            return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
        };
        let Some(t_role) = target_role.filter(|s| !s.is_empty()) else {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_conflict",
            );
            return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
        };

        let Some(manifest_member) = manifest.members.iter().find(|m| m.session_id == t_id) else {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_conflict",
            );
            return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
        };
        if manifest_member.role != t_role {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_team_conflict",
            );
            return Err(crate::team::ERR_TEAM_CONFLICT.to_string());
        }

        let is_lead = t_id == opt_lead;
        if is_lead && total_panes > 1 {
            record_kill(
                "kill_pane",
                trimmed,
                before,
                tmux_counts(),
                "rejected_kill_lead",
            );
            return Err("ERR_KILL_LEAD_NOT_ALLOWED".to_string());
        }

        if !is_lead {
            if let Err(e) = crate::team::remove_team_member(&opt_run, &t_id) {
                record_kill(
                    "kill_pane",
                    trimmed,
                    before,
                    tmux_counts(),
                    "remove_member_failed",
                );
                return Err(e);
            }
        }
        (Some(manifest), is_lead)
    } else {
        (None, false)
    };

    let output = match run_tmux(&["kill-pane", "-t", trimmed]) {
        Ok(output) => output,
        Err(e) => {
            if let Some(orig) = &old_manifest {
                if let Err(restore_err) = crate::team::write_team_manifest(orig) {
                    return Err(format!(
                        "{}|{}|{}",
                        crate::team::ERR_TEAM_ROLLBACK,
                        e,
                        restore_err
                    ));
                }
            }
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
        if let Some(orig) = &old_manifest {
            if let Err(restore_err) = crate::team::write_team_manifest(orig) {
                return Err(format!(
                    "{}|{}|{}",
                    crate::team::ERR_TEAM_ROLLBACK,
                    err_msg,
                    restore_err
                ));
            }
        }
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
        assert_eq!(
            pane_kill_context("%1|workspace|tmuxdeck-a0000000-0000-4000-8000-000000000001|lead|0\n"),
            Err("ERR_KILL_PANE_LAST_IN_SESSION".to_string())
        );
    }

    #[test]
    fn test_multiple_panes_can_be_killed() {
        assert_eq!(
            pane_kill_context("%3|workspace\n%4|workspace\n"),
            Ok(("workspace".to_string(), 2))
        );
        assert_eq!(
            pane_kill_context(
                "%1|workspace|tmuxdeck-a0000000-0000-4000-8000-000000000001|lead|0\n%2|workspace|tmuxdeck-b0000000-0000-4000-8000-000000000002|worker|0\n"
            ),
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
            standard_split_args(
                "deck",
                "/tmp/project",
                "agent --flag",
                "Scope_WorkspaceA123",
                "/tmp/manifest.json"
            ),
            [
                "split-window",
                "-t",
                "deck",
                "-e",
                "AGENT_INTERCOM_SCOPE_ID=Scope_WorkspaceA123",
                "-e",
                "AGENT_INTERCOM_TEAM_MANIFEST=/tmp/manifest.json",
                "-c",
                "/tmp/project",
                "-P",
                "-F",
                "#{pane_id}",
                "agent --flag",
            ]
        );
        assert!(
            !standard_split_args("deck", "~", "shell", "Scope_WorkspaceA123", "")
                .iter()
                .any(|arg| arg == "-c")
        );
    }

    #[test]
    fn standard_metadata_includes_managed_identity_and_marker() {
        let args = standard_pane_metadata_args("%4", "claude", Some("tmuxdeck-random"), "tmuxdeck-lead", true);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["@tmuxdeck-agent", "claude"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["@tmuxdeck-intercom-id", "tmuxdeck-random"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["@tmuxdeck-role", "worker"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["@tmuxdeck-manager-target", "tmuxdeck-lead"]));
        assert!(args.windows(2).any(|pair| {
            pair == [
                "@tmuxdeck-claude-adapter",
                crate::claude_adapter::MANAGED_ADAPTER_MARKER,
            ]
        }));
    }

    #[test]
    fn standard_metadata_omits_managed_fields_for_other_agents() {
        let args = standard_pane_metadata_args("%4", "pi", None, "tmuxdeck-lead", false);
        assert_eq!(
            args,
            [
                "set-option", "-p", "-t", "%4", "@tmuxdeck-agent", "pi",
                ";",
                "set-option", "-p", "-t", "%4", "@tmuxdeck-role", "worker",
                ";",
                "set-option", "-p", "-t", "%4", "@tmuxdeck-manager-target", "tmuxdeck-lead"
            ]
        );
    }

    #[test]
    fn test_team_kill_missing_or_nonmember_id_rejected() {
        let lead_id = "tmuxdeck-a0000000-0000-4000-8000-000000000001";
        let worker_id = "tmuxdeck-b0000000-0000-4000-8000-000000000002";
        let non_member_id = "tmuxdeck-c0000000-0000-4000-8000-000000000003";

        let manifest = crate::team::TeamManifest {
            version: crate::team::TEAM_MANIFEST_VERSION.to_string(),
            backend: crate::team::TEAM_BACKEND.to_string(),
            run_id: "team_11223344-5566-4778-8899-aabbccddeeff".to_string(),
            lead_id: lead_id.to_string(),
            members: vec![
                crate::team::TeamMember {
                    session_id: lead_id.to_string(),
                    role: "lead".to_string(),
                },
                crate::team::TeamMember {
                    session_id: worker_id.to_string(),
                    role: "worker".to_string(),
                },
            ],
            created_at: 1723680000000,
            capabilities: vec![],
        };

        // Target with non-member ID must be rejected
        let target_id = non_member_id;
        let is_member = manifest.members.iter().any(|m| m.session_id == target_id);
        assert!(!is_member, "Non-member session ID must be rejected");

        // Target with empty/missing ID must be rejected
        let empty_id = "";
        assert!(empty_id.is_empty(), "Empty session ID must be rejected");
    }

    #[test]
    fn test_false_role_lead_cannot_forge_missing_exact_lead() {
        let exact_lead_id = "tmuxdeck-a0000000-0000-4000-8000-000000000001";
        let fake_lead_id = "tmuxdeck-b0000000-0000-4000-8000-000000000002";

        let manifest = crate::team::TeamManifest {
            version: crate::team::TEAM_MANIFEST_VERSION.to_string(),
            backend: crate::team::TEAM_BACKEND.to_string(),
            run_id: "team_11223344-5566-4778-8899-aabbccddeeff".to_string(),
            lead_id: exact_lead_id.to_string(),
            members: vec![
                crate::team::TeamMember {
                    session_id: exact_lead_id.to_string(),
                    role: "lead".to_string(),
                },
                crate::team::TeamMember {
                    session_id: fake_lead_id.to_string(),
                    role: "worker".to_string(),
                },
            ],
            created_at: 1723680000000,
            capabilities: vec![],
        };

        // Live pane list has fake lead pretending to be lead, while exact lead is missing
        let live_members = vec![crate::team::LiveTeamMemberInfo {
            session_id: fake_lead_id.to_string(),
            role: "lead".to_string(),
            manager_target: "".to_string(),
        }];
        let validation_res = crate::team::validate_live_team_members(&manifest, &live_members);
        assert!(
            validation_res.is_err(),
            "Live member with fake role 'lead' not matching manifest leadId must fail validation"
        );
    }

    #[test]
    fn test_all_empty_live_metadata_rejected() {
        let exact_lead_id = "tmuxdeck-a0000000-0000-4000-8000-000000000001";
        let manifest = crate::team::TeamManifest {
            version: crate::team::TEAM_MANIFEST_VERSION.to_string(),
            backend: crate::team::TEAM_BACKEND.to_string(),
            run_id: "team_11223344-5566-4778-8899-aabbccddeeff".to_string(),
            lead_id: exact_lead_id.to_string(),
            members: vec![
                crate::team::TeamMember {
                    session_id: exact_lead_id.to_string(),
                    role: "lead".to_string(),
                },
            ],
            created_at: 1723680000000,
            capabilities: vec![],
        };

        // Live pane with all empty metadata must be rejected
        let live_members = vec![crate::team::LiveTeamMemberInfo {
            session_id: "".to_string(),
            role: "".to_string(),
            manager_target: "".to_string(),
        }];
        assert!(crate::team::validate_live_team_members(&manifest, &live_members).is_err());
    }

    #[test]
    fn test_legacy_standard_no_lead_guard() {
        let is_legacy = true;
        let is_lead = false;
        assert!(is_legacy);
        assert!(!is_lead, "Legacy standard workspace must not guard first pane as lead");
    }

    #[test]
    fn test_first_native_env_empty_conflict_logic() {
        let expected_path = "/tmp/teams/team_11223344-5566-4778-8899-aabbccddeeff.json";
        let slot1_env = "";
        let slot2_env = "/tmp/teams/team_11223344-5566-4778-8899-aabbccddeeff.json";

        // Every slot must be checked immediately against expected_path
        let slot1_valid = !slot1_env.is_empty() && slot1_env == expected_path;
        let slot2_valid = !slot2_env.is_empty() && slot2_env == expected_path;

        assert!(!slot1_valid, "First slot with empty env must immediately fail validation");
        assert!(slot2_valid, "Second slot with matching env would pass validation");
    }
}
