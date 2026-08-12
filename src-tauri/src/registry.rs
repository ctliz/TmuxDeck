use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
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

const ENVIRONMENT_CACHE_TTL: Duration = Duration::from_secs(60);
static ENVIRONMENT_CACHE: OnceLock<Mutex<Option<(Instant, Environment)>>> = OnceLock::new();

fn environment_cache() -> &'static Mutex<Option<(Instant, Environment)>> {
    ENVIRONMENT_CACHE.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_environment_cache() {
    if let Ok(mut cache) = environment_cache().lock() {
        *cache = None;
    }
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
fn agent_candidate_paths(home: &str, bin: &str) -> Vec<String> {
    vec![
        format!("/opt/homebrew/bin/{}", bin),
        format!("/usr/local/bin/{}", bin),
        format!("/usr/bin/{}", bin),
        format!("{}/.cargo/bin/{}", home, bin),
        format!("{}/.local/bin/{}", home, bin),
        format!("{}/.opencode/bin/{}", home, bin),
    ]
}

#[cfg(not(target_os = "windows"))]
pub fn find_agent_binary(bin: &str) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    for p in agent_candidate_paths(&home, bin) {
        if std::path::Path::new(&p).exists() {
            return Some(p);
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

fn cci_supports_panel_mode(path: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(metadata) = std::fs::metadata(path) else {
            return false;
        };
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return false;
        }
    }

    // cci is a script entry point. Inspecting its installed source avoids
    // launching the worker/daemon during UI environment discovery.
    #[cfg(target_os = "windows")]
    let help = {
        let output = Command::new("wsl.exe")
            .args(["--", "cat", path])
            .output();
        match output {
            Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).into_owned(),
            _ => return false,
        }
    };
    #[cfg(not(target_os = "windows"))]
    let help = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(_) => return false,
    };
    ["--tui", "--id", "--name"]
        .iter()
        .all(|flag| help.contains(flag))
}

fn select_claude_entry<F>(
    cci: Option<String>,
    claude: Option<String>,
    supports_panel_mode: F,
) -> Option<ToolInfo>
where
    F: FnOnce(&str) -> bool,
{
    if let Some(path) = cci {
        if supports_panel_mode(&path) {
            return Some(ToolInfo {
                id: "claude".to_string(),
                name: "Claude Code · Intercom (cci)".to_string(),
                path,
                icon_path: None,
            });
        }
    }
    claude.map(|path| ToolInfo {
        id: "claude".to_string(),
        name: "Claude Code · Standard".to_string(),
        path,
        icon_path: None,
    })
}

#[tauri::command]
pub fn detect_environment() -> Environment {
    if let Ok(cache) = environment_cache().lock() {
        if let Some((cached_at, environment)) = cache.as_ref() {
            if cached_at.elapsed() < ENVIRONMENT_CACHE_TTL {
                return environment.clone();
            }
        }
    }
    let environment = detect_environment_uncached();
    if let Ok(mut cache) = environment_cache().lock() {
        *cache = Some((Instant::now(), environment.clone()));
    }
    environment
}

fn detect_environment_uncached() -> Environment {
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
        ("codex", "Codex", "codex"),
        ("opencode", "OpenCode", "opencode"),
        ("gemini", "Gemini CLI", "gemini"),
        ("aider", "Aider", "aider"),
    ];

    let mut installed_agents = Vec::new();
    // Only select cci after runtime verification that it is executable and
    // supports the identity flags used by the panel. Otherwise fall back to
    // the independently detected ordinary Claude binary without an error.
    if let Some(agent) = select_claude_entry(
        find_agent_binary("cci"),
        find_agent_binary("claude"),
        cci_supports_panel_mode,
    ) {
        installed_agents.push(agent);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_cache_returns_recent_value() {
        let sample = Environment {
            tmux: Some("tmux".into()),
            terminals: Vec::new(),
            agents: vec![ToolInfo {
                id: "shell".into(),
                name: "Shell".into(),
                path: "/bin/sh".into(),
                icon_path: None,
            }],
        };
        *environment_cache().lock().unwrap() = Some((Instant::now(), sample.clone()));
        let cached = detect_environment();
        assert_eq!(cached.tmux, sample.tmux);
        assert_eq!(cached.agents[0].id, "shell");
        *environment_cache().lock().unwrap() = None;
    }

    #[test]
    fn environment_cache_can_be_invalidated_after_config_save() {
        let stale = Environment {
            tmux: None,
            terminals: Vec::new(),
            agents: vec![ToolInfo {
                id: "shell".into(),
                name: "agent.shell".into(),
                path: "/bin/sh".into(),
                icon_path: None,
            }],
        };
        *environment_cache().lock().unwrap() = Some((Instant::now(), stale));
        invalidate_environment_cache();
        assert!(environment_cache().lock().unwrap().is_none());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn agent_candidates_include_native_claude_and_opencode_installs() {
        let claude = agent_candidate_paths("/Users/test", "claude");
        assert!(claude.contains(&"/Users/test/.local/bin/claude".to_string()));

        let opencode = agent_candidate_paths("/Users/test", "opencode");
        assert!(opencode.contains(&"/Users/test/.opencode/bin/opencode".to_string()));
    }

    #[test]
    fn claude_entry_uses_verified_cci_when_installed() {
        let entry = select_claude_entry(
            Some("/opt/bin/cci".to_string()),
            Some("/opt/bin/claude".to_string()),
            |path| path == "/opt/bin/cci",
        )
        .unwrap();
        assert_eq!(entry.path, "/opt/bin/cci");
        assert!(entry.name.contains("Intercom (cci)"));
    }

    #[test]
    fn claude_entry_falls_back_when_cci_is_missing_or_incompatible() {
        let missing = select_claude_entry(
            None,
            Some("/opt/bin/claude".to_string()),
            |_| unreachable!(),
        )
        .unwrap();
        assert_eq!(missing.path, "/opt/bin/claude");

        let incompatible = select_claude_entry(
            Some("/old/bin/cci".to_string()),
            Some("/opt/bin/claude".to_string()),
            |_| false,
        )
        .unwrap();
        assert_eq!(incompatible.path, "/opt/bin/claude");
        assert!(incompatible.name.contains("Standard"));
    }
}
