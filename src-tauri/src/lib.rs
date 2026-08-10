use serde::{Deserialize, Serialize};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Environment {
    pub tmux: Option<String>,
    pub terminals: Vec<ToolInfo>,
    pub agents: Vec<ToolInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CustomAgent {
    pub name: String,
    pub command: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub default_terminal: String,
    pub default_agent: String,
    pub default_panes: u8,
    pub custom_agent: Option<CustomAgent>,
    pub recent_dirs: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_terminal: String::new(),
            default_agent: "pi".to_string(),
            default_panes: 4,
            custom_agent: None,
            recent_dirs: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateOpts {
    pub name: String,
    pub dir: Option<String>,
    pub agent_id: String,
    pub panes: u8,
    pub terminal_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TmuxPane {
    pub id: String,
    pub command: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TmuxSession {
    pub id: String,
    pub name: String,
    pub windows_count: usize,
    pub panes_count: usize,
    pub attached: bool,
    pub created_at: String,
    pub panes: Vec<TmuxPane>,
}

fn get_config_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config").join("tmuxdeck").join("config.json")
}

fn find_binary(bin_name: &str, candidate_paths: &[&str]) -> Option<String> {
    for path in candidate_paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    // Try `which`
    if let Ok(output) = Command::new("which").arg(bin_name).output() {
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    None
}

fn get_tmux_bin() -> Option<String> {
    find_binary(
        "tmux",
        &[
            "/opt/homebrew/bin/tmux",
            "/usr/local/bin/tmux",
            "/usr/bin/tmux",
        ],
    )
}

fn find_agent_binary(bin: &str) -> Option<String> {
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
    // Check ~/.nvm/versions/node/*/bin/<bin>
    let nvm_dir = format!("{}/.nvm/versions/node", home);
    if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
        for entry in entries.flatten() {
            let path = entry.path().join("bin").join(bin);
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    // Try `which`
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
fn load_config() -> Config {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&content) {
                return cfg;
            }
        }
    }
    Config::default()
}

#[tauri::command]
fn save_config(config: Config) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn detect_environment() -> Environment {
    let tmux = get_tmux_bin();

    // 终端注册表 (macOS)
    let known_terminals = vec![
        ("ghostty", "Ghostty", vec!["/Applications/Ghostty.app"]),
        ("iterm2", "iTerm2", vec!["/Applications/iTerm.app"]),
        (
            "terminal",
            "Terminal (系统)",
            vec![
                "/System/Applications/Utilities/Terminal.app",
                "/Applications/Utilities/Terminal.app",
            ],
        ),
        ("wezterm", "WezTerm", vec!["/Applications/WezTerm.app"]),
        ("kitty", "kitty", vec!["/Applications/kitty.app"]),
        ("alacritty", "Alacritty", vec!["/Applications/Alacritty.app"]),
    ];

    let mut installed_terminals = Vec::new();
    for (id, name, paths) in known_terminals {
        let mut found_path: Option<String> = None;
        for p in paths {
            if std::path::Path::new(p).exists() {
                found_path = Some(p.to_string());
                break;
            }
        }
        // Terminal.app 兜底保证存在
        if id == "terminal" && found_path.is_none() {
            found_path = Some("/System/Applications/Utilities/Terminal.app".to_string());
        }

        if let Some(path) = found_path {
            installed_terminals.push(ToolInfo {
                id: id.to_string(),
                name: name.to_string(),
                path,
            });
        }
    }

    // Agent 注册表
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
            });
        }
    }

    // 自定义 Agent 支持
    let cfg = load_config();
    if let Some(custom) = cfg.custom_agent {
        if !custom.command.trim().is_empty() {
            installed_agents.push(ToolInfo {
                id: "custom".to_string(),
                name: if custom.name.is_empty() { "自定义 Agent".to_string() } else { custom.name },
                path: custom.command,
            });
        }
    }

    // 纯 Shell 兜底 (永远可用)
    let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    installed_agents.push(ToolInfo {
        id: "shell".to_string(),
        name: "纯 Shell".to_string(),
        path: shell_path,
    });

    Environment {
        tmux,
        terminals: installed_terminals,
        agents: installed_agents,
    }
}

#[tauri::command]
fn open_session(name: String, terminal_id: String) -> Result<(), String> {
    let tmux = get_tmux_bin().ok_or_else(|| "未找到 tmux 安装".to_string())?;
    let script_path = format!("/tmp/tmuxdeck-{}.sh", name);

    let script_content = format!(
        "#!/bin/bash\nexec '{}' attach-session -t '{}'\n",
        tmux, name
    );
    std::fs::write(&script_path, script_content).map_err(|e| format!("写入脚本失败: {}", e))?;

    #[cfg(unix)]
    {
        if let Ok(meta) = std::fs::metadata(&script_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&script_path, perms);
        }
    }

    let status = match terminal_id.as_str() {
        "ghostty" => Command::new("/usr/bin/open")
            .args(["-na", "Ghostty", "--args", &format!("--command={}", script_path)])
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
        Ok(st) => Err(format!("终端打开返回错误状态: {}", st)),
        Err(e) => Err(format!("打开终端失败: {}", e)),
    }
}

#[tauri::command]
fn create_session(opts: CreateOpts) -> Result<(), String> {
    let tmux = get_tmux_bin().ok_or_else(|| "未找到 tmux 安装".to_string())?;
    let env_info = detect_environment();

    // 确定 agent 执行命令
    let agent_cmd = if opts.agent_id == "shell" {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
    } else {
        env_info
            .agents
            .iter()
            .find(|a| a.id == opts.agent_id)
            .map(|a| a.path.clone())
            .unwrap_or_else(|| opts.agent_id.clone())
    };

    let work_dir = opts
        .dir
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| "/".to_string()));

    let cd_arg = format!("-c '{}'", work_dir);
    let session_name = &opts.name;

    // 按照分屏数构建 tmux 命令
    let script = match opts.panes {
        1 => format!(
            "{} new-session -d -s '{}' {} '{}'",
            tmux, session_name, cd_arg, agent_cmd
        ),
        2 => format!(
            "{} new-session -d -s '{}' {} '{}'; {} split-window -h -t '{}' {} '{}'; {} select-layout -t '{}' tiled",
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name
        ),
        6 => format!(
            "{} new-session -d -s '{}' {} '{}'; {} split-window -h -t '{}' {} '{}'; {} split-window -v -t '{}:0.0' {} '{}'; {} split-window -v -t '{}:0.1' {} '{}'; {} split-window -v -t '{}:0.2' {} '{}'; {} split-window -v -t '{}:0.3' {} '{}'; {} select-layout -t '{}' tiled",
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name
        ),
        _ => format!( // 默认 4 分屏
            "{} new-session -d -s '{}' {} '{}'; {} split-window -h -t '{}' {} '{}'; {} split-window -v -t '{}:0.0' {} '{}'; {} split-window -v -t '{}:0.1' {} '{}'; {} select-layout -t '{}' tiled",
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name, cd_arg, agent_cmd,
            tmux, session_name
        ),
    };

    let output = Command::new("/bin/bash")
        .args(["-c", &script])
        .output()
        .map_err(|e| format!("创建 tmux session 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "创建会话报错: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // 持久化配置
    let mut cfg = load_config();
    cfg.default_terminal = opts.terminal_id.clone();
    cfg.default_agent = opts.agent_id.clone();
    cfg.default_panes = opts.panes;
    if let Some(dir) = opts.dir {
        if !dir.trim().is_empty() {
            cfg.recent_dirs.retain(|d| d != &dir);
            cfg.recent_dirs.insert(0, dir);
            if cfg.recent_dirs.len() > 5 {
                cfg.recent_dirs.truncate(5);
            }
        }
    }
    let _ = save_config(cfg);

    // 立即打开 session
    open_session(opts.name, opts.terminal_id)
}

#[tauri::command]
fn get_tmux_sessions() -> Result<Vec<TmuxSession>, String> {
    let tmux = match get_tmux_bin() {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };

    let output = Command::new(&tmux)
        .args([
            "list-sessions",
            "-F",
            "#{session_id}|#{session_name}|#{session_windows}|#{session_attached}|#{session_created}",
        ])
        .output()
        .map_err(|e| format!("无法运行 tmux list-sessions: {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if err_msg.contains("no server running") {
            return Ok(Vec::new());
        }
        return Err(format!("tmux 错误: {}", err_msg));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sessions = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 5 {
            let id = parts[0].to_string();
            let name = parts[1].to_string();
            let windows_count = parts[2].parse::<usize>().unwrap_or(1);
            let attached = parts[3] == "1";
            let created_ts = parts[4].parse::<i64>().unwrap_or(0);

            let created_at = if created_ts > 0 {
                let datetime =
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(created_ts as u64);
                if let Ok(elapsed) = datetime.elapsed() {
                    let secs = elapsed.as_secs();
                    if secs < 60 {
                        format!("{} 秒前", secs)
                    } else if secs < 3600 {
                        format!("{} 分钟前", secs / 60)
                    } else if secs < 86400 {
                        format!("{} 小时前", secs / 3600)
                    } else {
                        format!("{} 天前", secs / 86400)
                    }
                } else {
                    "刚刚".to_string()
                }
            } else {
                "未知".to_string()
            };

            let panes = get_session_panes(&tmux, &name);
            let panes_count = panes.len();

            sessions.push(TmuxSession {
                id,
                name,
                windows_count,
                panes_count,
                attached,
                created_at,
                panes,
            });
        }
    }

    Ok(sessions)
}

fn get_session_panes(tmux: &str, session_name: &str) -> Vec<TmuxPane> {
    let output = Command::new(tmux)
        .args([
            "list-panes",
            "-s",
            "-t",
            session_name,
            "-F",
            "#{pane_id}|#{pane_current_command}|#{pane_active}",
        ])
        .output();

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

#[tauri::command]
fn kill_session(session_name: String) -> Result<(), String> {
    let tmux = get_tmux_bin().ok_or_else(|| "未找到 tmux 安装".to_string())?;
    let output = Command::new(&tmux)
        .args(["kill-session", "-t", &session_name])
        .output()
        .map_err(|e| format!("销毁 session 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "销毁会话失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[tauri::command]
fn rename_session(old_name: String, new_name: String) -> Result<(), String> {
    let tmux = get_tmux_bin().ok_or_else(|| "未找到 tmux 安装".to_string())?;
    let output = Command::new(&tmux)
        .args(["rename-session", "-t", &old_name, &new_name])
        .output()
        .map_err(|e| format!("重命名 session 失败: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "重命名会话失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            detect_environment,
            load_config,
            save_config,
            create_session,
            open_session,
            get_tmux_sessions,
            kill_session,
            rename_session
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
