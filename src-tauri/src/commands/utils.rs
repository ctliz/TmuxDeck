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

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn isolated_agent_command(command: &str) -> String {
    let unset = AGENT_IDENTITY_ENV_VARS
        .iter()
        .map(|name| format!("-u {}", name))
        .collect::<Vec<_>>()
        .join(" ");
    format!("env {} /bin/sh -c {}", unset, shell_single_quote(command))
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
        assert!(!command.contains("-u PI_CODING_AGENT_DIR"));
        assert!(command.ends_with("/bin/sh -c 'pi --model test'"));
    }

    #[test]
    fn isolated_command_preserves_custom_shell_command() {
        let command = isolated_agent_command("custom-agent --name 'A B' && echo done");
        assert!(command.ends_with(
            "/bin/sh -c 'custom-agent --name '\\''A B'\\'' && echo done'"
        ));
    }
}
