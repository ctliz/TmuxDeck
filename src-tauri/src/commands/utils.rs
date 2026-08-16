use crate::registry::detect_environment;
use std::process::Command;

pub(crate) const PROCESS_SCRUB_ENV_VARS: &[&str] = &[
    "PI_SESSION_ID",
    "PI_SESSION_FILE",
    "PI_INTERCOM_SESSION_ID",
    "PI_SUBAGENT_INTERCOM_SESSION_NAME",
    "PI_CODING_AGENT",
    "PI_MODEL",
    "PI_PROVIDER",
    "PI_REASONING_LEVEL",
    "CLAUDE_INTERCOM_SESSION_ID",
    "CLAUDE_INTERCOM_NAME",
    "CLAUDE_INTERCOM_MODEL",
    "CLAUDE_PEER_ID",
    "CLAUDE_PEER_NAME",
    "CLAUDE_INTERCOM_ID",
    "CLAUDE_INTERCOM_WORKER_NAME",
    "CODEX_INTERCOM_SESSION_ID",
    "CODEX_INTERCOM_NAME",
    "CODEX_INTERCOM_MODEL",
    "CODEX_PEER_ID",
    "CODEX_PEER_NAME",
    "CODEX_INTERCOM_BRIDGE_NAME",
    "OPENCODE_INTERCOM_SESSION_ID",
    "OPENCODE_INTERCOM_NAME",
    "OPENCODE_INTERCOM_MODEL",
    "OPENCODE_PEER_ID",
    "OPENCODE_PEER_NAME",
    "OPENCODE_SESSION_ID",
    "AGENT_INTERCOM_SESSION_ID",
    "AGENT_INTERCOM_SESSION_NAME",
    "AGENT_INTERCOM_NAME",
    "AGENT_INTERCOM_MANAGER_TARGET",
    "AGENT_INTERCOM_MANAGER_SESSION_ID",
    "AGENT_INTERCOM_ROLE",
    "AGENT_INTERCOM_WORKER_ID",
    "AGENT_INTERCOM_RUN_ID",
    "AGENT_INTERCOM_OWNED",
    "AGENT_INTERCOM_SYSTEMD_UNIT",
    "AGENT_INTERCOM_FRESH",
    "AGENT_INTERCOM_WORKER_INCARNATION_ID",
    "AGENT_INTERCOM_WORKER_GENERATION",
    "AGENT_INTERCOM_PARTICIPANT_ID",
    "AGENT_INTERCOM_BINDING_EPOCH",
    "AGENT_INTERCOM_MANAGER_CONTEXT",
    "AGENT_INTERCOM_ORCHESTRATOR_DISABLED",
    "AGENT_INTERCOM_BOSS_RUN_ID",
    "AGENT_INTERCOM_BOSS_ROLE",
    "AGENT_INTERCOM_BOSS_CONTROLLER_TARGET",
    "AGENT_INTERCOM_BOSS_MANAGER_TARGET",
    "AGENT_INTERCOM_BOSS_TEAM_TARGETS",
    "AGENT_INTERCOM_BOSS_VISIBILITY",
    "AGENT_INTERCOM_TEAM_MANIFEST",
];

pub(crate) const SESSION_SCRUB_ENV_VARS: &[&str] = &[
    "PI_SESSION_ID",
    "PI_SESSION_FILE",
    "PI_INTERCOM_SESSION_ID",
    "PI_SUBAGENT_INTERCOM_SESSION_NAME",
    "PI_CODING_AGENT",
    "PI_MODEL",
    "PI_PROVIDER",
    "PI_REASONING_LEVEL",
    "CLAUDE_INTERCOM_SESSION_ID",
    "CLAUDE_INTERCOM_NAME",
    "CLAUDE_INTERCOM_MODEL",
    "CLAUDE_PEER_ID",
    "CLAUDE_PEER_NAME",
    "CLAUDE_INTERCOM_ID",
    "CLAUDE_INTERCOM_WORKER_NAME",
    "CODEX_INTERCOM_SESSION_ID",
    "CODEX_INTERCOM_NAME",
    "CODEX_INTERCOM_MODEL",
    "CODEX_PEER_ID",
    "CODEX_PEER_NAME",
    "CODEX_INTERCOM_BRIDGE_NAME",
    "OPENCODE_INTERCOM_SESSION_ID",
    "OPENCODE_INTERCOM_NAME",
    "OPENCODE_INTERCOM_MODEL",
    "OPENCODE_PEER_ID",
    "OPENCODE_PEER_NAME",
    "OPENCODE_SESSION_ID",
    "AGENT_INTERCOM_SESSION_ID",
    "AGENT_INTERCOM_SESSION_NAME",
    "AGENT_INTERCOM_NAME",
    "AGENT_INTERCOM_MANAGER_TARGET",
    "AGENT_INTERCOM_MANAGER_SESSION_ID",
    "AGENT_INTERCOM_ROLE",
    "AGENT_INTERCOM_WORKER_ID",
    "AGENT_INTERCOM_RUN_ID",
    "AGENT_INTERCOM_OWNED",
    "AGENT_INTERCOM_SYSTEMD_UNIT",
    "AGENT_INTERCOM_FRESH",
    "AGENT_INTERCOM_WORKER_INCARNATION_ID",
    "AGENT_INTERCOM_WORKER_GENERATION",
    "AGENT_INTERCOM_PARTICIPANT_ID",
    "AGENT_INTERCOM_BINDING_EPOCH",
    "AGENT_INTERCOM_MANAGER_CONTEXT",
    "AGENT_INTERCOM_ORCHESTRATOR_DISABLED",
    "AGENT_INTERCOM_BOSS_RUN_ID",
    "AGENT_INTERCOM_BOSS_ROLE",
    "AGENT_INTERCOM_BOSS_CONTROLLER_TARGET",
    "AGENT_INTERCOM_BOSS_MANAGER_TARGET",
    "AGENT_INTERCOM_BOSS_TEAM_TARGETS",
    "AGENT_INTERCOM_BOSS_VISIBILITY",
];

#[allow(dead_code)]
pub(crate) const AGENT_IDENTITY_ENV_VARS: &[&str] = PROCESS_SCRUB_ENV_VARS;

pub(crate) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn expand_tilde(path: &str) -> String {
    if path == "~" || path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            if path == "~" {
                return home;
            } else {
                return format!("{}{}", home, &path[1..]);
            }
        }
    }
    path.to_string()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn build_augmented_path() -> String {
    let mut paths: Vec<String> = Vec::new();

    if let Ok(current_path) = std::env::var("PATH") {
        for p in std::env::split_paths(&current_path) {
            let path_str = expand_tilde(&p.to_string_lossy());
            if !path_str.is_empty() && !paths.iter().any(|existing| existing == &path_str) {
                paths.push(path_str);
            }
        }
    }

    let mut candidates = vec![
        "/opt/homebrew/bin".to_string(),
        "/opt/homebrew/sbin".to_string(),
        "/usr/local/bin".to_string(),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
        "/home/linuxbrew/.linuxbrew/bin".to_string(),
    ];

    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{}/.cargo/bin", home));
        candidates.push(format!("{}/.local/bin", home));
        candidates.push(format!("{}/.bun/bin", home));

        let nvm_dir = std::path::Path::new(&home)
            .join(".nvm")
            .join("versions")
            .join("node");
        if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
            for entry in entries.flatten() {
                let bin_path = entry.path().join("bin");
                if bin_path.exists() {
                    candidates.push(bin_path.to_string_lossy().to_string());
                }
            }
        }
    }

    for c in candidates {
        if std::path::Path::new(&c).exists() && !paths.iter().any(|existing| existing == &c) {
            paths.push(c);
        }
    }

    paths.join(":")
}

#[cfg(target_os = "windows")]
pub(crate) fn build_augmented_path() -> String {
    // Windows hosts run tmux inside WSL. Do not inherit Windows host PATH (which uses semicolon and C:\ drive letters).
    // Construct standard WSL/Linux environment PATH for execution inside WSL /bin/sh.
    let candidates = [
        "/usr/local/sbin",
        "/usr/local/bin",
        "/usr/sbin",
        "/usr/bin",
        "/sbin",
        "/bin",
        "/usr/games",
        "/usr/local/games",
        "/home/linuxbrew/.linuxbrew/bin",
    ];
    candidates.join(":")
}

pub(crate) fn extract_parent_bin_dir(command: &str) -> Option<String> {
    let first_word = command.trim().split_whitespace().next()?;
    if first_word.starts_with('/') {
        let path = std::path::Path::new(first_word);
        if let Some(parent) = path.parent() {
            let parent_str = parent.to_string_lossy().to_string();
            if !parent_str.is_empty() && parent_str != "/" {
                return Some(parent_str);
            }
        }
    }
    None
}

pub(crate) fn build_augmented_path_for_command(command: &str) -> String {
    let base_path = build_augmented_path();
    if let Some(parent_dir) = extract_parent_bin_dir(command) {
        let parts: Vec<&str> = base_path.split(':').collect();
        if !parts.contains(&parent_dir.as_str()) {
            return format!("{}:{}", parent_dir, base_path);
        } else if parts.first() != Some(&parent_dir.as_str()) {
            let mut new_parts = vec![parent_dir.as_str()];
            for p in parts {
                if p != parent_dir.as_str() {
                    new_parts.push(p);
                }
            }
            return new_parts.join(":");
        }
    }
    base_path
}

pub(crate) fn isolated_agent_command_with_team_env(
    command: &str,
    return_to_shell: bool,
    team_envs: &[(String, String)],
) -> String {
    let unset = PROCESS_SCRUB_ENV_VARS
        .iter()
        .map(|name| format!("-u {}", name))
        .collect::<Vec<_>>()
        .join(" ");
    let term_program = std::env::var("TERM_PROGRAM")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "ghostty".to_string());
    let mut env_pairs = vec![
        ("COLORTERM".to_string(), "truecolor".to_string()),
        ("TERM_PROGRAM".to_string(), term_program),
    ];
    for (k, v) in team_envs {
        env_pairs.push((k.clone(), v.clone()));
    }
    let set_envs = env_pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, shell_single_quote(v)))
        .collect::<Vec<_>>()
        .join(" ");
    let path = build_augmented_path_for_command(command);
    let pane_command = if return_to_shell {
        format!(
            "{}; exit_code=$?; if [ \"$exit_code\" -eq 0 ] || [ \"$exit_code\" -eq 130 ]; then tmux set-option -p -t \"$TMUX_PANE\" @tmuxdeck-agent shell 2>/dev/null || true; exec \"${{SHELL:-/bin/sh}}\"; else exit \"$exit_code\"; fi",
            command
        )
    } else {
        command.to_string()
    };
    format!(
        "env {} {} PATH={} /bin/sh -c {}",
        unset,
        set_envs,
        shell_single_quote(&path),
        shell_single_quote(&pane_command)
    )
}

#[allow(dead_code)]
pub(crate) fn isolated_agent_command(command: &str, return_to_shell: bool) -> String {
    isolated_agent_command_with_team_env(command, return_to_shell, &[])
}

#[tauri::command]
pub fn use_standard_claude() -> Result<(), String> {
    if crate::registry::find_agent_binary("claude").is_none() {
        return Err("ERR_STANDARD_CLAUDE_UNAVAILABLE".to_string());
    }
    let mut config = crate::config::load_config();
    config.use_standard_claude = true;
    crate::config::save_config(config)?;
    crate::claude_adapter::invalidate_managed_claude_health_cache();
    Ok(())
}

pub(crate) fn append_identity_env_clears(args: &mut Vec<String>, target: &str) {
    for name in SESSION_SCRUB_ENV_VARS {
        args.extend([
            ";".to_string(),
            "set-environment".to_string(),
            "-u".to_string(),
            "-t".to_string(),
            target.to_string(),
            (*name).to_string(),
        ]);
    }
}

pub(crate) fn terminal_capability_envs(terminal_id: Option<&str>) -> Vec<String> {
    let term_program = std::env::var("TERM_PROGRAM")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| terminal_id.unwrap_or("ghostty").to_string());
    vec![
        "-e".to_string(),
        "COLORTERM=truecolor".to_string(),
        "-e".to_string(),
        format!("TERM_PROGRAM={}", term_program),
    ]
}

pub(crate) fn panel_agent_command(
    agent_id: &str,
    command: &str,
    bypass_permissions: bool,
) -> String {
    if !bypass_permissions {
        return command.to_string();
    }
    let flag = match agent_id {
        "claude" => "--dangerously-skip-permissions",
        "codex" => "--dangerously-bypass-approvals-and-sandbox",
        "opencode" => "--auto",
        // agy/Gemini are intentionally deferred to v1.15.0.
        _ => return command.to_string(),
    };
    if shell_words(command).iter().any(|token| token == flag) {
        command.to_string()
    } else if command.trim().is_empty() {
        command.to_string()
    } else {
        format!("{} {}", command.trim_end(), flag)
    }
}

fn shell_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in command.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                word.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(ch);
        }
    }
    if escaped {
        word.push('\\');
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

pub(crate) fn session_terminal_options(target: &str) -> Vec<String> {
    vec![
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        target.to_string(),
        "focus-events".to_string(),
        "on".to_string(),
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        target.to_string(),
        "extended-keys".to_string(),
        "on".to_string(),
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        target.to_string(),
        "default-terminal".to_string(),
        "tmux-256color".to_string(),
        ";".to_string(),
        "set-option".to_string(),
        "-t".to_string(),
        target.to_string(),
        "-a".to_string(),
        "terminal-overrides".to_string(),
        ",*:RGB".to_string(),
    ]
}

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
        if let Ok(out) = Command::new("wsl.exe")
            .args(["wslpath", "-u", &path])
            .output()
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_command_clears_known_identity_but_keeps_broker_dir() {
        let command = isolated_agent_command("pi --model test", false);
        assert!(command.contains("env -u PI_SESSION_ID"));
        assert!(command.contains("-u PI_INTERCOM_SESSION_ID"));
        assert!(command.contains("PATH="));
        assert!(!command.contains("-u PI_CODING_AGENT_DIR"));
        assert!(command.contains("/bin/sh -c 'pi --model test'"));
    }

    #[test]
    fn test_orchestrator_and_boss_vars_are_scrubbed() {
        let command = isolated_agent_command("claude", false);
        for required_var in [
            "AGENT_INTERCOM_WORKER_ID",
            "AGENT_INTERCOM_RUN_ID",
            "AGENT_INTERCOM_OWNED",
            "AGENT_INTERCOM_SYSTEMD_UNIT",
            "AGENT_INTERCOM_FRESH",
            "AGENT_INTERCOM_WORKER_INCARNATION_ID",
            "AGENT_INTERCOM_WORKER_GENERATION",
            "AGENT_INTERCOM_PARTICIPANT_ID",
            "AGENT_INTERCOM_BINDING_EPOCH",
            "AGENT_INTERCOM_MANAGER_CONTEXT",
            "AGENT_INTERCOM_ORCHESTRATOR_DISABLED",
            "AGENT_INTERCOM_BOSS_RUN_ID",
            "AGENT_INTERCOM_BOSS_ROLE",
            "AGENT_INTERCOM_BOSS_CONTROLLER_TARGET",
            "AGENT_INTERCOM_BOSS_MANAGER_TARGET",
            "AGENT_INTERCOM_BOSS_TEAM_TARGETS",
            "AGENT_INTERCOM_BOSS_VISIBILITY",
        ] {
            assert!(
                command.contains(&format!("-u {}", required_var)),
                "Process scrub must include {}",
                required_var
            );
            assert!(
                SESSION_SCRUB_ENV_VARS.contains(&required_var),
                "Session scrub must include {}",
                required_var
            );
        }
        // AGENT_INTERCOM_SCOPE_ID must NEVER be scrubbed
        assert!(!command.contains("-u AGENT_INTERCOM_SCOPE_ID"));
        assert!(!SESSION_SCRUB_ENV_VARS.contains(&"AGENT_INTERCOM_SCOPE_ID"));
    }

    #[test]
    fn test_build_augmented_path_format() {
        let path = build_augmented_path();
        assert!(!path.is_empty());
        assert!(!path.contains(';'));
        assert!(!path.contains('~'));
        #[cfg(not(target_os = "windows"))]
        {
            assert!(path.contains("/usr/bin") || path.contains("/bin"));
        }
        #[cfg(target_os = "windows")]
        {
            assert!(path.contains("/usr/bin"));
            assert!(!path.contains("C:\\"));
        }
    }

    #[test]
    fn test_isolated_command_prepends_absolute_nvm_agent_parent_bin_dir() {
        let nvm_cmd = "/home/dev/.nvm/versions/node/v24.14.0/bin/pi --name test-session";
        let command = isolated_agent_command(nvm_cmd, false);
        assert!(command.contains("PATH='/home/dev/.nvm/versions/node/v24.14.0/bin:"));
        assert!(command.contains(
            "/bin/sh -c '/home/dev/.nvm/versions/node/v24.14.0/bin/pi --name test-session'"
        ));
    }

    #[test]
    fn isolated_command_preserves_custom_shell_command() {
        let command = isolated_agent_command("custom-agent --name 'A B' && echo done", false);
        assert!(command.ends_with("/bin/sh -c 'custom-agent --name '\\''A B'\\'' && echo done'"));
    }

    #[test]
    fn agent_command_returns_to_shell_after_normal_exit_or_ctrl_c() {
        let command = isolated_agent_command("pi", true);
        assert!(command.contains("exit_code=$?"));
        assert!(command.contains(r#"[ "$exit_code" -eq 0 ]"#));
        assert!(command.contains(r#"[ "$exit_code" -eq 130 ]"#));
        assert!(command.contains("@tmuxdeck-agent shell"));
        assert!(command.contains(r#"exec "${SHELL:-/bin/sh}""#));
        assert!(command.contains(r#"else exit "$exit_code""#));
    }

    #[test]
    fn test_panel_agent_bypass_is_scoped_and_token_aware() {
        assert_eq!(
            panel_agent_command("claude", "claude", true),
            "claude --dangerously-skip-permissions"
        );
        assert_eq!(
            panel_agent_command("codex", "codex", true),
            "codex --dangerously-bypass-approvals-and-sandbox"
        );
        assert_eq!(
            panel_agent_command("opencode", "opencode", true),
            "opencode --auto"
        );
        assert_eq!(panel_agent_command("pi", "pi", true), "pi");
        assert_eq!(panel_agent_command("agy", "agy", true), "agy");
        assert_eq!(
            panel_agent_command("custom", "claude --custom", true),
            "claude --custom"
        );
        assert_eq!(
            panel_agent_command("claude", "claude --dangerously-skip-permissions", true),
            "claude --dangerously-skip-permissions"
        );
        assert_eq!(
            panel_agent_command(
                "claude",
                "claude --dangerously-skip-permissions-extra",
                true
            ),
            "claude --dangerously-skip-permissions-extra --dangerously-skip-permissions"
        );
        assert_eq!(
            panel_agent_command("codex", "codex --auto", false),
            "codex --auto"
        );
    }

    #[test]
    fn test_terminal_capability_envs_and_options() {
        let envs = terminal_capability_envs(Some("ghostty"));
        assert_eq!(envs[0], "-e");
        assert_eq!(envs[1], "COLORTERM=truecolor");
        assert_eq!(envs[2], "-e");
        assert!(envs[3].starts_with("TERM_PROGRAM="));

        let opts = session_terminal_options("sess-1");
        assert!(opts.contains(&"focus-events".to_string()));
        assert!(opts.contains(&"extended-keys".to_string()));
        assert!(opts.contains(&"default-terminal".to_string()));
        assert!(opts.contains(&"tmux-256color".to_string()));
        assert!(opts.contains(&"terminal-overrides".to_string()));
        assert!(opts.contains(&",*:RGB".to_string()));
    }
}
