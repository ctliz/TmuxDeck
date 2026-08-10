use serde::{Deserialize, Serialize};
use std::process::Command;
use crate::config::load_config;
use crate::tmux::check_tmux_installed;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub icon_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Environment {
    pub tmux: Option<String>,
    pub terminals: Vec<ToolInfo>,
    pub agents: Vec<ToolInfo>,
}

pub fn find_app_icon(app_path_str: &str) -> Option<String> {
    let res = std::path::Path::new(app_path_str).join("Contents/Resources");
    let entries = std::fs::read_dir(res).ok()?;

    let mut icns_files: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().map(|ext| ext == "icns").unwrap_or(false) {
            icns_files.push(p);
        }
    }

    if icns_files.is_empty() {
        return None;
    }

    let app_stem = std::path::Path::new(app_path_str)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    for p in &icns_files {
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
        if stem == "appicon" || stem == "icon" || stem == app_stem {
            return Some(p.to_string_lossy().to_string());
        }
    }

    Some(icns_files[0].to_string_lossy().to_string())
}

pub fn find_binary(bin_name: &str, candidate_paths: &[&str]) -> Option<String> {
    for path in candidate_paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    let cmd = if cfg!(target_os = "windows") { "where.exe" } else { "which" };
    if let Ok(output) = Command::new(cmd).arg(bin_name).output() {
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().lines().next().unwrap_or("").to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
pub fn find_agent_binary(bin: &str) -> Option<String> {
    if let Ok(out) = Command::new("wsl.exe").args(["--", "which", bin]).output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    let nvm_cmd = format!("ls ~/.nvm/versions/node/*/bin/{} 2>/dev/null | head -n 1", bin);
    if let Ok(out) = Command::new("wsl.exe").args(["--", "bash", "-c", &nvm_cmd]).output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub fn find_agent_binary(bin: &str) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidate_paths = [
        format!("/opt/homebrew/bin/{}", bin),
        format!("/usr/local/bin/{}", bin),
        format!("/usr/bin/{}", bin),
        format!("{}/.cargo/bin/{}", home, bin),
    ];
    for p in &candidate_paths {
        if std::path::Path::new(p).exists() {
            return Some(p.clone());
        }
    }
    let nvm_dir = format!("{}/.nvm/versions/node", home);
    if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
        for entry in entries.flatten() {
            let path = entry.path().join("bin").join(bin);
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    if let Ok(output) = Command::new("which").arg(bin).output() {
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    None
}

#[tauri::command]
pub fn detect_environment() -> Environment {
    let tmux = check_tmux_installed();

    let mut installed_terminals = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let known_windows_terminals = vec![
            ("wt", "Windows Terminal", "wt.exe"),
            ("cmd", "Command Prompt", "cmd.exe"),
            ("powershell", "PowerShell", "powershell.exe"),
        ];
        for (id, name, bin) in known_windows_terminals {
            if id == "cmd" || id == "powershell" || find_binary(bin, &[]).is_some() {
                installed_terminals.push(ToolInfo {
                    id: id.to_string(),
                    name: name.to_string(),
                    path: bin.to_string(),
                    icon_path: None,
                });
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let known_terminals = vec![
            ("ghostty", "Ghostty", vec!["/Applications/Ghostty.app"]),
            ("iterm2", "iTerm2", vec!["/Applications/iTerm.app"]),
            (
                "terminal",
                "terminal.system",
                vec![
                    "/System/Applications/Utilities/Terminal.app",
                    "/Applications/Utilities/Terminal.app",
                ],
            ),
            ("wezterm", "WezTerm", vec!["/Applications/WezTerm.app"]),
            ("kitty", "kitty", vec!["/Applications/kitty.app"]),
            ("alacritty", "Alacritty", vec!["/Applications/Alacritty.app"]),
        ];

        for (id, name, paths) in known_terminals {
            let mut found_path: Option<String> = None;
            for p in paths {
                if std::path::Path::new(p).exists() {
                    found_path = Some(p.to_string());
                    break;
                }
            }
            if id == "terminal" && found_path.is_none() {
                found_path = Some("/System/Applications/Utilities/Terminal.app".to_string());
            }
            if let Some(path) = found_path {
                let icon_path = find_app_icon(&path);
                installed_terminals.push(ToolInfo {
                    id: id.to_string(),
                    name: name.to_string(),
                    path,
                    icon_path,
                });
            }
        }
    }

    // Agent Registry
    let known_agents = vec![
        ("pi", "Pi", "pi"),
        ("claude", "Claude Code", "claude"),
        ("codex", "Codex", "codex"),
        ("opencode", "OpenCode", "opencode"),
        ("gemini", "Gemini CLI", "gemini"),
        ("aider", "Aider", "aider"),
    ];

    let mut installed_agents = Vec::new();
    for (id, name, bin) in known_agents {
        if let Some(p) = find_agent_binary(bin) {
            installed_agents.push(ToolInfo {
                id: id.to_string(),
                name: name.to_string(),
                path: p,
                icon_path: None,
            });
        }
    }

    // Custom Agent Support
    let cfg = load_config();
    if let Some(custom) = cfg.custom_agent {
        if !custom.command.trim().is_empty() {
            installed_agents.push(ToolInfo {
                id: "custom".to_string(),
                name: if custom.name.trim().is_empty() { "agent.custom".to_string() } else { custom.name },
                path: custom.command,
                icon_path: None,
            });
        }
    }

    // Plain Shell fallback
    let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    installed_agents.push(ToolInfo {
        id: "shell".to_string(),
        name: "agent.shell".to_string(),
        path: shell_path,
        icon_path: None,
    });

    Environment {
        tmux,
        terminals: installed_terminals,
        agents: installed_agents,
    }
}
