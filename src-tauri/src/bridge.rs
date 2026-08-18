//! 会话桥：把 tmux pane 与 intercom 会话对应起来，向上暴露统一的「对话」抽象。
//!
//! 手机端要的不是终端模拟，也不是通知收件箱，而是**一组可以同时进行的对话**——
//! 每个 pane 里的 agent 是一个对话对象，既能直接跟它说话，也能让它去找别的 agent。
//! 本模块提供那个对话模型，以及一个与具体传输无关的抽象层。
//!
//! 三条数据通路：
//!
//! | 用途 | 来源 | 说明 |
//! |---|---|---|
//! | 有哪些对话、各自什么状态 | intercom broker + `tmux list-panes -a` | broker 提供事实状态，tmux 提供未接入 intercom 的 pane |
//! | 我说的话 → agent | `intercom send`（优先）/ `tmux send-keys`（兜底） | 前者由 broker 做忙时排队，不会打断思考中的 agent |
//! | agent 说的话 → 我 | `TranscriptSource` | 见下方说明，这是目前唯一没有理想解的一环 |

use crate::intercom::{AgentStatus, IntercomClient, SessionInfo};
use crate::tmux::{send_keys, PaneDetail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// 对话模型
// ─────────────────────────────────────────────────────────────────────────────

/// 已知的 agent 类型。用于挑选 transcript 读取器与状态判定策略。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    Pi,
    ClaudeCode,
    Codex,
    OpenCode,
    Grok,
    Agy,
    Gemini,
    Aider,
    Shell,
    Unknown,
}

impl AgentKind {
    /// 从 `pane_current_command` 推断。注意这拿到的是前台进程名，
    /// agent 在跑子进程时（例如 pi 执行 bash 工具）会短暂变成别的名字，
    /// 所以推断结果应当缓存，不要每轮重算后直接覆盖。
    pub fn from_command(cmd: &str) -> Self {
        let c = cmd.trim().to_ascii_lowercase();
        let base = c.rsplit('/').next().unwrap_or(&c);
        match base {
            "pi" => AgentKind::Pi,
            "claude" | "claude-code" | "cci" | "ccim" => AgentKind::ClaudeCode,
            "codex" | "coi" => AgentKind::Codex,
            "opencode" => AgentKind::OpenCode,
            "grok" => AgentKind::Grok,
            "agy" => AgentKind::Agy,
            "gemini" => AgentKind::Gemini,
            "aider" => AgentKind::Aider,
            "bash" | "zsh" | "fish" | "sh" => AgentKind::Shell,
            _ => AgentKind::Unknown,
        }
    }

    /// 该类型是否有 intercom 适配器可用。
    ///
    /// 注意：Claude Code / Codex / OpenCode 的适配器属于 `dataforxyz` 的跨 harness
    /// 分支；若本机装的是 `nicobailon/pi-intercom` 原版，则只有 pi 能接入。
    /// 因此这里只表示「理论上可接入」，实际是否连上以 broker 的注册表为准。
    pub fn intercom_capable(self) -> bool {
        matches!(
            self,
            AgentKind::Pi
                | AgentKind::ClaudeCode
                | AgentKind::Codex
                | AgentKind::OpenCode
                | AgentKind::Grok
                | AgentKind::Agy
        )
    }
}

/// 一个对话 = 一个可寻址的 agent。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    /// 对话 ID 即 pane ID（`%3`），全局唯一且稳定
    pub id: String,
    pub session: String,
    /// Authoritative workspace grouping supplied by tmux metadata.
    pub workspace_id: String,
    pub workspace_name: String,
    pub cwd: String,
    pub kind: AgentKind,
    /// Backend-authoritative transcript reliability for the source selected now.
    pub transcript_kind: TranscriptKind,
    /// 展示名：优先用 intercom 会话名，否则回退到 tmux session 名
    pub title: String,
    /// 对应的 intercom 会话 ID；None 表示这个 pane 没接入总线，只能走 send-keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intercom_session_id: Option<String>,
    /// TmuxDeck-managed panes persist the expected routing ID in tmux metadata.
    /// It is a consistency check after process-tree matching, never authorization.
    #[serde(skip)]
    pub expected_intercom_id: Option<String>,
    /// True only for panes launched through TmuxDeck's pinned Claude adapter.
    /// Legacy panes intentionally do not acquire stricter matching rules.
    #[serde(skip)]
    pub managed_claude_adapter: bool,
    /// 实时状态。有 intercom 时是事实；没有时为 Unknown（不猜）
    pub status: ConversationStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TranscriptKind {
    Structured,
    Capture,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConversationStatus {
    Idle,
    Thinking,
    RunningTool,
    /// 有人在 ask 我们，正阻塞等待回复——手机端应当置顶并推送
    AwaitingHuman,
    Unknown,
}

impl From<AgentStatus> for ConversationStatus {
    fn from(s: AgentStatus) -> Self {
        match s {
            AgentStatus::Idle => ConversationStatus::Idle,
            AgentStatus::Thinking => ConversationStatus::Thinking,
            AgentStatus::Tool(_) => ConversationStatus::RunningTool,
            AgentStatus::Unknown => ConversationStatus::Unknown,
        }
    }
}

/// 投递路径。手机端不需要知道，但日志和调试需要。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryRoute {
    /// 经 broker：可靠、忙时排队、有回执
    Intercom,
    /// 直接往 pane 塞字符：可能被吞、可能打断，仅用于未接入总线的 agent
    SendKeys,
}

// ─────────────────────────────────────────────────────────────────────────────
// pane ↔ intercom 会话 的关联
// ─────────────────────────────────────────────────────────────────────────────

/// 一次性读取系统进程父链。不能对每个 session、每层父进程单独 spawn `ps`：
/// broker 会话较多时会造成严重 fork storm。
#[cfg(unix)]
fn process_parent_map() -> HashMap<i64, i64> {
    let mut parents = HashMap::new();
    let Ok(out) = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid="])
        .output()
    else {
        return parents;
    };
    if !out.status.success() {
        return parents;
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.split_whitespace();
        let Some(pid) = parts.next().and_then(|value| value.parse::<i64>().ok()) else {
            continue;
        };
        let Some(ppid) = parts.next().and_then(|value| value.parse::<i64>().ok()) else {
            continue;
        };
        parents.insert(pid, ppid);
    }
    parents
}

#[cfg(not(unix))]
fn process_parent_map() -> HashMap<i64, i64> {
    HashMap::new()
}

/// 沿父链上溯，找到 `pid` 所属的 pane。
///
/// `pane_pids` 是 pane_pid → pane_id 的映射。最多上溯 12 层，
/// 足够覆盖 shell → agent → 包装脚本 这类嵌套，同时防止环形父链导致死循环。
pub fn find_owning_pane(
    pid: i64,
    pane_pids: &HashMap<i64, String>,
    parents: &HashMap<i64, i64>,
) -> Option<String> {
    let mut current = pid;
    for _ in 0..12 {
        if let Some(pane) = pane_pids.get(&current) {
            return Some(pane.clone());
        }
        match parents.get(&current).copied() {
            Some(p) if p > 1 && p != current => current = p,
            _ => break,
        }
    }
    None
}

/// 读取所有 pane 的 pane_pid，用于关联。
pub fn pane_pid_map() -> HashMap<i64, String> {
    let mut map = HashMap::new();
    if let Ok(out) = crate::tmux::run_tmux(&["list-panes", "-a", "-F", "#{pane_pid}|#{pane_id}"]) {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if let Some((pid, pane)) = line.split_once('|') {
                    if let Ok(pid) = pid.trim().parse::<i64>() {
                        map.insert(pid, pane.trim().to_string());
                    }
                }
            }
        }
    }
    map
}

fn compatible_cwd(left: &str, right: &str) -> bool {
    let canonical = |value: &str| std::fs::canonicalize(value).ok();
    match (canonical(left), canonical(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left.trim_end_matches('/') == right.trim_end_matches('/'),
    }
}

fn session_matches_pane(session: &SessionInfo, conversation: &Conversation) -> bool {
    if !conversation.managed_claude_adapter {
        // Backward compatibility: the process tree is the primary evidence for
        // existing Pi/Codex/OpenCode/Claude panes. Do not add model substring
        // guesses here; they can misclassify valid sessions.
        return true;
    }
    conversation.kind == AgentKind::ClaudeCode
        && conversation
            .expected_intercom_id
            .as_deref()
            .is_some_and(|expected| session.id == expected)
        && compatible_cwd(&session.cwd, &conversation.cwd)
        && session.model.eq_ignore_ascii_case("claude")
}

// ─────────────────────────────────────────────────────────────────────────────
// 对话注册表
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ConversationRegistry {
    /// pane_id → 对话
    conversations: HashMap<String, Conversation>,
    /// intercom session_id → pane_id
    intercom_to_pane: HashMap<String, String>,
}

impl ConversationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 用 tmux 的 pane 清单重建骨架。没有 intercom 时这就是全部信息。
    pub fn refresh_panes(&mut self, panes: Vec<PaneDetail>) {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for p in panes {
            seen.insert(p.id.clone());
            let kind = p
                .agent_id
                .as_deref()
                .map(AgentKind::from_command)
                .unwrap_or_else(|| AgentKind::from_command(&p.command));
            let entry = self
                .conversations
                .entry(p.id.clone())
                .or_insert_with(|| Conversation {
                    id: p.id.clone(),
                    session: p.session.clone(),
                    workspace_id: p.workspace_id.clone(),
                    workspace_name: p.workspace_name.clone(),
                    cwd: p.cwd.clone(),
                    kind,
                    transcript_kind: TranscriptKind::Capture,
                    title: p.session.clone(),
                    intercom_session_id: None,
                    expected_intercom_id: p.expected_intercom_id.clone(),
                    managed_claude_adapter: p.managed_claude_adapter,
                    status: ConversationStatus::Unknown,
                });
            entry.session = p.session;
            entry.workspace_id = p.workspace_id;
            entry.workspace_name = p.workspace_name;
            entry.cwd = p.cwd;
            entry.expected_intercom_id = p.expected_intercom_id;
            entry.managed_claude_adapter = p.managed_claude_adapter;
            // 只在推断出具体 agent 时更新 kind：agent 执行工具时
            // pane_current_command 会临时变成 bash 之类，不能据此把 kind 打回 Shell
            if kind != AgentKind::Unknown && kind != AgentKind::Shell {
                entry.kind = kind;
            }
        }
        // 清掉已消失的 pane（先算存活集合，避免两个字段的借用在闭包里交叠）
        self.conversations.retain(|id, _| seen.contains(id));
        let alive: std::collections::HashSet<String> = self.conversations.keys().cloned().collect();
        self.intercom_to_pane.retain(|_, pane| alive.contains(pane));
    }

    /// 把 intercom 会话并入对话表：补上真实状态与可靠投递路径。
    pub fn apply_intercom_sessions(&mut self, sessions: &[SessionInfo], self_id: Option<&str>) {
        let pane_pids = pane_pid_map();
        let parents = process_parent_map();
        self.apply_intercom_snapshot(sessions, self_id, &pane_pids, &parents);
    }

    /// Workspace-scoped snapshot: 只清空和重建指定 workspace 内的对话和路由，绝不清空其他 workspace。
    pub fn apply_workspace_intercom_sessions(
        &mut self,
        workspace_id: &str,
        sessions: &[SessionInfo],
        self_id: Option<&str>,
    ) {
        let pane_pids = pane_pid_map();
        let parents = process_parent_map();
        self.apply_workspace_intercom_snapshot(
            workspace_id,
            sessions,
            self_id,
            &pane_pids,
            &parents,
        );
    }

    fn apply_workspace_intercom_snapshot(
        &mut self,
        workspace_id: &str,
        sessions: &[SessionInfo],
        self_id: Option<&str>,
        pane_pids: &HashMap<i64, String>,
        parents: &HashMap<i64, i64>,
    ) {
        // 只清理属于当前 workspace 的路由映射
        self.intercom_to_pane.retain(|_, pane_id| {
            self.conversations
                .get(pane_id)
                .map(|c| c.workspace_id != workspace_id)
                .unwrap_or(false)
        });
        // 只重置属于当前 workspace 的 conversation 状态
        for conv in self.conversations.values_mut() {
            if conv.workspace_id == workspace_id {
                conv.intercom_session_id = None;
                conv.status = ConversationStatus::Unknown;
                conv.title = conv.session.clone();
            }
        }

        let mut candidates: HashMap<String, Vec<&SessionInfo>> = HashMap::new();
        for session in sessions {
            if Some(session.id.as_str()) == self_id {
                continue;
            }
            let Some(pane_id) = find_owning_pane(session.pid, pane_pids, parents) else {
                continue;
            };
            let Some(conversation) = self.conversations.get(&pane_id) else {
                continue;
            };
            // 必须匹配同 workspace 的 pane
            if conversation.workspace_id == workspace_id
                && session_matches_pane(session, conversation)
            {
                candidates.entry(pane_id).or_default().push(session);
            }
        }

        for (pane_id, matches) in candidates {
            let [session] = matches.as_slice() else {
                continue;
            };
            let Some(conversation) = self.conversations.get_mut(&pane_id) else {
                continue;
            };
            conversation.intercom_session_id = Some(session.id.clone());
            if let Some(name) = &session.name {
                conversation.title = name.clone();
            }
            conversation.status = session.agent_status().into();
            self.intercom_to_pane.insert(session.id.clone(), pane_id);
        }
    }

    fn apply_intercom_snapshot(
        &mut self,
        sessions: &[SessionInfo],
        self_id: Option<&str>,
        pane_pids: &HashMap<i64, String>,
        parents: &HashMap<i64, i64>,
    ) {
        // A full broker snapshot is authoritative. Rebuild mappings so a newly
        // conflicting/rejected session can never leave a stale delivery route.
        self.intercom_to_pane.clear();
        for conv in self.conversations.values_mut() {
            conv.intercom_session_id = None;
            conv.status = ConversationStatus::Unknown;
            conv.title = conv.session.clone();
        }

        let mut candidates: HashMap<String, Vec<&SessionInfo>> = HashMap::new();
        for session in sessions {
            if Some(session.id.as_str()) == self_id {
                continue;
            }
            let Some(pane_id) = find_owning_pane(session.pid, pane_pids, parents) else {
                continue;
            };
            let Some(conversation) = self.conversations.get(&pane_id) else {
                continue;
            };
            if session_matches_pane(session, conversation) {
                candidates.entry(pane_id).or_default().push(session);
            }
        }

        for (pane_id, matches) in candidates {
            // A duplicate claim is ambiguous. The entire pane fails closed and
            // remains on the send-keys fallback; iteration order never wins.
            let [session] = matches.as_slice() else {
                continue;
            };
            let Some(conversation) = self.conversations.get_mut(&pane_id) else {
                continue;
            };
            conversation.intercom_session_id = Some(session.id.clone());
            if let Some(name) = &session.name {
                conversation.title = name.clone();
            }
            conversation.status = session.agent_status().into();
            self.intercom_to_pane.insert(session.id.clone(), pane_id);
        }
    }

    /// 单个会话状态变更（presence_update）。
    pub fn apply_presence(&mut self, session: &SessionInfo) {
        if let Some(pane_id) = self.intercom_to_pane.get(&session.id) {
            if let Some(conv) = self.conversations.get_mut(pane_id) {
                conv.status = session.agent_status().into();
                if let Some(name) = &session.name {
                    conv.title = name.clone();
                }
            }
        }
    }

    /// 标记某会话正在等我们回话（收到 expectsReply 的消息时调用）。
    pub fn mark_awaiting_human(&mut self, intercom_session_id: &str) {
        if let Some(pane_id) = self.intercom_to_pane.get(intercom_session_id) {
            if let Some(conv) = self.conversations.get_mut(pane_id) {
                conv.status = ConversationStatus::AwaitingHuman;
            }
        }
    }

    pub fn get(&self, pane_id: &str) -> Option<&Conversation> {
        self.conversations.get(pane_id)
    }

    pub fn set_transcript_kind(&mut self, pane_id: &str, kind: TranscriptKind) {
        if let Some(conv) = self.conversations.get_mut(pane_id) {
            conv.transcript_kind = kind;
        }
    }

    pub fn mark_pane_awaiting_human(&mut self, pane_id: &str) {
        if let Some(conv) = self.conversations.get_mut(pane_id) {
            conv.status = ConversationStatus::AwaitingHuman;
        }
    }

    pub fn clear_pane_awaiting_human(&mut self, pane_id: &str) {
        if let Some(conv) = self.conversations.get_mut(pane_id) {
            if conv.status == ConversationStatus::AwaitingHuman {
                conv.status = ConversationStatus::Idle;
            }
        }
    }

    pub fn by_intercom_id(&self, session_id: &str) -> Option<&Conversation> {
        self.intercom_to_pane
            .get(session_id)
            .and_then(|p| self.conversations.get(p))
    }

    /// 列出全部对话，等人的排在最前。
    pub fn list(&self) -> Vec<Conversation> {
        let mut v: Vec<Conversation> = self.conversations.values().cloned().collect();
        v.sort_by_key(|c| {
            let rank = match c.status {
                ConversationStatus::AwaitingHuman => 0,
                ConversationStatus::Idle => 1,
                ConversationStatus::Thinking => 2,
                ConversationStatus::RunningTool => 2,
                ConversationStatus::Unknown => 3,
            };
            (rank, c.title.clone(), c.id.clone())
        });
        v
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 投递
// ─────────────────────────────────────────────────────────────────────────────

/// 把一段文本送给某个对话。
///
/// 优先经 intercom：broker 会在目标空闲时才注入，不打断正在思考的 agent，
/// 并给出送达回执。目标未接入总线时才退回 send-keys。
pub fn deliver(
    conv: &Conversation,
    text: &str,
    intercom: Option<&IntercomClient>,
) -> Result<DeliveryRoute, String> {
    if let (Some(client), Some(session_id)) = (intercom, conv.intercom_session_id.as_deref()) {
        if client.is_connected() {
            client.send(session_id, text)?;
            return Ok(DeliveryRoute::Intercom);
        }
    }
    send_keys(&conv.id, text, true)?;
    Ok(DeliveryRoute::SendKeys)
}

/// 跨对话转发：把 A 的内容送给 B，并标注来源。
///
/// pi 家族内部本来就能互通，但 pi ↔ Claude Code ↔ Codex 之间没有通道时，
/// 人就是那座桥。这个函数是手机端「转发」按钮的后端。
pub fn forward(
    from: &Conversation,
    to: &Conversation,
    text: &str,
    intercom: Option<&IntercomClient>,
) -> Result<DeliveryRoute, String> {
    let body = format!("[来自 {} · {}]\n{}", from.title, from.session, text);
    deliver(to, &body, intercom)
}

// ─────────────────────────────────────────────────────────────────────────────
// 对话内容的来源
// ─────────────────────────────────────────────────────────────────────────────

/// 一条对话消息，供手机端渲染。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub conversation_id: String,
    pub role: TurnRole,
    pub text: String,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TurnRole {
    Human,
    Agent,
    /// 来自另一个 agent 的消息（经 intercom 转发）
    Peer,
    System,
}

/// 对话内容的来源。
///
/// **这是目前唯一没有理想解的一环。** 三个候选：
///
/// 1. `capture-pane` —— 只能拿到当前屏幕，历史会滚掉；TUI 重绘导致内容抖动。
///    只适合做卡片预览，不适合做对话流。本文件提供的 `CapturePaneSource` 属于此类，
///    定位是兜底。
/// 2. `pipe-pane` 抓原始输出流 —— 拿得到全部字节，但混着大量光标移动与重绘转义序列，
///    从中还原「谁说了什么」非常困难。
/// 3. **读 agent 自己的结构化会话记录** —— Claude Code 的 `~/.claude/projects/**/*.jsonl`、
///    pi 的会话历史等，本身就是干净的分轮次记录。这是唯一能真正撑起「对话」体验的路径，
///    代价是每个 agent 要写一个读取器，且需要把 pane 关联到对应的记录文件。
///
/// 建议按 3 做主路径、1 做兜底。trait 在此，具体实现待定。
pub trait TranscriptSource: Send {
    /// 拉取自 `since`（毫秒时间戳）之后的新轮次。
    fn poll(&mut self, conv: &Conversation, since: i64) -> Result<Vec<Turn>, String>;
}

/// 兜底实现：抓当前屏幕，整屏作为一条 agent 轮次返回。
///
/// 明确的局限：没有轮次边界、没有历史、内容会随重绘抖动。
/// 它的意义只是让未接入 intercom、也没有结构化记录的 agent（Aider、纯 shell）
/// 在手机端不至于完全空白。
pub struct CapturePaneSource {
    pub max_lines: usize,
}

impl Default for CapturePaneSource {
    fn default() -> Self {
        Self { max_lines: 40 }
    }
}

impl TranscriptSource for CapturePaneSource {
    fn poll(&mut self, conv: &Conversation, _since: i64) -> Result<Vec<Turn>, String> {
        let text = crate::commands::capture_pane(conv.id.clone(), self.max_lines)?;
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Turn {
            conversation_id: conv.id.clone(),
            role: TurnRole::Agent,
            text,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        }])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 传输抽象（手机端）
// ─────────────────────────────────────────────────────────────────────────────

/// 推给手机端的事件。传输层只负责把它序列化送出去。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ClientEvent {
    /// 全量对话列表，连接建立时下发一次
    Conversations {
        items: Vec<Conversation>,
    },
    /// 单个对话状态变化
    StatusChanged {
        id: String,
        status: ConversationStatus,
    },
    /// 对话里新增一轮内容
    Turn {
        turn: Turn,
    },
    /// 某个 agent 在等人回话——手机端应据此发通知
    AwaitingHuman {
        id: String,
        title: String,
        preview: String,
        /// 回复时要带上的 intercom 消息 ID
        #[serde(rename = "replyTo")]
        reply_to: Option<String>,
    },
    Error {
        message: String,
    },
}

/// 手机端发来的指令。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ClientCommand {
    /// 在某个对话里说一句话
    Say { id: String, text: String },
    /// 发一个控制键（Escape / C-c 等）
    Key { id: String, key: String },
    /// 把 A 的内容转发给 B
    Forward {
        from: String,
        to: String,
        text: String,
    },
    /// 请求刷新对话列表
    Refresh,
    /// 进入某个对话：只推它的 turn（单活跃订阅，新 subscribe 替换旧）
    Subscribe { id: String },
    /// 退出当前对话：停止推 turn
    Unsubscribe,
}

/// 与手机端之间的传输通道。
///
/// 目前刻意只定义抽象：真正的实现（WebSocket 服务端、或经由 IM Bot 的降级通道）
/// 取决于后续决定。`LogTransport` 让上层逻辑现在就能跑起来并被测试。
pub trait Transport: Send {
    /// 向客户端推送一个事件
    fn emit(&mut self, event: &ClientEvent) -> Result<(), String>;
    /// 当前是否有客户端连着。无人在线时上层可以省掉 transcript 轮询
    fn has_clients(&self) -> bool;
}

/// 把事件打到日志的实现，用于开发期与单元测试。
#[derive(Default)]
pub struct LogTransport {
    pub events: Vec<ClientEvent>,
}

impl Transport for LogTransport {
    fn emit(&mut self, event: &ClientEvent) -> Result<(), String> {
        println!(
            "[transport] {}",
            serde_json::to_string(event).unwrap_or_else(|e| format!("<序列化失败: {}>", e))
        );
        self.events.push(event.clone());
        Ok(())
    }

    fn has_clients(&self) -> bool {
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, session: &str, cmd: &str) -> PaneDetail {
        PaneDetail {
            id: id.to_string(),
            session: session.to_string(),
            workspace_id: session.to_string(),
            workspace_name: session.to_string(),
            agent_id: None,
            expected_intercom_id: None,
            managed_claude_adapter: false,
            command: cmd.to_string(),
            cwd: "/tmp/proj".to_string(),
            active: true,
        }
    }

    #[test]
    fn test_find_owning_pane_uses_parent_snapshot() {
        let pane_pids = HashMap::from([(100, "%1".to_string())]);
        let parents = HashMap::from([(300, 200), (200, 100)]);
        assert_eq!(
            find_owning_pane(300, &pane_pids, &parents),
            Some("%1".to_string())
        );
        assert_eq!(
            find_owning_pane(100, &pane_pids, &parents),
            Some("%1".to_string())
        );
    }

    #[test]
    fn test_find_owning_pane_stops_on_missing_or_cyclic_parent() {
        let pane_pids = HashMap::from([(100, "%1".to_string())]);
        assert_eq!(find_owning_pane(300, &pane_pids, &HashMap::new()), None);

        let cycle = HashMap::from([(300, 200), (200, 300)]);
        assert_eq!(find_owning_pane(300, &pane_pids, &cycle), None);
    }

    #[test]
    fn test_find_owning_pane_limits_parent_depth() {
        let pane_pids = HashMap::from([(2, "%1".to_string())]);
        let parents: HashMap<i64, i64> = (3..=15).map(|pid| (pid, pid - 1)).collect();
        assert_eq!(find_owning_pane(15, &pane_pids, &parents), None);
    }

    #[test]
    fn test_agent_kind_from_command() {
        assert_eq!(AgentKind::from_command("pi"), AgentKind::Pi);
        assert_eq!(AgentKind::from_command("claude"), AgentKind::ClaudeCode);
        assert_eq!(
            AgentKind::from_command("/usr/local/bin/codex"),
            AgentKind::Codex
        );
        assert_eq!(AgentKind::from_command("grok"), AgentKind::Grok);
        assert_eq!(AgentKind::from_command("agy"), AgentKind::Agy);
        assert!(AgentKind::Grok.intercom_capable());
        assert!(AgentKind::Agy.intercom_capable());
        assert_eq!(AgentKind::from_command("zsh"), AgentKind::Shell);
        assert_eq!(AgentKind::from_command("vim"), AgentKind::Unknown);
    }

    #[test]
    fn test_refresh_panes_builds_conversations() {
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![pane("%1", "proj", "pi"), pane("%2", "proj", "zsh")]);
        assert_eq!(reg.list().len(), 2);
        assert_eq!(reg.get("%1").unwrap().kind, AgentKind::Pi);
        assert_eq!(reg.get("%2").unwrap().kind, AgentKind::Shell);
        assert_eq!(reg.get("%1").unwrap().workspace_id, "proj");
        assert_eq!(reg.get("%1").unwrap().workspace_name, "proj");
    }

    #[test]
    fn native_workspace_metadata_is_authoritative_and_updates_without_changing_id() {
        let mut native = pane("%1", "proj__td_slot_01", "pi");
        native.workspace_id = "proj".into();
        native.workspace_name = "proj".into();
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![native.clone()]);
        assert_eq!(reg.get("%1").unwrap().workspace_id, "proj");
        assert_eq!(reg.get("%1").unwrap().id, "%1");

        native.workspace_id = "renamed".into();
        native.workspace_name = "renamed".into();
        reg.refresh_panes(vec![native]);
        let conversation = reg.get("%1").unwrap();
        assert_eq!(conversation.workspace_id, "renamed");
        assert_eq!(conversation.workspace_name, "renamed");
        assert_eq!(conversation.id, "%1");
    }

    #[test]
    fn test_kind_not_downgraded_when_agent_runs_tool() {
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![pane("%1", "proj", "pi")]);
        // agent 执行 bash 工具时 pane_current_command 变成 bash，kind 不应被打回 Shell
        reg.refresh_panes(vec![pane("%1", "proj", "bash")]);
        assert_eq!(reg.get("%1").unwrap().kind, AgentKind::Pi);
    }

    #[test]
    fn test_disappeared_panes_are_pruned() {
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![pane("%1", "proj", "pi"), pane("%2", "proj", "pi")]);
        reg.refresh_panes(vec![pane("%1", "proj", "pi")]);
        assert_eq!(reg.list().len(), 1);
        assert!(reg.get("%2").is_none());
    }

    #[test]
    fn test_list_sorts_awaiting_human_first() {
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![
            pane("%1", "a", "pi"),
            pane("%2", "b", "pi"),
            pane("%3", "c", "pi"),
        ]);
        reg.conversations.get_mut("%3").unwrap().status = ConversationStatus::AwaitingHuman;
        reg.conversations.get_mut("%1").unwrap().status = ConversationStatus::Thinking;
        reg.conversations.get_mut("%2").unwrap().status = ConversationStatus::Idle;

        let listed = reg.list();
        assert_eq!(listed[0].id, "%3");
        assert_eq!(listed[1].id, "%2");
    }

    fn intercom_session(id: &str, pid: i64, cwd: &str, model: &str) -> SessionInfo {
        SessionInfo {
            id: id.into(),
            name: Some(id.into()),
            pid,
            cwd: cwd.into(),
            model: model.into(),
            status: Some("idle".into()),
            ..Default::default()
        }
    }

    #[test]
    fn legacy_panes_use_process_tree_without_model_substring_gates() {
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![pane("%1", "proj", "pi")]);
        let pane_pids = HashMap::from([(100, "%1".to_string())]);
        let parents = HashMap::from([(200, 100)]);
        let session = intercom_session("legacy", 200, "/different", "not-pi");
        reg.apply_intercom_snapshot(&[session], None, &pane_pids, &parents);
        assert_eq!(
            reg.get("%1").unwrap().intercom_session_id.as_deref(),
            Some("legacy")
        );
    }

    #[test]
    fn managed_claude_requires_id_cwd_and_exact_adapter_model() {
        let mut managed = pane("%1", "proj", "claude");
        managed.expected_intercom_id = Some("expected".into());
        managed.managed_claude_adapter = true;
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![managed]);
        let pane_pids = HashMap::from([(100, "%1".to_string())]);
        let parents = HashMap::from([(200, 100)]);

        for session in [
            intercom_session("wrong", 200, "/tmp/proj", "claude"),
            intercom_session("expected", 200, "/tmp/other", "claude"),
            intercom_session("expected", 200, "/tmp/proj", "pi"),
        ] {
            reg.apply_intercom_snapshot(&[session], None, &pane_pids, &parents);
            assert!(reg.get("%1").unwrap().intercom_session_id.is_none());
        }
        reg.apply_intercom_snapshot(
            &[intercom_session("expected", 200, "/tmp/proj", "claude")],
            None,
            &pane_pids,
            &parents,
        );
        assert_eq!(
            reg.get("%1").unwrap().intercom_session_id.as_deref(),
            Some("expected")
        );
    }

    #[test]
    fn duplicate_candidates_fail_closed_and_full_snapshot_clears_stale_route() {
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![pane("%1", "proj", "pi")]);
        let pane_pids = HashMap::from([(100, "%1".to_string())]);
        let parents = HashMap::from([(200, 100), (201, 100)]);
        reg.apply_intercom_snapshot(
            &[intercom_session("one", 200, "/tmp/proj", "pi")],
            None,
            &pane_pids,
            &parents,
        );
        assert_eq!(
            reg.get("%1").unwrap().intercom_session_id.as_deref(),
            Some("one")
        );
        reg.set_transcript_kind("%1", TranscriptKind::Structured);
        reg.apply_intercom_snapshot(
            &[
                intercom_session("one", 200, "/tmp/proj", "pi"),
                intercom_session("two", 201, "/tmp/proj", "pi"),
            ],
            None,
            &pane_pids,
            &parents,
        );
        assert!(reg.get("%1").unwrap().intercom_session_id.is_none());
        assert_eq!(
            reg.get("%1").unwrap().transcript_kind,
            TranscriptKind::Structured
        );
        assert!(reg.by_intercom_id("one").is_none());
        assert!(reg.by_intercom_id("two").is_none());
    }

    #[test]
    fn test_workspace_intercom_snapshot_isolation() {
        let mut reg = ConversationRegistry::new();
        let mut pane_a = pane("%1", "ws-a", "pi");
        pane_a.workspace_id = "ws-a".into();
        pane_a.workspace_name = "ws-a".into();

        let mut pane_b = pane("%2", "ws-b", "pi");
        pane_b.workspace_id = "ws-b".into();
        pane_b.workspace_name = "ws-b".into();

        reg.refresh_panes(vec![pane_a, pane_b]);

        let pane_pids = HashMap::from([(100, "%1".to_string()), (200, "%2".to_string())]);
        let parents = HashMap::from([(101, 100), (201, 200)]);

        let session_a = intercom_session("sess-a", 101, "/tmp/ws-a", "pi");
        let session_b = intercom_session("sess-b", 201, "/tmp/ws-b", "pi");

        reg.apply_workspace_intercom_snapshot("ws-a", &[session_a], None, &pane_pids, &parents);
        reg.apply_workspace_intercom_snapshot("ws-b", &[session_b], None, &pane_pids, &parents);

        assert_eq!(
            reg.get("%1").unwrap().intercom_session_id.as_deref(),
            Some("sess-a")
        );
        assert_eq!(
            reg.get("%2").unwrap().intercom_session_id.as_deref(),
            Some("sess-b")
        );
        assert_eq!(reg.by_intercom_id("sess-a").unwrap().id, "%1");
        assert_eq!(reg.by_intercom_id("sess-b").unwrap().id, "%2");

        // 对 ws-b 应用空 snapshot 只会重置 %2 和移除 sess-b 路由，绝不清空 ws-a 的 %1 与 sess-a 路由
        reg.apply_workspace_intercom_snapshot("ws-b", &[], None, &pane_pids, &parents);

        assert_eq!(
            reg.get("%1").unwrap().intercom_session_id.as_deref(),
            Some("sess-a")
        );
        assert_eq!(reg.by_intercom_id("sess-a").unwrap().id, "%1");
        assert!(reg.get("%2").unwrap().intercom_session_id.is_none());
        assert_eq!(reg.get("%2").unwrap().status, ConversationStatus::Unknown);
        assert!(reg.by_intercom_id("sess-b").is_none());
    }

    #[test]
    fn test_status_mapping() {
        assert_eq!(
            ConversationStatus::from(AgentStatus::Tool("bash".into())),
            ConversationStatus::RunningTool
        );
        assert_eq!(
            ConversationStatus::from(AgentStatus::Idle),
            ConversationStatus::Idle
        );
    }

    #[test]
    fn test_client_event_serialization_shape() {
        let ev = ClientEvent::StatusChanged {
            id: "%1".into(),
            status: ConversationStatus::AwaitingHuman,
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"type\":\"status-changed\""));
        assert!(s.contains("\"awaiting-human\""));

        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![pane("%1", "workspace", "pi")]);
        let s = serde_json::to_string(&ClientEvent::Conversations { items: reg.list() }).unwrap();
        assert!(s.contains("\"workspaceId\":\"workspace\""));
        assert!(s.contains("\"workspaceName\":\"workspace\""));
        assert!(s.contains("\"transcriptKind\":\"capture\""));
        assert!(!s.contains("workspace_id"));
        assert!(!s.contains("transcript_kind"));
    }

    #[test]
    fn test_client_command_roundtrip() {
        let json = r#"{"type":"say","id":"%3","text":"继续"}"#;
        match serde_json::from_str::<ClientCommand>(json).unwrap() {
            ClientCommand::Say { id, text } => {
                assert_eq!(id, "%3");
                assert_eq!(text, "继续");
            }
            _ => panic!("expected Say"),
        }
    }

    #[test]
    fn test_log_transport_records() {
        let mut t = LogTransport::default();
        t.emit(&ClientEvent::Error {
            message: "x".into(),
        })
        .unwrap();
        assert_eq!(t.events.len(), 1);
    }
}
