use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Emitter;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
    pub last_active_ts: i64,
    pub panes: Vec<TmuxPane>,
}

fn find_app_icon(app_path_str: &str) -> Option<String> {
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

#[tauri::command]
fn get_terminal_icon(terminal_id: String) -> Result<Vec<u8>, String> {
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

/// Runtime language detection (Rust side) for native tray menus.
/// Returns true when the system locale starts with "zh".
fn is_zh_locale() -> bool {
    sys_locale::get_locale()
        .map(|l| l.to_lowercase().starts_with("zh"))
        .unwrap_or(false)
}

/// Translate a tray menu label. English is the default; zh-CN overrides.
/// The `key` parameter IS the English text, so non-zh locales pass through.
fn tr(key: &str) -> String {
    if !is_zh_locale() {
        return key.to_string();
    }
    match key {
        "No Active Workspaces" => "无活动工作区".to_string(),
        "+ New Workspace..." => "+ 新建工作区...".to_string(),
        "TmuxDeck Main Window" => "TmuxDeck 主界面".to_string(),
        "Quit TmuxDeck" => "退出 TmuxDeck".to_string(),
        "Open ({})" => "打开 ({})".to_string(),
        "Add Pane" => "新增分屏".to_string(),
        "View All ({} total)..." => "查看全部（共 {} 个）...".to_string(),
        _ => key.to_string(),
    }
}

fn sanitize_session_name(raw: &str) -> Result<String, String> {
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

fn get_config_path() -> std::path::PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        config_dir.join("tmuxdeck").join("config.json")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home).join(".config").join("tmuxdeck").join("config.json")
    }
}

fn find_binary(bin_name: &str, candidate_paths: &[&str]) -> Option<String> {
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
fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("wsl.exe").arg("--").arg("tmux").args(args).output()
}

#[cfg(not(target_os = "windows"))]
fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    let tmux = check_tmux_installed().unwrap_or_else(|| "tmux".to_string());
    Command::new(tmux).args(args).output()
}

#[cfg(target_os = "windows")]
fn check_tmux_installed() -> Option<String> {
    if let Ok(out) = Command::new("wsl.exe").args(["--", "tmux", "-V"]).output() {
        if out.status.success() {
            return Some("wsl.exe -- tmux".to_string());
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn check_tmux_installed() -> Option<String> {
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
fn find_agent_binary(bin: &str) -> Option<String> {
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
fn to_wsl_path(path: String) -> String {
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
    let json = serde_json::to_string_pretty(&config).map_err(|e| format!("ERR_CONFIG_SAVE|{}", e))?;
    std::fs::write(path, json).map_err(|e| format!("ERR_CONFIG_SAVE|{}", e))?;
    Ok(())
}

#[tauri::command]
fn detect_environment() -> Environment {
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

fn is_session_attached(session_name: &str) -> bool {
    if let Ok(out) = run_tmux(&["list-sessions", "-F", "#{session_attached}", "-t", session_name]) {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            return stdout.trim() == "1";
        }
    }
    false
}

#[tauri::command]
fn open_session(name: String, terminal_id: String) -> Result<(), String> {
    let sanitized_name = sanitize_session_name(&name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
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
                .args(["new-tab", "--", "wsl.exe", "--", "tmux", "attach-session", "-t", &sanitized_name])
                .status(),
            "powershell" => Command::new("powershell.exe")
                .args(["-NoExit", "-Command", &format!("wsl.exe -- tmux attach-session -t '{}'", sanitized_name)])
                .status(),
            _ => Command::new("cmd.exe")
                .args(["/c", "start", "cmd", "/k", "wsl.exe", "--", "tmux", "attach-session", "-t", &sanitized_name])
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
        let tmux = check_tmux_installed().ok_or_else(|| "ERR_TMUX_NOT_FOUND".to_string())?;
        let script_path = format!("/tmp/tmuxdeck-{}.sh", sanitized_name);

        let script_content = format!(
            "#!/bin/bash\nexec '{}' attach-session -t '{}'\n",
            tmux, sanitized_name
        );
        std::fs::write(&script_path, script_content).map_err(|e| format!("ERR_SCRIPT_WRITE_FAILED|{}", e))?;

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
            Ok(st) => Err(format!("ERR_TERMINAL_RETURN_ERR|{}", st)),
            Err(e) => Err(format!("ERR_TERMINAL_LAUNCH_FAILED|{}", e)),
        }
    }
}

fn get_session_first_pane_dir(session_name: &str) -> Option<String> {
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

#[tauri::command]
fn add_pane(session_name: String) -> Result<(), String> {
    let sanitized = sanitize_session_name(&session_name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let work_dir = get_session_first_pane_dir(&sanitized).unwrap_or_else(|| "~".to_string());
    let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());

    let mut split_args = vec!["split-window", "-t", &sanitized];
    if !work_dir.is_empty() && work_dir != "~" {
        split_args.push("-c");
        split_args.push(&work_dir);
    }
    split_args.push(&shell_path);

    let output = run_tmux(&split_args).map_err(|e| format!("ERR_ADD_PANE_FAILED|{}", e))?;
    if !output.status.success() {
        return Err(format!(
            "ERR_ADD_PANE_OUTPUT_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let _ = run_tmux(&["select-layout", "-t", &sanitized, "tiled"]);
    Ok(())
}

#[tauri::command]
fn create_session(opts: CreateOpts) -> Result<(), String> {
    let sanitized_name = sanitize_session_name(&opts.name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let env_info = detect_environment();

    let agent_cmd = if opts.agent_id == "shell" {
        "bash".to_string()
    } else {
        env_info
            .agents
            .iter()
            .find(|a| a.id == opts.agent_id)
            .map(|a| a.path.clone())
            .unwrap_or_else(|| opts.agent_id.clone())
    };

    let work_dir_clean = opts
        .dir
        .clone()
        .filter(|d| !d.trim().is_empty())
        .map(|d| to_wsl_path(d))
        .unwrap_or_else(|| {
            if cfg!(target_os = "windows") {
                "~".to_string()
            } else {
                std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
            }
        });

    let mut new_args = vec!["new-session", "-d", "-s", &sanitized_name];
    if !work_dir_clean.is_empty() && work_dir_clean != "~" {
        new_args.push("-c");
        new_args.push(&work_dir_clean);
    }
    new_args.push(&agent_cmd);

    let output = run_tmux(&new_args).map_err(|e| format!("ERR_CREATE_FAILED|{}", e))?;
    if !output.status.success() {
        return Err(format!(
            "ERR_CREATE_OUTPUT_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let count = match opts.panes {
        1 | 2 | 4 | 6 => opts.panes as usize,
        _ => 4,
    };

    for _ in 1..count {
        let mut split_args = vec!["split-window", "-t", &sanitized_name];
        if !work_dir_clean.is_empty() && work_dir_clean != "~" {
            split_args.push("-c");
            split_args.push(&work_dir_clean);
        }
        split_args.push(&agent_cmd);

        let _ = run_tmux(&split_args);
        let _ = run_tmux(&["select-layout", "-t", &sanitized_name, "tiled"]);
    }

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

    open_session(sanitized_name, opts.terminal_id)
}

#[tauri::command]
fn get_tmux_sessions() -> Result<Vec<TmuxSession>, String> {
    if check_tmux_installed().is_none() {
        return Ok(Vec::new());
    }

    let output = run_tmux(&[
        "list-sessions",
        "-F",
        "#{session_id}|#{session_name}|#{session_windows}|#{session_attached}|#{session_created}|#{session_activity}",
    ])
    .map_err(|e| format!("ERR_TMUX_LIST_FAILED|{}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        if err_msg.contains("no server running") {
            return Ok(Vec::new());
        }
        return Err(format!("ERR_TMUX_GENERIC|{}", err_msg));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sessions = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 6 {
            let id = parts[0].to_string();
            let name = parts[1].to_string();
            let windows_count = parts[2].parse::<usize>().unwrap_or(1);
            let attached = parts[3] == "1";
            let created_ts = parts[4].parse::<i64>().unwrap_or(0);
            let last_active_ts = parts[5].parse::<i64>().unwrap_or(0);

            let created_at = if created_ts > 0 {
                let datetime =
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(created_ts as u64);
                if let Ok(elapsed) = datetime.elapsed() {
                    let secs = elapsed.as_secs();
                    if secs < 60 {
                        format!("{}s", secs)
                    } else if secs < 3600 {
                        format!("{}m", secs / 60)
                    } else if secs < 86400 {
                        format!("{}h", secs / 3600)
                    } else {
                        format!("{}d", secs / 86400)
                    }
                } else {
                    "0s".to_string()
                }
            } else {
                "-".to_string()
            };

            let panes = get_session_panes(&name);
            let panes_count = panes.len();

            sessions.push(TmuxSession {
                id,
                name,
                windows_count,
                panes_count,
                attached,
                created_at,
                last_active_ts,
                panes,
            });
        }
    }

    Ok(sessions)
}

fn get_session_panes(session_name: &str) -> Vec<TmuxPane> {
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

#[tauri::command]
fn kill_session(session_name: String) -> Result<(), String> {
    let sanitized_name = sanitize_session_name(&session_name)?;
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let output = run_tmux(&["kill-session", "-t", &sanitized_name])
        .map_err(|e| format!("ERR_KILL_FAILED|{}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ERR_KILL_OUTPUT_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[tauri::command]
fn rename_session(old_name: String, new_name: String) -> Result<(), String> {
    let sanitized_old = sanitize_session_name(&old_name)?;
    let sanitized_new = sanitize_session_name(&new_name)?;

    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let output = run_tmux(&["rename-session", "-t", &sanitized_old, &sanitized_new])
        .map_err(|e| format!("ERR_RENAME_FAILED|{}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ERR_RENAME_OUTPUT_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn validate_pane_id(pane_id: &str) -> bool {
    let trimmed = pane_id.trim();
    if !trimmed.starts_with('%') || trimmed.len() < 2 {
        return false;
    }
    trimmed[1..].chars().all(|c| c.is_ascii_digit())
}

#[tauri::command]
fn kill_pane(pane_id: String) -> Result<(), String> {
    let trimmed = pane_id.trim();
    if !validate_pane_id(trimmed) {
        return Err("ERR_KILL_PANE_INVALID".to_string());
    }
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let output = run_tmux(&["kill-pane", "-t", trimmed]).map_err(|e| format!("ERR_KILL_PANE_FAILED|{}", e))?;
    if !output.status.success() {
        return Err(format!(
            "ERR_KILL_PANE_OUTPUT_ERR|{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn strip_ansi(input: &str) -> String {
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

#[tauri::command]
fn capture_pane(pane_id: String, max_lines: usize) -> Result<String, String> {
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

fn build_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<tauri::menu::Menu<R>, Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    let cfg = load_config();
    let default_terminal = if !cfg.default_terminal.is_empty() {
        cfg.default_terminal
    } else {
        "ghostty".to_string()
    };

    let sessions = get_tmux_sessions().unwrap_or_default();
    let menu = MenuBuilder::new(app);

    if sessions.is_empty() {
        let no_sess_item = MenuItemBuilder::with_id("no-sessions", tr("No Active Workspaces"))
            .enabled(false)
            .build(app)?;
        let new_item = MenuItemBuilder::with_id("new-workspace", tr("+ New Workspace...")).build(app)?;
        let show_item = MenuItemBuilder::with_id("show-main", tr("TmuxDeck Main Window")).build(app)?;
        let quit_item = MenuItemBuilder::with_id("quit", tr("Quit TmuxDeck")).build(app)?;

        return Ok(menu.item(&no_sess_item).separator().item(&new_item).separator().item(&show_item).item(&quit_item).build()?);
    }

    let mut sorted_sessions = sessions.clone();
    sorted_sessions.sort_by(|a, b| {
        if a.attached != b.attached {
            return b.attached.cmp(&a.attached);
        }
        b.last_active_ts.cmp(&a.last_active_ts)
    });

    let primary = &sorted_sessions[0];
    let icon_dot = if primary.attached { "●" } else { "○" };
    let active_header_title = format!("{} Active: {}", icon_dot, primary.name);

    let active_open = MenuItemBuilder::with_id(
        format!("open:{}", primary.name),
        tr("Open ({})").replace("{}", &default_terminal),
    )
    .build(app)?;
    let active_add_pane = MenuItemBuilder::with_id(format!("addpane:{}", primary.name), tr("Add Pane")).build(app)?;

    let active_submenu = SubmenuBuilder::new(app, active_header_title)
        .item(&active_open)
        .item(&active_add_pane)
        .build()?;

    let mut menu = menu.item(&active_submenu).separator();

    let limit = 8;
    for session in sorted_sessions.iter().take(limit) {
        let sess_dot = if session.attached { "●" } else { "○" };
        let title = format!("{} {}", sess_dot, session.name);
        let item = MenuItemBuilder::with_id(format!("open:{}", session.name), title).build(app)?;
        menu = menu.item(&item);
    }

    if sorted_sessions.len() > limit {
        let more_title = tr("View All ({} total)...").replace("{}", &sorted_sessions.len().to_string());
        let view_more_item = MenuItemBuilder::with_id("show-main", more_title).build(app)?;
        menu = menu.item(&view_more_item);
    }

    let menu = menu.separator();

    let new_item = MenuItemBuilder::with_id("new-workspace", tr("+ New Workspace...")).build(app)?;
    let show_item = MenuItemBuilder::with_id("show-main", tr("TmuxDeck Main Window")).build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", tr("Quit TmuxDeck")).build(app)?;

    let menu = menu
        .item(&new_item)
        .separator()
        .item(&show_item)
        .item(&quit_item);

    Ok(menu.build()?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            #[cfg(desktop)]
            {
                use tauri::tray::TrayIconBuilder;
                use tauri::Manager;

                let handle = app.handle().clone();

                if let Ok(initial_menu) = build_tray_menu(&handle) {
                    let _tray = TrayIconBuilder::with_id("main")
                        .icon(app.default_window_icon().unwrap().clone())
                        .menu(&initial_menu)
                        .on_menu_event(|app, event| {
                            let event_id = event.id().as_ref();
                            if event_id.starts_with("open:") {
                                let session_name = &event_id[5..];
                                let cfg = load_config();
                                let term = if !cfg.default_terminal.is_empty() {
                                    cfg.default_terminal
                                } else {
                                    "ghostty".to_string()
                                };
                                let _ = open_session(session_name.to_string(), term);
                            } else if event_id.starts_with("addpane:") {
                                let session_name = &event_id[8..];
                                let _ = add_pane(session_name.to_string());
                            } else if event_id == "new-workspace" || event_id == "show-main" {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                    if event_id == "new-workspace" {
                                        let _ = window.emit("trigger-new-workspace", ());
                                    }
                                }
                            } else if event_id == "quit" {
                                app.exit(0);
                            }
                        })
                        .build(app);
                }

                let refresh_handle = app.handle().clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    if let Some(tray) = refresh_handle.tray_by_id("main") {
                        if let Ok(new_menu) = build_tray_menu(&refresh_handle) {
                            let _ = tray.set_menu(Some(new_menu));
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            to_wsl_path,
            detect_environment,
            load_config,
            save_config,
            create_session,
            open_session,
            get_tmux_sessions,
            kill_session,
            rename_session,
            capture_pane,
            add_pane,
            kill_pane,
            get_terminal_icon
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_session_name() {
        assert_eq!(sanitize_session_name(""), Err("ERR_NAME_EMPTY".to_string()));
        assert_eq!(sanitize_session_name("   "), Err("ERR_NAME_EMPTY".to_string()));

        assert_eq!(sanitize_session_name("foo@bar#baz!"), Ok("foo-bar-baz".to_string()));
        assert_eq!(sanitize_session_name("  hello   world  "), Ok("hello-world".to_string()));

        let long_name = "a".repeat(70);
        let result = sanitize_session_name(&long_name).unwrap();
        assert_eq!(result.len(), 60);
        assert_eq!(result, "a".repeat(60));

        assert_eq!(sanitize_session_name("---"), Err("ERR_NAME_INVALID".to_string()));
        assert_eq!(sanitize_session_name("!!!"), Err("ERR_NAME_INVALID".to_string()));
    }

    #[test]
    fn test_validate_pane_id() {
        assert!(validate_pane_id("%123"));
        assert!(validate_pane_id("%0"));

        assert!(!validate_pane_id("%abc"));
        assert!(!validate_pane_id(""));
        assert!(!validate_pane_id("123"));
        assert!(!validate_pane_id("%"));
    }

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\x1b[31mHello\x1b[0m"), "Hello");
        assert_eq!(strip_ansi("\x1b[1m\x1b[32mNested\x1b[0m"), "Nested");
        assert_eq!(strip_ansi("Plain text"), "Plain text");
    }

    #[test]
    fn test_run_tmux_smoke() {
        if check_tmux_installed().is_some() {
            let res = run_tmux(&["list-sessions"]);
            assert!(res.is_ok(), "run_tmux failed to execute command");
            let output = res.unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            // tmux exits non-zero when no server is running yet; both outcomes are valid.
            assert!(
                output.status.success() || stderr.contains("no server running"),
                "tmux should either succeed or report no server running, stderr: {}",
                stderr
            );
        }
    }
}

