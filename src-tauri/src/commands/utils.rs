use std::process::Command;
use crate::registry::detect_environment;

pub(crate) const AGENT_IDENTITY_ENV_VARS: &[&str] = &[
    "PI_SESSION_ID",
    "PI_SESSION_FILE",
    "PI_INTERCOM_SESSION_ID",
    "PI_CODING_AGENT",
    "PI_MODEL",
    "PI_PROVIDER",
    "PI_REASONING_LEVEL",
    "OPENCODE_INTERCOM_SESSION_ID",
    "OPENCODE_INTERCOM_NAME",
    "OPENCODE_INTERCOM_MODEL",
    "CODEX_INTERCOM_SESSION_ID",
    "CODEX_INTERCOM_NAME",
    "CODEX_INTERCOM_MODEL",
    "CLAUDE_INTERCOM_SESSION_ID",
    "CLAUDE_INTERCOM_NAME",
    "CLAUDE_INTERCOM_MODEL",
];

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

        let nvm_dir = std::path::Path::new(&home).join(".nvm").join("versions").join("node");
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

pub(crate) fn isolated_agent_command(command: &str) -> String {
    let unset = AGENT_IDENTITY_ENV_VARS
        .iter()
        .map(|name| format!("-u {}", name))
        .collect::<Vec<_>>()
        .join(" ");
    let path = build_augmented_path_for_command(command);
    format!(
        "env {} PATH={} /bin/sh -c {}",
        unset,
        shell_single_quote(&path),
        shell_single_quote(command)
    )
}

pub(crate) fn append_identity_env_clears(args: &mut Vec<String>, target: &str) {
    for name in AGENT_IDENTITY_ENV_VARS {
        args.extend([
            ";".to_string(),
            "set-environment".to_string(),
            "-t".to_string(),
            target.to_string(),
            (*name).to_string(),
            String::new(),
        ]);
    }
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
        if let Ok(out) = Command::new("wsl.exe").args(["wslpath", "-u", &path]).output() {
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
        let command = isolated_agent_command("pi --model test");
        assert!(command.contains("env -u PI_SESSION_ID"));
        assert!(command.contains("-u PI_INTERCOM_SESSION_ID"));
        assert!(command.contains("PATH="));
        assert!(!command.contains("-u PI_CODING_AGENT_DIR"));
        assert!(command.contains("/bin/sh -c 'pi --model test'"));
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
        let command = isolated_agent_command(nvm_cmd);
        assert!(command.contains("PATH='/home/dev/.nvm/versions/node/v24.14.0/bin:"));
        assert!(command.contains("/bin/sh -c '/home/dev/.nvm/versions/node/v24.14.0/bin/pi --name test-session'"));
    }

    #[test]
    fn isolated_command_preserves_custom_shell_command() {
        let command = isolated_agent_command("custom-agent --name 'A B' && echo done");
        assert!(command.ends_with(
            "/bin/sh -c 'custom-agent --name '\\''A B'\\'' && echo done'"
        ));
    }
}
