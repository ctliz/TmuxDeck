use crate::registry::find_binary;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TmuxPane {
    pub id: String,
    pub command: String,
    pub agent_id: Option<String>,
    pub active: bool,
    pub session_target: String,
    pub slot: Option<String>,
    pub attached: bool,
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
    Command::new("wsl.exe")
        .arg("--")
        .arg("tmux")
        .args(args)
        .output()
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
        || (lower.contains("no such file or directory")
            && (lower.contains("tmux") || lower.contains("socket") || lower.contains("/tmp/")))
}

/// 判定 stderr 是否为「目标 session 不存在」类错误（server 在，但找不到指定 session）。
///
/// 与 `is_no_server_err` 互斥：前者说没有 server，这里说 server 在但 session 没了。
/// 调用方应先查 no-server，再查本函数——两者都命不中时才是其他错误。
pub fn is_session_missing_err(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("can't find session")
        || lower.contains("invalid or unknown session")
        || lower.contains("no session")
}

pub fn sanitize_session_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("ERR_NAME_EMPTY".to_string());
    }

    let sanitized: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
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

pub(crate) fn has_attached_clients(value: &str) -> bool {
    value.trim().parse::<usize>().unwrap_or(0) > 0
}

pub fn is_session_attached(session_name: &str) -> bool {
    if let Ok(out) = run_tmux(&[
        "list-sessions",
        "-F",
        "#{session_attached}",
        "-t",
        session_name,
    ]) {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            return has_attached_clients(&stdout);
        }
    }
    false
}

pub fn get_session_first_pane_dir(session_name: &str) -> Option<String> {
    let output = run_tmux(&[
        "list-panes",
        "-t",
        session_name,
        "-F",
        "#{pane_current_path}",
    ]);
    if let Ok(out) = output {
        if out.status.success() {
            let line = String::from_utf8_lossy(&out.stdout)
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            if !line.is_empty() {
                return Some(line);
            }
        }
    }
    None
}

/// 取第一个 attached 客户端的窗口尺寸（字符宽×高），用于决定新增 split 的方向。
/// 无法获取时返回 None（调用方保持旧行为）。
pub fn get_attach_client_size() -> Option<(u32, u32)> {
    let output = run_tmux(&["list-clients", "-F", "#{client_width}|#{client_height}"]);
    if let Ok(out) = output {
        if out.status.success() {
            if let Some(line) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 2 {
                    let w = parts[0].trim().parse::<u32>().ok()?;
                    let h = parts[1].trim().parse::<u32>().ok()?;
                    if w > 0 && h > 0 {
                        return Some((w, h));
                    }
                }
            }
        }
    }
    None
}

pub fn get_session_panes(session_name: &str, attached: bool, slot: Option<&str>) -> Vec<TmuxPane> {
    let output = run_tmux(&[
        "list-panes",
        "-s",
        "-t",
        session_name,
        "-F",
        "#{pane_id}|#{pane_current_command}|#{pane_active}|#{@tmuxdeck-agent}",
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
                        agent_id: parts
                            .get(3)
                            .filter(|value| !value.is_empty())
                            .map(|value| (*value).to_string()),
                        active: parts[2] == "1",
                        session_target: session_name.to_string(),
                        slot: slot.map(String::from),
                        attached,
                    });
                }
            }
            return panes;
        }
    }
    Vec::new()
}

// ─────────────────────────────────────────────────────────────────────────────
// 全局 pane 清单
//
// 一次 list-panes -a 拿到所有 pane 的归属、进程与工作目录。相比逐 session 调用，
// 这是把 pane 关联到 agent 会话（intercom / transcript 文件）时唯一需要的一跳。
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PaneDetail {
    pub id: String,
    pub session: String,
    /// Authoritative mobile grouping metadata. Native slot sessions use the
    /// TmuxDeck workspace option; ordinary tmux panes use their session name.
    pub workspace_id: String,
    pub workspace_name: String,
    pub agent_id: Option<String>,
    pub expected_intercom_id: Option<String>,
    pub managed_claude_adapter: bool,
    /// pane 内前台进程名，例如 pi / claude / codex / zsh
    pub command: String,
    pub cwd: String,
    pub active: bool,
}

fn parse_pane_detail(line: &str) -> Option<PaneDetail> {
    let parts: Vec<&str> = line.splitn(10, '|').collect();
    if parts.len() != 10 {
        return None;
    }
    let session = parts[1].to_string();
    let workspace = if parts[8] == "1" {
        parts[9].to_string()
    } else {
        session.clone()
    };
    Some(PaneDetail {
        id: parts[0].to_string(),
        session,
        workspace_id: workspace.clone(),
        workspace_name: workspace,
        agent_id: (!parts[5].is_empty()).then(|| parts[5].to_string()),
        expected_intercom_id: (!parts[6].is_empty()).then(|| parts[6].to_string()),
        managed_claude_adapter: parts[7] == crate::claude_adapter::MANAGED_ADAPTER_MARKER,
        command: parts[2].to_string(),
        cwd: parts[3].to_string(),
        active: parts[4] == "1",
    })
}

pub fn list_all_panes() -> Vec<PaneDetail> {
    let output = run_tmux(&[
        "list-panes",
        "-a",
        "-F",
        "#{pane_id}|#{session_name}|#{pane_current_command}|#{pane_current_path}|#{pane_active}|#{@tmuxdeck-agent}|#{@tmuxdeck-intercom-id}|#{@tmuxdeck-claude-adapter}|#{@tmuxdeck-native-split}|#{@tmuxdeck-workspace}",
    ]);

    if let Ok(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut panes = Vec::new();
            for line in stdout.lines() {
                if let Some(pane) = parse_pane_detail(line) {
                    panes.push(pane);
                }
            }
            return panes;
        }
    }
    Vec::new()
}

// ─────────────────────────────────────────────────────────────────────────────
// 向 pane 发送输入
//
// 这是「手机端能回一句话」的最底层能力，也是没有 intercom 适配器的 agent
// （Aider / Gemini CLI / 纯 shell）唯一的投递通道。
//
// 安全要点：自由文本必须走 `-l`（literal）。不加时 tmux 会把 "C-c"、"Escape"
// 这类字符串当键名解析，用户消息里出现这些词就会被误当控制键执行。
// 控制键因此走 send_key_name 这条独立通道，两者不混用。
// ─────────────────────────────────────────────────────────────────────────────

pub const MAX_SEND_TEXT_BYTES: usize = 8192;

/// 发送字面文本。submit=true 时追加一个独立的 Enter。
pub fn send_keys(pane_id: &str, text: &str, submit: bool) -> Result<(), String> {
    let pane = pane_id.trim();
    if !validate_pane_id(pane) {
        return Err("ERR_PANE_INVALID".to_string());
    }
    if text.is_empty() {
        return Err("ERR_TEXT_EMPTY".to_string());
    }
    if text.len() > MAX_SEND_TEXT_BYTES {
        return Err("ERR_TEXT_TOO_LONG".to_string());
    }
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    // 多行文本逐行发送：tmux send-keys -l 不会把 \n 当提交，
    // 但部分 TUI 对裸 \n 的处理不一致，改为「行内容 + 显式 Enter」更可控。
    let lines: Vec<&str> = text.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.is_empty() {
            let output = run_tmux(&["send-keys", "-t", pane, "-l", line])
                .map_err(|e| format!("ERR_SEND_FAILED|{}", e))?;
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                if is_no_server_err(&err) {
                    return Err("ERR_TMUX_NO_SERVER".to_string());
                }
                return Err(format!("ERR_SEND_OUTPUT_ERR|{}", err));
            }
        }
        // 行间换行；最后一行是否换行由 submit 决定
        if i + 1 < lines.len() {
            send_key_name(pane, "Enter")?;
        }
    }

    if submit {
        send_key_name(pane, "Enter")?;
    }
    Ok(())
}

/// 允许发送的控制键白名单。不开放任意键名，避免把 send-keys 变成通用键盘注入口。
pub const ALLOWED_KEYS: &[&str] = &[
    "Enter", "Escape", "Tab", "BSpace", "Space", "Up", "Down", "Left", "Right", "C-c", "C-d",
    "C-l", "C-u", "C-r", "C-p", "C-n",
];

/// 发送一个具名控制键（Escape / C-c / 方向键等）。
pub fn send_key_name(pane_id: &str, key: &str) -> Result<(), String> {
    let pane = pane_id.trim();
    if !validate_pane_id(pane) {
        return Err("ERR_PANE_INVALID".to_string());
    }
    if !ALLOWED_KEYS.contains(&key) {
        return Err("ERR_KEY_NOT_ALLOWED".to_string());
    }
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let output =
        run_tmux(&["send-keys", "-t", pane, key]).map_err(|e| format!("ERR_SEND_FAILED|{}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_SEND_OUTPUT_ERR|{}", err));
    }
    Ok(())
}

pub fn swap_panes(pane_id_a: &str, pane_id_b: &str) -> Result<(), String> {
    let a = pane_id_a.trim();
    let b = pane_id_b.trim();
    if !validate_pane_id(a) || !validate_pane_id(b) {
        return Err("ERR_PANE_INVALID".to_string());
    }
    if check_tmux_installed().is_none() {
        return Err("ERR_TMUX_NOT_FOUND".to_string());
    }

    let output = run_tmux(&["swap-pane", "-s", a, "-t", b])
        .map_err(|e| format!("ERR_SWAP_PANE_FAILED|{}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        if is_no_server_err(&err) {
            return Err("ERR_TMUX_NO_SERVER".to_string());
        }
        return Err(format!("ERR_SWAP_PANE_FAILED|{}", err));
    }
    Ok(())
}

#[cfg(test)]
mod attached_tests {
    use super::{has_attached_clients, parse_pane_detail};

    #[test]
    fn attached_count_accepts_multiple_clients() {
        assert!(!has_attached_clients("0"));
        assert!(has_attached_clients("1"));
        assert!(has_attached_clients("2"));
    }

    #[test]
    fn pane_workspace_uses_session_for_ordinary_tmux() {
        let pane = parse_pane_detail("%1|plain|pi|/repo|1|pi|||0|").unwrap();
        assert_eq!(pane.workspace_id, "plain");
        assert_eq!(pane.workspace_name, "plain");
    }

    #[test]
    fn pane_workspace_uses_native_metadata_without_parsing_session_name() {
        let pane = parse_pane_detail(
            "%2|alpha__td_slot_inside__td_slot_07|pi|/repo|0|pi|||1|alpha__td_slot_inside",
        )
        .unwrap();
        assert_eq!(pane.workspace_id, "alpha__td_slot_inside");
        assert_eq!(pane.workspace_name, "alpha__td_slot_inside");
        assert_eq!(pane.session, "alpha__td_slot_inside__td_slot_07");
    }
}

#[cfg(all(test, target_os = "macos"))]
mod lifecycle_tests {
    use super::*;
    use std::process::{Child, Command, Output, Stdio};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct IsolatedServer {
        tmux: String,
        socket: String,
    }

    impl Drop for IsolatedServer {
        fn drop(&mut self) {
            let _ = tmux_output(&self.tmux, &self.socket, &["kill-server"]);
        }
    }

    fn tmux_output(tmux: &str, socket: &str, args: &[&str]) -> std::io::Result<Output> {
        Command::new(tmux).arg("-L").arg(socket).args(args).output()
    }

    fn wait_for_client(tmux: &str, socket: &str) -> Option<String> {
        for _ in 0..30 {
            if let Ok(output) = tmux_output(tmux, socket, &["list-clients", "-F", "#{client_pid}"])
            {
                let pid = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !pid.is_empty() {
                    return Some(pid);
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        None
    }

    fn stop_child(child: &mut Child) {
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
    }

    #[test]
    fn test_closing_attach_client_preserves_session_and_panes() {
        let Some(tmux) = check_tmux_installed() else {
            return;
        };
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let socket = format!("tmuxdeck-test-{}-{}", std::process::id(), unique);
        let _server = IsolatedServer {
            tmux: tmux.clone(),
            socket: socket.clone(),
        };

        let created = Command::new(&tmux)
            .args([
                "-L",
                &socket,
                "-f",
                "/dev/null",
                "new-session",
                "-d",
                "-s",
                "four",
                "sleep 30",
            ])
            .output()
            .unwrap();
        assert!(created.status.success());
        for _ in 1..4 {
            let split =
                tmux_output(&tmux, &socket, &["split-window", "-t", "four", "sleep 30"]).unwrap();
            assert!(split.status.success());
        }

        let mut attach = Command::new("/usr/bin/script")
            .args([
                "-q",
                "/dev/null",
                &tmux,
                "-L",
                &socket,
                "attach-session",
                "-t",
                "four",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let client_pid = wait_for_client(&tmux, &socket).expect("attach client did not connect");
        let status = Command::new("kill")
            .args(["-HUP", &client_pid])
            .status()
            .unwrap();
        assert!(status.success());

        let mut detached = false;
        for _ in 0..30 {
            let clients = tmux_output(&tmux, &socket, &["list-clients"])
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_default();
            if clients.is_empty() {
                detached = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        assert!(
            detached,
            "attach client should detach after its terminal closes"
        );
        stop_child(&mut attach);

        let sessions =
            tmux_output(&tmux, &socket, &["list-sessions", "-F", "#{session_name}"]).unwrap();
        let panes = tmux_output(&tmux, &socket, &["list-panes", "-a", "-F", "#{pane_id}"]).unwrap();
        assert_eq!(String::from_utf8_lossy(&sessions.stdout).lines().count(), 1);
        assert_eq!(String::from_utf8_lossy(&panes.stdout).lines().count(), 4);
    }
}
