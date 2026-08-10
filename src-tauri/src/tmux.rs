use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::registry::find_binary;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TmuxPane {
    pub id: String,
    pub command: String,
    pub active: bool,
}

#[cfg(target_os = "windows")]
pub fn check_tmux_installed() -> Option<String> {
    if let Ok(out) = Command::new("wsl.exe").args(["--", "tmux", "-V"]).output() {
        if out.status.success() {
            return Some("wsl.exe -- tmux".to_string());
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub fn check_tmux_installed() -> Option<String> {
    find_binary(
        "tmux",
        &[
            "/opt/homebrew/bin/tmux",
            "/usr/local/bin/tmux",
            "/usr/bin/tmux",
        ],
    )
}

#[cfg(target_os = "windows")]
pub fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("wsl.exe").arg("--").arg("tmux").args(args).output()
}

#[cfg(not(target_os = "windows"))]
pub fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    let tmux = check_tmux_installed().unwrap_or_else(|| "tmux".to_string());
    Command::new(tmux).args(args).output()
}

pub fn is_no_server_err(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("no server running")
        || lower.contains("error connecting")
        || lower.contains("failed to connect")
        || (lower.contains("no such file or directory") && (lower.contains("tmux") || lower.contains("socket") || lower.contains("/tmp/")))
}

pub fn sanitize_session_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("ERR_NAME_EMPTY".to_string());
    }

    let sanitized: String = trimmed
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '-' })
        .collect();

    let mut result = String::new();
    let mut last_dash = false;
    for c in sanitized.chars() {
        if c == '-' {
            if !last_dash {
                result.push(c);
                last_dash = true;
            }
        } else {
            result.push(c);
            last_dash = false;
        }
    }
    let result = result.trim_matches('-').to_string();

    if result.is_empty() {
        return Err("ERR_NAME_INVALID".to_string());
    }
    if result.len() > 60 {
        return Ok(result[..60].to_string());
    }
    Ok(result)
}

pub fn validate_pane_id(pane_id: &str) -> bool {
    let trimmed = pane_id.trim();
    if !trimmed.starts_with('%') || trimmed.len() < 2 {
        return false;
    }
    trimmed[1..].chars().all(|c| c.is_ascii_digit())
}

pub fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if (next >= 'a' && next <= 'z') || (next >= 'A' && next <= 'Z') {
                        break;
                    }
                }
            }
            continue;
        }
        result.push(c);
    }
    result
}

pub fn is_session_attached(session_name: &str) -> bool {
    if let Ok(out) = run_tmux(&["list-sessions", "-F", "#{session_attached}", "-t", session_name]) {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            return stdout.trim() == "1";
        }
    }
    false
}

pub fn get_session_first_pane_dir(session_name: &str) -> Option<String> {
    let output = run_tmux(&["list-panes", "-t", session_name, "-F", "#{pane_current_path}"]);
    if let Ok(out) = output {
        if out.status.success() {
            let line = String::from_utf8_lossy(&out.stdout).trim().lines().next().unwrap_or("").to_string();
            if !line.is_empty() {
                return Some(line);
            }
        }
    }
    None
}

pub fn get_session_panes(session_name: &str) -> Vec<TmuxPane> {
    let output = run_tmux(&[
        "list-panes",
        "-s",
        "-t",
        session_name,
        "-F",
        "#{pane_id}|#{pane_current_command}|#{pane_active}",
    ]);

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut panes = Vec::new();
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 3 {
                    panes.push(TmuxPane {
                        id: parts[0].to_string(),
                        command: parts[1].to_string(),
                        active: parts[2] == "1",
                    });
                }
            }
            return panes;
        }
    }
    Vec::new()
}
