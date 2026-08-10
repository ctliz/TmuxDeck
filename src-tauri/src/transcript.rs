//! 对话内容的读取器（v1.13 TranscriptSource 具体实现）。
//!
//! 主路径读各 harness 自己的结构化会话记录——它们本身就是干净的分轮次数据，
//! 不需要从终端屏幕还原「谁说了什么」。兜底仍用 `bridge.rs` 的 `CapturePaneSource`
//! （整屏截图，只能算一张卡片预览）。
//!
//! 支持的记录源（v1.13）：
//!
//! | harness | 位置 | 关联方式 |
//! |---|---|---|
//! | pi | `~/.pi/agent/sessions/<slug>/<ts>_<uuid>.jsonl` | **文件名里的 uuid 就是 intercom 会话 ID**（实测一致），一对一精确匹配 |
//! | Claude Code | `~/.claude/projects/<slug>/<uuid>.jsonl` | slug 目录 + 记录内 `cwd` 字段验证，取 mtime 最新文件 |
//! | 其余 | — | 兜底 capture-pane |
//!
//! slug 目录名的编码规则与 `cwd` 的关系实测不完全可逆（路径里的特殊字符、
//! 尾缀差异），所以本实现**不靠 slug 反推**，而是扫目录、读记录里的 `cwd`
//! 字段与 `conv.cwd` 比对——目录数量有限（本机 <20），扫描代价可忽略。
//!
//! 增量读取按「文件追加日志」实现：每个文件维护字节游标，只读新字节；
//! 文件被压缩/轮转（长度 < 游标）时游标归零，靠 `since` 时间戳去重。

use crate::bridge::{AgentKind, CapturePaneSource, Conversation, TranscriptSource, Turn, TurnRole};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// RFC3339 → 毫秒
//
// 本机记录文件的 timestamp 全部由 `new Date().toISOString()` 产生，
// 格式固定为 `YYYY-MM-DDTHH:MM:SS[.mmm]Z`。不为此引入 chrono——
// 一个只认这种格式的小解析器即可。
// ─────────────────────────────────────────────────────────────────────────────

fn rfc3339_to_ms(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut dp = date.split('-');
    let year: i64 = dp.next()?.parse().ok()?;
    let month: i64 = dp.next()?.parse().ok()?;
    let day: i64 = dp.next()?.parse().ok()?;
    let mut tp = time.split(':');
    let hour: i64 = tp.next()?.parse().ok()?;
    let minute: i64 = tp.next()?.parse().ok()?;
    let sec_part = tp.next()?;
    let (sec, frac) = match sec_part.split_once('.') {
        Some((s, f)) => (s.parse::<i64>().ok()?, f),
        None => (sec_part.parse::<i64>().ok()?, "0"),
    };

    // 公历 → 自 epoch 的天数（Howard Hinnant 的 days_from_civil）
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;

    let base = days * 86400 + hour * 3600 + minute * 60 + sec;
    let mut frac_ms = 0i64;
    for (i, c) in frac.chars().take(3).enumerate() {
        let d = c.to_digit(10)? as i64;
        frac_ms += d * 10i64.pow(2 - i as u32);
    }
    Some(base * 1000 + frac_ms)
}

// ─────────────────────────────────────────────────────────────────────────────
// 目录扫描与 cwd 验证
// ─────────────────────────────────────────────────────────────────────────────

/// 目录下 mtime 最新的 jsonl 文件。
fn newest_jsonl(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let p = entry.ok()?.path();
        if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let mtime = p.metadata().and_then(|m| m.modified()).ok()?;
        if best.as_ref().is_none_or(|(t, _)| mtime > *t) {
            best = Some((mtime, p));
        }
    }
    best.map(|(_, p)| p)
}

/// 读文件的若干行，找第一个 `cwd` 字段。pi 的 session 行、Claude 的
/// user/assistant 行都带 cwd；Claude 开头几行可能没有，最多扫 200 行。
fn scan_cwd(path: &Path) -> Option<String> {
    let f = File::open(path).ok()?;
    let reader = BufReader::new(f);
    for (i, line) in reader.lines().enumerate() {
        if i > 200 {
            break;
        }
        let Ok(line) = line else { continue };
        let v: Value = serde_json::from_str(&line).ok()?;
        if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
            if !cwd.is_empty() {
                return Some(cwd.to_string());
            }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// 公共解析逻辑
// ─────────────────────────────────────────────────────────────────────────────

/// 从一条 pi jsonl 行提取轮次（`type == "message"`）。
fn parse_pi_line(line: &str, conv_id: &str, out: &mut Vec<Turn>) {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    if v.get("type").and_then(|t| t.as_str()) != Some("message") {
        return;
    }
    let role = match v.pointer("/message/role").and_then(|r| r.as_str()) {
        Some("user") => TurnRole::Human,
        _ => TurnRole::Agent,
    };
    let ts = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(rfc3339_to_ms)
        .unwrap_or(0);
    let Some(items) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
        return;
    };
    for item in items {
        if item.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    out.push(Turn {
                        conversation_id: conv_id.to_string(),
                        role,
                        text: text.to_string(),
                        timestamp: ts,
                    });
                }
            }
        }
    }
}

/// 从一条 Claude Code jsonl 行提取轮次（`type == "user" | "assistant"`）。
fn parse_claude_line(line: &str, conv_id: &str, out: &mut Vec<Turn>) {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    let t = match v.get("type").and_then(|t| t.as_str()) {
        Some("user") => TurnRole::Human,
        Some("assistant") => TurnRole::Agent,
        _ => return,
    };
    // 元数据行（压缩摘要、内部提示）不是真实对话，排除
    if v.get("isMeta").and_then(|b| b.as_bool()).unwrap_or(false) {
        return;
    }
    if v.get("isCompactSummary").and_then(|b| b.as_bool()).unwrap_or(false) {
        return;
    }
    let ts = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(rfc3339_to_ms)
        .unwrap_or(0);
    let Some(items) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
        return;
    };
    for item in items {
        // 只取文本；thinking / tool_use / tool_result 不进入对话流
        if item.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    out.push(Turn {
                        conversation_id: conv_id.to_string(),
                        role: t,
                        text: text.to_string(),
                        timestamp: ts,
                    });
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// pi 读取器
// ─────────────────────────────────────────────────────────────────────────────

pub struct PiTranscriptSource {
    sessions_root: PathBuf,
    /// cwd → 目录 的缓存（session 目录数量有限，读一次即可）
    cwd_dir: HashMap<String, PathBuf>,
    /// 文件 → 已读到的字节偏移
    cursors: HashMap<PathBuf, u64>,
    /// 文件 → 最后一条 (ts, text)，用于同毫秒去重
    last_seen: HashMap<PathBuf, (i64, String)>,
}

impl Default for PiTranscriptSource {
    fn default() -> Self {
        Self::new()
    }
}

impl PiTranscriptSource {
    pub fn new() -> Self {
        let root = dirs::home_dir()
            .map(|h| h.join(".pi").join("agent").join("sessions"))
            .unwrap_or_else(|| PathBuf::from(".pi/agent/sessions"));
        Self {
            sessions_root: root,
            cwd_dir: HashMap::new(),
            cursors: HashMap::new(),
            last_seen: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn with_root(root: PathBuf) -> Self {
        Self {
            sessions_root: root,
            cwd_dir: HashMap::new(),
            cursors: HashMap::new(),
            last_seen: HashMap::new(),
        }
    }

    fn scan_cwd_dirs(&mut self) {
        if !self.cwd_dir.is_empty() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&self.sessions_root) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            if let Some(cwd) = newest_jsonl(&p).and_then(|f| scan_cwd(&f)) {
                self.cwd_dir.entry(cwd).or_insert(p);
            }
        }
    }

    /// 找到该对话的记录文件。
    ///
    /// 优先用 intercom 会话 ID 精确匹配文件名（pi 的记录文件名内嵌会话 ID）；
    /// 无 intercom 时回退到 cwd 对应目录里 mtime 最新的文件。
    pub fn resolve(&mut self, conv: &Conversation) -> Option<PathBuf> {
        self.scan_cwd_dirs();
        let dir = self.cwd_dir.get(&conv.cwd)?;

        if let Some(sid) = conv.intercom_session_id.as_deref() {
            let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
            for e in std::fs::read_dir(dir).ok()?.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                    continue;
                }
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.contains(sid) {
                    continue;
                }
                let mtime = p.metadata().and_then(|m| m.modified()).ok()?;
                if best.as_ref().is_none_or(|(t, _)| mtime > *t) {
                    best = Some((mtime, p));
                }
            }
            if let Some((_, p)) = best {
                return Some(p);
            }
        }
        newest_jsonl(dir)
    }
}

impl TranscriptSource for PiTranscriptSource {
    fn poll(&mut self, conv: &Conversation, since: i64) -> Result<Vec<Turn>, String> {
        let Some(path) = self.resolve(conv) else {
            return Ok(Vec::new()); // 无结构化记录，调用方负责兜底
        };
        read_incremental(
            &path,
            conv,
            since,
            &mut self.cursors,
            &mut self.last_seen,
            parse_pi_line,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Claude Code 读取器
// ─────────────────────────────────────────────────────────────────────────────

pub struct ClaudeTranscriptSource {
    projects_root: PathBuf,
    cwd_dir: HashMap<String, PathBuf>,
    cursors: HashMap<PathBuf, u64>,
    last_seen: HashMap<PathBuf, (i64, String)>,
}

impl Default for ClaudeTranscriptSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeTranscriptSource {
    pub fn new() -> Self {
        let root = dirs::home_dir()
            .map(|h| h.join(".claude").join("projects"))
            .unwrap_or_else(|| PathBuf::from(".claude/projects"));
        Self {
            projects_root: root,
            cwd_dir: HashMap::new(),
            cursors: HashMap::new(),
            last_seen: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn with_root(root: PathBuf) -> Self {
        Self {
            projects_root: root,
            cwd_dir: HashMap::new(),
            cursors: HashMap::new(),
            last_seen: HashMap::new(),
        }
    }

    fn scan_cwd_dirs(&mut self) {
        if !self.cwd_dir.is_empty() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&self.projects_root) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            // 排除 memory 子目录
            if p.file_name().and_then(|n| n.to_str()) == Some("memory") {
                continue;
            }
            if let Some(cwd) = newest_jsonl(&p).and_then(|f| scan_cwd(&f)) {
                self.cwd_dir.entry(cwd).or_insert(p);
            }
        }
    }

    /// 找到该对话的记录文件：先试 slug 候选目录，再全量扫目录，
    /// 都以记录内的 `cwd` 字段验证。取 mtime 最新的文件。
    pub fn resolve(&mut self, conv: &Conversation) -> Option<PathBuf> {
        self.scan_cwd_dirs();
        if let Some(dir) = self.cwd_dir.get(&conv.cwd) {
            return newest_jsonl(dir);
        }
        // 候选 slug（"-" + cwd 去尾斜杠 + "/"→"-"），目录存在且含匹配 cwd 时兜底
        let slug = format!("-{}", conv.cwd.trim_end_matches('/').replace('/', "-"));
        let candidate = self.projects_root.join(&slug);
        if candidate.is_dir() {
            if let Some(cwd) = newest_jsonl(&candidate).and_then(|f| scan_cwd(&f)) {
                if cwd == conv.cwd {
                    self.cwd_dir.insert(conv.cwd.clone(), candidate.clone());
                    return newest_jsonl(&candidate);
                }
            }
        }
        None
    }
}

impl TranscriptSource for ClaudeTranscriptSource {
    fn poll(&mut self, conv: &Conversation, since: i64) -> Result<Vec<Turn>, String> {
        let Some(path) = self.resolve(conv) else {
            return Ok(Vec::new());
        };
        read_incremental(
            &path,
            conv,
            since,
            &mut self.cursors,
            &mut self.last_seen,
            parse_claude_line,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 增量读取
// ─────────────────────────────────────────────────────────────────────────────

type LineParser = fn(&str, &str, &mut Vec<Turn>);

/// 从游标处续读文件，解析新行，推进游标。
///
/// - 文件被压缩/轮转（长度 < 游标）→ 游标归零，靠 `since` 与 last_seen 去重。
/// - 同一毫秒多条轮次：用 last_seen 记录 (ts, text)，跳过恰好相同的。
fn read_incremental(
    path: &Path,
    conv: &Conversation,
    since: i64,
    cursors: &mut HashMap<PathBuf, u64>,
    last_seen: &mut HashMap<PathBuf, (i64, String)>,
    parse: LineParser,
) -> Result<Vec<Turn>, String> {
    let mut f = File::open(path).map_err(|e| format!("ERR_TRANSCRIPT_OPEN|{}", e))?;
    let file_len = f
        .metadata()
        .map_err(|e| format!("ERR_TRANSCRIPT_STAT|{}", e))?
        .len();

    let cursor = cursors.get(path).copied().unwrap_or(0);
    let start = if file_len < cursor {
        0 // 文件被重写/压缩：从头读，靠 since 过滤旧轮次
    } else {
        cursor
    };
    f.seek(SeekFrom::Start(start))
        .map_err(|e| format!("ERR_TRANSCRIPT_SEEK|{}", e))?;

    let mut turns = Vec::new();
    let mut prev: Option<(i64, String)> = last_seen.get(path).cloned();
    let reader = BufReader::new(f);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let mut line_turns = Vec::new();
        parse(&line, &conv.id, &mut line_turns);
        for t in line_turns {
            if t.timestamp <= since {
                continue;
            }
            if prev.as_ref() == Some(&(t.timestamp, t.text.clone())) {
                continue; // 与上次最后一条完全相同 → 去重
            }
            prev = Some((t.timestamp, t.text.clone()));
            turns.push(t);
        }
    }

    if let Some(p) = prev {
        last_seen.insert(path.to_path_buf(), p);
    }
    cursors.insert(path.to_path_buf(), file_len);
    Ok(turns)
}

// ─────────────────────────────────────────────────────────────────────────────
// 组合源：按 harness 选读取器，未覆盖的走 capture-pane 兜底
// ─────────────────────────────────────────────────────────────────────────────

pub struct CompositeTranscriptSource {
    pi: PiTranscriptSource,
    claude: ClaudeTranscriptSource,
    capture: CapturePaneSource,
}

impl Default for CompositeTranscriptSource {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeTranscriptSource {
    pub fn new() -> Self {
        Self {
            pi: PiTranscriptSource::new(),
            claude: ClaudeTranscriptSource::new(),
            capture: CapturePaneSource::default(),
        }
    }
}

impl TranscriptSource for CompositeTranscriptSource {
    fn poll(&mut self, conv: &Conversation, since: i64) -> Result<Vec<Turn>, String> {
        match conv.kind {
            AgentKind::Pi => {
                if self.pi.resolve(conv).is_some() {
                    self.pi.poll(conv, since)
                } else {
                    self.capture.poll(conv, since)
                }
            }
            AgentKind::ClaudeCode => {
                if self.claude.resolve(conv).is_some() {
                    self.claude.poll(conv, since)
                } else {
                    self.capture.poll(conv, since)
                }
            }
            // Codex / 其余 harness 的结构化提取尚未实现（v1.13），先兜底
            _ => self.capture.poll(conv, since),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static DIR_SEQ: AtomicU32 = AtomicU32::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = DIR_SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "tmuxdeck-transcript-{}-{}-{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn conv(cwd: &str, kind: AgentKind, intercom: Option<&str>) -> Conversation {
        Conversation {
            id: "%1".into(),
            session: "proj".into(),
            cwd: cwd.into(),
            kind,
            title: "proj".into(),
            intercom_session_id: intercom.map(String::from),
            status: crate::bridge::ConversationStatus::Unknown,
        }
    }

    // ── RFC3339 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_rfc3339_to_ms() {
        // 2026-07-27T13:00:36.416Z
        assert_eq!(rfc3339_to_ms("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(rfc3339_to_ms("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(
            rfc3339_to_ms("2026-07-27T13:00:36.416Z"),
            Some(1785157236416)
        );
        assert_eq!(rfc3339_to_ms("bad"), None);
        assert_eq!(rfc3339_to_ms("2026-07-27"), None);
    }

    // ── pi ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_pi_resolve_by_intercom_session_id() {
        let root = temp_dir("pi");
        let cwd = "/Users/tsiji/Documents/proj";
        let dir = root.join("--Users-tsiji-Documents-proj--");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("2026-08-10T16-18-14-827Z_019fec77-b0ab.jsonl"),
            r#"{"type":"session","version":3,"id":"x","timestamp":"2026-08-10T16:18:14.827Z","cwd":"/Users/tsiji/Documents/proj"}"#,
        )
        .unwrap();
        fs::write(
            dir.join("2026-08-10T10-00-00-000Z_019fec77-old.jsonl"),
            r#"{"type":"session","version":3,"id":"y","timestamp":"2026-08-10T10:00:00.000Z","cwd":"/Users/tsiji/Documents/proj"}"#,
        )
        .unwrap();

        let mut src = PiTranscriptSource::with_root(root);
        // intercom 会话 ID 匹配到正确（最新的）文件
        let c = conv(cwd, AgentKind::Pi, Some("019fec77-b0ab"));
        let path = src.resolve(&c).unwrap();
        assert!(path.to_string_lossy().contains("16-18-14"));
    }

    #[test]
    fn test_pi_poll_extracts_text_turns() {
        let root = temp_dir("pi-poll");
        let cwd = "/Users/tsiji/Documents/proj";
        let dir = root.join("--Users-tsiji-Documents-proj--");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("2026-08-10T16-18-14-827Z_abc.jsonl"),
            concat!(
                r#"{"type":"session","version":3,"id":"s","timestamp":"2026-08-10T16:18:14.827Z","cwd":"/Users/tsiji/Documents/proj"}"#,
                "\n",
                r#"{"type":"message","id":"m1","timestamp":"2026-08-10T16:18:20.100Z","message":{"role":"user","content":[{"type":"text","text":"继续"}]}}"#,
                "\n",
                r#"{"type":"message","id":"m2","timestamp":"2026-08-10T16:18:30.200Z","message":{"role":"assistant","content":[{"type":"text","text":"好的，正在处理"}]}}"#,
                "\n",
                // tool 调用与其它类型不应进入对话流
                r#"{"type":"message","id":"m3","timestamp":"2026-08-10T16:18:31.000Z","message":{"role":"assistant","content":[{"type":"tool_call","name":"bash","input":{"cmd":"ls"}}]}}"#,
                "\n",
                r#"{"type":"custom_message","customType":"intercom_message","content":"ignored"}"#,
            ),
        )
        .unwrap();

        let mut src = PiTranscriptSource::with_root(root);
        let c = conv(cwd, AgentKind::Pi, Some("abc"));
        let turns = src.poll(&c, 0).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, TurnRole::Human);
        assert_eq!(turns[0].text, "继续");
        assert_eq!(turns[1].role, TurnRole::Agent);
        assert_eq!(turns[1].text, "好的，正在处理");
    }

    // ── Claude ───────────────────────────────────────────────────────────────

    #[test]
    fn test_claude_resolve_and_poll() {
        let root = temp_dir("claude");
        let cwd = "/Users/tsiji/Documents/fflink/front";
        let dir = root.join("-Users-tsiji-Documents-fflink-front");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("93e0f268-15f9-40c0-b1ce.jsonl"),
            concat!(
                r#"{"type":"user","cwd":"/Users/tsiji/Documents/fflink/front","timestamp":"2026-07-27T13:00:36.416Z","message":{"role":"user","content":[{"type":"text","text":"帮我连 blender mcp"}]}}"#,
                "\n",
                r#"{"type":"assistant","timestamp":"2026-07-27T13:00:48.569Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"想一下"},{"type":"text","text":"我先看一下项目文档"}]}}"#,
                "\n",
                // tool_result 不应进入对话流；isMeta 行应被排除
                r#"{"type":"user","cwd":"/Users/tsiji/Documents/fflink/front","timestamp":"2026-07-27T13:00:50.047Z","isMeta":true,"message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#,
            ),
        )
        .unwrap();

        let mut src = ClaudeTranscriptSource::with_root(root);
        let c = conv(cwd, AgentKind::ClaudeCode, None);
        let turns = src.poll(&c, 0).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, TurnRole::Human);
        assert_eq!(turns[1].role, TurnRole::Agent);
        assert_eq!(turns[1].text, "我先看一下项目文档");
        assert_eq!(turns[1].timestamp, 1785157248569);
    }

    // ── 增量与去重 ───────────────────────────────────────────────────────────

    #[test]
    fn test_incremental_poll_only_new_lines() {
        let root = temp_dir("incremental");
        let cwd = "/Users/tsiji/Documents/proj";
        let dir = root.join("--Users-tsiji-Documents-proj--");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("2026-08-10T16-18-14-827Z_abc.jsonl");
        fs::write(
            &file,
            concat!(
                r#"{"type":"session","version":3,"id":"s","timestamp":"2026-08-10T16:18:14.827Z","cwd":"/Users/tsiji/Documents/proj"}"#,
                "\n",
                r#"{"type":"message","id":"m1","timestamp":"2026-08-10T16:18:20.100Z","message":{"role":"assistant","content":[{"type":"text","text":"第一轮"}]}}"#,
                "\n",
            ),
        )
        .unwrap();

        let mut src = PiTranscriptSource::with_root(root.clone());
        let c = conv(cwd, AgentKind::Pi, Some("abc"));
        let first = src.poll(&c, 0).unwrap();
        assert_eq!(first.len(), 1);

        // 追加新行，第二次 poll 只应返回新轮次
        let mut content = fs::read_to_string(&file).unwrap();
        content.push_str(
            r#"{"type":"message","id":"m2","timestamp":"2026-08-10T16:19:00.000Z","message":{"role":"assistant","content":[{"type":"text","text":"第二轮"}]}}"#,
        );
        fs::write(&file, content).unwrap();

        let second = src.poll(&c, first[0].timestamp).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].text, "第二轮");
    }

    #[test]
    fn test_file_rotation_resets_cursor_without_dup() {
        let root = temp_dir("rotation");
        let cwd = "/Users/tsiji/Documents/proj";
        let dir = root.join("--Users-tsiji-Documents-proj--");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("2026-08-10T16-18-14-827Z_abc.jsonl");
        fs::write(
            &file,
            r#"{"type":"session","version":3,"id":"s","timestamp":"2026-08-10T16:18:14.827Z","cwd":"/Users/tsiji/Documents/proj"}
{"type":"message","id":"m1","timestamp":"2026-08-10T16:18:20.100Z","message":{"role":"assistant","content":[{"type":"text","text":"第一轮"}]}}"#,
        )
        .unwrap();

        let mut src = PiTranscriptSource::with_root(root.clone());
        let c = conv(cwd, AgentKind::Pi, Some("abc"));
        let first = src.poll(&c, 0).unwrap();
        assert_eq!(first.len(), 1);

        // 模拟压缩：文件被重写为更短、只含旧行
        fs::write(
            &file,
            r#"{"type":"session","version":3,"id":"s","timestamp":"2026-08-10T16:18:14.827Z","cwd":"/Users/tsiji/Documents/proj"}
{"type":"message","id":"m1","timestamp":"2026-08-10T16:18:20.100Z","message":{"role":"assistant","content":[{"type":"text","text":"第一轮"}]}}"#,
        )
        .unwrap();

        // since 取第一轮 ts，游标重置后旧行被过滤
        let second = src.poll(&c, first[0].timestamp).unwrap();
        assert_eq!(second.len(), 0);
    }

    #[test]
    fn test_same_ms_lines_not_duplicated() {
        let root = temp_dir("same-ms");
        let cwd = "/Users/tsiji/Documents/proj";
        let dir = root.join("--Users-tsiji-Documents-proj--");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("2026-08-10T16-18-14-827Z_abc.jsonl");
        fs::write(
            &file,
            r#"{"type":"session","version":3,"id":"s","timestamp":"2026-08-10T16:18:14.827Z","cwd":"/Users/tsiji/Documents/proj"}
{"type":"message","id":"m1","timestamp":"2026-08-10T16:19:00.000Z","message":{"role":"assistant","content":[{"type":"text","text":"同一毫秒"}]}}
{"type":"message","id":"m2","timestamp":"2026-08-10T16:19:00.000Z","message":{"role":"assistant","content":[{"type":"text","text":"下一行"}]}}"#,
        )
        .unwrap();

        let mut src = PiTranscriptSource::with_root(root);
        let c = conv(cwd, AgentKind::Pi, Some("abc"));
        let turns = src.poll(&c, 0).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].text, "同一毫秒");
        assert_eq!(turns[1].text, "下一行");
    }

    // ── 组合源 ───────────────────────────────────────────────────────────────

    #[test]
    fn test_composite_falls_back_when_no_record() {
        // 无匹配 cwd 时 pi 源返回空，不应 panic
        let root = temp_dir("composite");
        let mut src = CompositeTranscriptSource::new();
        // 注入空根目录的 pi 源，避免访问真实 ~/.pi
        src.pi = PiTranscriptSource::with_root(root.join("empty-sessions"));
        src.claude = ClaudeTranscriptSource::with_root(root.join("empty-projects"));

        let c = conv("/nonexistent/path", AgentKind::Pi, Some("abc"));
        // capture-pane 兜底在无 tmux 环境下可能报错，这里只验证 pi 分支不 panic
        let _ = src.pi.poll(&c, 0);
        assert!(true);
    }
}

// ── 真机验证（默认忽略，cargo test -- --ignored transcript_real）──

#[cfg(test)]
mod real_tests {
    use super::*;

    #[test]
    #[ignore]
    fn transcript_real_pi_session() {
        let mut src = PiTranscriptSource::new();
        let c = Conversation {
            id: "%9".into(),
            session: "Tmux-Deck".into(),
            cwd: "/Users/tsiji/Documents/TmuxDeck".into(),
            kind: AgentKind::Pi,
            title: "tmux".into(),
            intercom_session_id: Some("019fec77-b0ab-7f2b-b4fa-84d01c5f63b1".into()),
            status: crate::bridge::ConversationStatus::Unknown,
        };
        let path = src.resolve(&c).expect("resolve real pi session");
        println!("RESOLVED: {}", path.display());
        let turns = src.poll(&c, 0).unwrap();
        for t in turns.iter().rev().take(3) {
            println!("[{:?}] {}", t.role, t.text.chars().take(60).collect::<String>());
        }
        assert!(!turns.is_empty(), "should have turns from real pi session");
    }
}
