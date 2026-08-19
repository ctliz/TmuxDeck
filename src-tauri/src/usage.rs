//! Agent token 用量采集。
//!
//! 全部数据来自本机日志，零网络请求、零费用估算：
//!   - Codex      ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl  (token_count 事件，会话内累计值)
//!   - Claude Code ~/.claude/projects/<slug>/*.jsonl            (message.usage，逐条)
//!   - Pi         ~/.pi/agent/sessions/<cwd-slug>/*.jsonl       (message.usage.totalTokens，逐条)
//!   - opencode   ~/.local/share/opencode/opencode.db           (SQLite；storage/ 下的 JSON 已废弃)
//!
//! 采集在后台线程进行，结果存进程内快照；前端通过 get_usage_snapshot 读快照，永不阻塞。

use crate::config::get_config_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// 统计窗口。只扫这个窗口内有改动的文件，避免每次都碰全部历史（Codex 全量近 1GB）。
const WINDOW_DAYS: i64 = 30;
const DAY_SECS: i64 = 86_400;

// ---------------------------------------------------------------------------
// 对外数据结构
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsage {
    pub agent_id: String,
    pub display_name: String,
    pub today_tokens: u64,
    pub tokens_30d: u64,
    pub sessions_30d: u32,
    pub last_active_ts: Option<i64>,
    /// 数据源不存在 / 不可读时为 false，前端渲染「未检测到」空态，不影响其他 Agent。
    pub available: bool,
}

impl AgentUsage {
    fn unavailable(agent_id: &str, display_name: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            display_name: display_name.to_string(),
            available: false,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub agents: Vec<AgentUsage>,
    pub total_today: u64,
    pub total_30d: u64,
    /// 采集完成时间（unix 秒）。为 0 表示首轮采集尚未结束。
    pub updated_at: i64,
    /// 本轮采集耗时，用于排查性能回归。
    pub elapsed_ms: u64,
}

// ---------------------------------------------------------------------------
// 时间工具（不引入 chrono）
// ---------------------------------------------------------------------------

/// 本地 UTC 偏移（秒）。向系统 `date +%z` 取一次并在进程内缓存；
/// Windows 或取值失败时回退 0（退化为按 UTC 分天）。
fn local_offset_secs() -> i64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(|| {
        if cfg!(target_os = "windows") {
            return 0;
        }
        Command::new("date")
            .arg("+%z")
            .output()
            .ok()
            .and_then(|out| parse_utc_offset(String::from_utf8_lossy(&out.stdout).trim()))
            .unwrap_or(0)
    })
}

/// 解析 `+0800` / `-0530` 形式的偏移。
fn parse_utc_offset(s: &str) -> Option<i64> {
    let sign = match s.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let h: i64 = s.get(1..3)?.parse().ok()?;
    let m: i64 = s.get(3..5)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

/// 民用日期 -> 距 1970-01-01 的天数（Howard Hinnant days_from_civil）。
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 解析定宽 ISO8601 UTC 串（`2026-08-12T04:26:31.928Z`）为 unix 秒。
fn parse_iso_secs(s: &str) -> Option<i64> {
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19)?.parse().ok()?;
    Some(days_from_civil(y, mo, d) * DAY_SECS + h * 3600 + mi * 60 + se)
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 把 unix 秒折算成「本地日序号」。分桶只需要整数天，不需要日历运算。
fn day_index(ts_secs: i64) -> i64 {
    (ts_secs + local_offset_secs()).div_euclid(DAY_SECS)
}

fn mtime_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 增量缓存
// ---------------------------------------------------------------------------

/// 单个日志文件的解析结果。mtime + size 同时未变即视为命中，跳过重新解析。
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct FileStat {
    mtime: i64,
    size: u64,
    /// 日序号（字符串，因 JSON 对象键必须是字符串）-> token 数
    daily: HashMap<String, u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct UsageCache {
    files: HashMap<String, FileStat>,
}

fn cache_path() -> PathBuf {
    get_config_dir().join("usage-cache.json")
}

fn load_cache() -> UsageCache {
    std::fs::read_to_string(cache_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &UsageCache) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

// ---------------------------------------------------------------------------
// 文件扫描
// ---------------------------------------------------------------------------

/// 朴素子串查找。只用于在整行 JSON 里快速排除不含标记的行，避免对每行都做 JSON 解析。
fn contains_sub(hay: &[u8], needle: &[u8]) -> bool {
    let n = needle.len();
    if n == 0 || hay.len() < n {
        return false;
    }
    let first = needle[0];
    let mut i = 0;
    while i + n <= hay.len() {
        match hay[i..=hay.len() - n].iter().position(|&b| b == first) {
            Some(off) => {
                let start = i + off;
                if &hay[start..start + n] == needle {
                    return true;
                }
                i = start + 1;
            }
            None => return false,
        }
    }
    false
}

/// 逐行读取，只把含 `marker` 的行交给回调。按字节读，避免非 UTF-8 行导致整个文件失败。
fn scan_lines<F: FnMut(&str)>(path: &Path, marker: &[u8], mut on_line: F) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let mut reader = BufReader::with_capacity(256 * 1024, file);
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if !contains_sub(&buf, marker) {
            continue;
        }
        if let Ok(s) = std::str::from_utf8(&buf) {
            on_line(s);
        }
    }
}

/// 递归收集匹配的文件。`depth` 上限防止意外深目录拖慢启动。
fn walk(dir: &Path, depth: u32, keep: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => walk(&path, depth - 1, keep, out),
            Ok(ft) if ft.is_file() && keep(&path) => out.push(path),
            _ => {}
        }
    }
}

fn is_jsonl(path: &Path) -> bool {
    path.extension().map(|e| e == "jsonl").unwrap_or(false)
}

// ---------------------------------------------------------------------------
// 各 Agent 的单文件解析
// ---------------------------------------------------------------------------

/// Codex：`token_count` 事件里的 `total_token_usage.total_tokens` 是**会话内累计值**，
/// 因此按事件做差分才能得到当天真实增量；直接取末条会把整个会话压到一天上。
/// 会话内计数器回退（compact 后重置）时，按新值整体计入。
fn parse_codex(path: &Path) -> HashMap<i64, u64> {
    let mut daily: HashMap<i64, u64> = HashMap::new();
    let mut prev: u64 = 0;
    scan_lines(path, b"\"token_count\"", |line| {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        let Some(cur) = v["payload"]["info"]["total_token_usage"]["total_tokens"].as_u64() else {
            return;
        };
        let Some(ts) = v["timestamp"].as_str().and_then(parse_iso_secs) else {
            return;
        };
        let delta = if cur >= prev { cur - prev } else { cur };
        prev = cur;
        if delta > 0 {
            *daily.entry(day_index(ts)).or_default() += delta;
        }
    });
    daily
}

/// Claude Code：每条 assistant 消息的 usage 是本次调用的量，直接累加。
fn parse_claude(path: &Path) -> HashMap<i64, u64> {
    let mut daily: HashMap<i64, u64> = HashMap::new();
    scan_lines(path, b"\"usage\"", |line| {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        if v["type"].as_str() != Some("assistant") {
            return;
        }
        let u = &v["message"]["usage"];
        let tokens = u["input_tokens"].as_u64().unwrap_or(0)
            + u["output_tokens"].as_u64().unwrap_or(0)
            + u["cache_creation_input_tokens"].as_u64().unwrap_or(0)
            + u["cache_read_input_tokens"].as_u64().unwrap_or(0);
        if tokens == 0 {
            return;
        }
        let Some(ts) = v["timestamp"].as_str().and_then(parse_iso_secs) else {
            return;
        };
        *daily.entry(day_index(ts)).or_default() += tokens;
    });
    daily
}

/// Pi：assistant 消息带 `usage.totalTokens`；缺失时回退到分项求和。
/// 时间优先用 `message.timestamp`（epoch 毫秒），回退到记录级 ISO 串。
fn parse_pi(path: &Path) -> HashMap<i64, u64> {
    let mut daily: HashMap<i64, u64> = HashMap::new();
    scan_lines(path, b"\"usage\"", |line| {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        let msg = &v["message"];
        if msg["role"].as_str() != Some("assistant") {
            return;
        }
        let u = &msg["usage"];
        let tokens = match u["totalTokens"].as_u64() {
            Some(t) if t > 0 => t,
            _ => {
                u["input"].as_u64().unwrap_or(0)
                    + u["output"].as_u64().unwrap_or(0)
                    + u["cacheRead"].as_u64().unwrap_or(0)
                    + u["cacheWrite"].as_u64().unwrap_or(0)
            }
        };
        if tokens == 0 {
            return;
        }
        let ts = msg["timestamp"]
            .as_i64()
            .map(|ms| ms / 1000)
            .or_else(|| v["timestamp"].as_str().and_then(parse_iso_secs));
        let Some(ts) = ts else { return };
        *daily.entry(day_index(ts)).or_default() += tokens;
    });
    daily
}

// ---------------------------------------------------------------------------
// 基于文件的采集（Codex / Claude / Pi）
// ---------------------------------------------------------------------------

struct FileAgentSpec {
    agent_id: &'static str,
    display_name: &'static str,
    root: PathBuf,
    depth: u32,
    keep: fn(&Path) -> bool,
    parse: fn(&Path) -> HashMap<i64, u64>,
}

fn collect_file_agent(spec: &FileAgentSpec, cache: &mut UsageCache) -> AgentUsage {
    if !spec.root.is_dir() {
        return AgentUsage::unavailable(spec.agent_id, spec.display_name);
    }

    let today = day_index(now_secs());
    let window_start = today - WINDOW_DAYS + 1;
    // 只有窗口内改动过的文件才可能贡献窗口内的用量，其余直接跳过（Codex 靠这条把近 1GB 削到 ~600MB）。
    let mtime_floor = (window_start - 1) * DAY_SECS - local_offset_secs();

    let mut files = Vec::new();
    walk(&spec.root, spec.depth, &|p| (spec.keep)(p), &mut files);

    let mut today_tokens = 0u64;
    let mut tokens_30d = 0u64;
    let mut sessions = 0u32;
    let mut last_active: Option<i64> = None;

    for path in files {
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        let mtime = mtime_secs(&meta);
        if mtime < mtime_floor {
            continue;
        }
        let size = meta.len();
        let key = path.to_string_lossy().to_string();

        let hit = cache
            .files
            .get(&key)
            .filter(|c| c.mtime == mtime && c.size == size);

        let daily: HashMap<String, u64> = match hit {
            Some(cached) => cached.daily.clone(),
            None => {
                let parsed = (spec.parse)(&path);
                let daily: HashMap<String, u64> = parsed
                    .into_iter()
                    .map(|(day, tok)| (day.to_string(), tok))
                    .collect();
                cache.files.insert(
                    key,
                    FileStat {
                        mtime,
                        size,
                        daily: daily.clone(),
                    },
                );
                daily
            }
        };

        let mut file_window_tokens = 0u64;
        for (day, tok) in &daily {
            let Ok(day) = day.parse::<i64>() else {
                continue;
            };
            if day < window_start || day > today {
                continue;
            }
            file_window_tokens += tok;
            if day == today {
                today_tokens += tok;
            }
        }
        if file_window_tokens > 0 {
            tokens_30d += file_window_tokens;
            sessions += 1;
            last_active = Some(last_active.map_or(mtime, |v: i64| v.max(mtime)));
        }
    }

    AgentUsage {
        agent_id: spec.agent_id.to_string(),
        display_name: spec.display_name.to_string(),
        today_tokens,
        tokens_30d,
        sessions_30d: sessions,
        last_active_ts: last_active,
        available: true,
    }
}

// ---------------------------------------------------------------------------
// opencode（SQLite）
// ---------------------------------------------------------------------------

fn opencode_db_path() -> Option<PathBuf> {
    // macOS 上 dirs::data_dir() 是 ~/Library/Application Support，而 opencode 实际写在
    // ~/.local/share 下，所以两处都要试，并且以「文件真实存在」为准而非以目录存在为准。
    [
        dirs::home_dir().map(|h| h.join(".local/share/opencode/opencode.db")),
        dirs::data_dir().map(|d| d.join("opencode").join("opencode.db")),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.is_file())
}

/// opencode 自 SQLite 迁移后，`~/.local/share/opencode/storage/` 下的 JSON 已停写，不能作为数据源。
/// 这里 shell-out 调系统 `sqlite3`（项目本就大量 shell-out），避免为单一 Agent 引入 rusqlite 的
/// bundled SQLite 编译负担；找不到 sqlite3 时优雅降级为「未检测到」。
fn collect_opencode() -> AgentUsage {
    const ID: &str = "opencode";
    const NAME: &str = "opencode";

    let Some(db) = opencode_db_path() else {
        return AgentUsage::unavailable(ID, NAME);
    };
    let Some(sqlite) = crate::registry::find_binary(
        "sqlite3",
        &[
            "/usr/bin/sqlite3",
            "/opt/homebrew/bin/sqlite3",
            "/usr/local/bin/sqlite3",
        ],
    ) else {
        return AgentUsage::unavailable(ID, NAME);
    };

    let today = day_index(now_secs());
    let window_start = today - WINDOW_DAYS + 1;
    let since_ms = (window_start * DAY_SECS - local_offset_secs()) * 1000;
    let offset = local_offset_secs();

    // 直接在 SQL 里折算成与 day_index 一致的日序号，省去回来再分桶。
    let query = format!(
        "select cast((time_created/1000 + {offset})/{DAY_SECS} as integer) d, \
                sum(coalesce(json_extract(data,'$.tokens.input'),0) \
                  + coalesce(json_extract(data,'$.tokens.output'),0) \
                  + coalesce(json_extract(data,'$.tokens.cache.read'),0) \
                  + coalesce(json_extract(data,'$.tokens.cache.write'),0)) tok, \
                count(distinct session_id) sess \
         from message \
         where json_extract(data,'$.role')='assistant' and time_created >= {since_ms} \
         group by d;"
    );

    let Ok(out) = Command::new(&sqlite)
        .arg("-readonly")
        .arg(&db)
        .arg(&query)
        .output()
    else {
        return AgentUsage::unavailable(ID, NAME);
    };
    if !out.status.success() {
        return AgentUsage::unavailable(ID, NAME);
    }

    let mut today_tokens = 0u64;
    let mut tokens_30d = 0u64;
    let mut sessions = 0u32;
    let mut last_active_day: Option<i64> = None;

    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.split('|');
        let (Some(day), Some(tok), Some(sess)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(day), Ok(tok), Ok(sess)) =
            (day.parse::<i64>(), tok.parse::<u64>(), sess.parse::<u32>())
        else {
            continue;
        };
        if day < window_start || day > today || tok == 0 {
            continue;
        }
        tokens_30d += tok;
        sessions += sess;
        if day == today {
            today_tokens += tok;
        }
        last_active_day = Some(last_active_day.map_or(day, |v: i64| v.max(day)));
    }

    AgentUsage {
        agent_id: ID.to_string(),
        display_name: NAME.to_string(),
        today_tokens,
        tokens_30d,
        sessions_30d: sessions,
        // 只有日粒度，取当天起始作为近似活跃时间。
        last_active_ts: last_active_day.map(|d| d * DAY_SECS - local_offset_secs()),
        available: true,
    }
}

// ---------------------------------------------------------------------------
// 采集入口与进程内快照
// ---------------------------------------------------------------------------

fn snapshot_slot() -> &'static Mutex<UsageSnapshot> {
    static SNAPSHOT: OnceLock<Mutex<UsageSnapshot>> = OnceLock::new();
    SNAPSHOT.get_or_init(|| Mutex::new(UsageSnapshot::default()))
}

fn home_join(rel: &str) -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(rel)
}

/// 全量采集（耗时操作，只应在后台线程调用）。
pub fn collect_usage() -> UsageSnapshot {
    let started = std::time::Instant::now();
    let mut cache = load_cache();

    let specs = [
        FileAgentSpec {
            agent_id: "codex",
            display_name: "Codex",
            root: home_join(".codex/sessions"),
            // sessions/YYYY/MM/DD/rollout-*.jsonl
            depth: 5,
            keep: |p| {
                is_jsonl(p)
                    && p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("rollout-"))
                        .unwrap_or(false)
            },
            parse: parse_codex,
        },
        FileAgentSpec {
            agent_id: "claude",
            display_name: "Claude Code",
            root: home_join(".claude/projects"),
            depth: 2,
            keep: is_jsonl,
            parse: parse_claude,
        },
        FileAgentSpec {
            agent_id: "pi",
            display_name: "Pi",
            root: home_join(".pi/agent/sessions"),
            depth: 2,
            keep: is_jsonl,
            parse: parse_pi,
        },
    ];

    let mut agents: Vec<AgentUsage> = specs
        .iter()
        .map(|spec| collect_file_agent(spec, &mut cache))
        .collect();
    #[cfg(not(target_os = "windows"))]
    agents.push(collect_opencode());
    #[cfg(target_os = "windows")]
    agents.push(AgentUsage::unavailable("opencode", "opencode"));

    // 丢弃已不存在的文件，防止缓存无限增长。
    cache.files.retain(|path, _| Path::new(path).is_file());
    save_cache(&cache);

    let total_today = agents.iter().map(|a| a.today_tokens).sum();
    let total_30d = agents.iter().map(|a| a.tokens_30d).sum();

    UsageSnapshot {
        agents,
        total_today,
        total_30d,
        updated_at: now_secs(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

/// 采集并写入进程内快照。返回新快照。
pub fn refresh_usage_snapshot() -> UsageSnapshot {
    let snapshot = collect_usage();
    if let Ok(mut slot) = snapshot_slot().lock() {
        *slot = snapshot.clone();
    }
    snapshot
}

/// 读取最近一次采集结果。首轮采集完成前返回空快照（`updated_at == 0`），不阻塞前端。
#[tauri::command]
pub fn get_usage_snapshot() -> UsageSnapshot {
    snapshot_slot()
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_file(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("tmuxdeck-usage-test-{name}"));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    /// 锁死上送给前端的字段名。`tokens_30d` 这类带数字的字段 camelCase 结果不直观，
    /// 与 src/types.ts 的 UsageSnapshot 必须逐字一致，否则前端静默读到 undefined。
    #[test]
    fn snapshot_serializes_with_expected_field_names() {
        let snapshot = UsageSnapshot {
            agents: vec![AgentUsage {
                agent_id: "codex".into(),
                display_name: "Codex".into(),
                today_tokens: 1,
                tokens_30d: 2,
                sessions_30d: 3,
                last_active_ts: Some(4),
                available: true,
            }],
            total_today: 1,
            total_30d: 2,
            updated_at: 5,
            elapsed_ms: 6,
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["totalToday"], 1);
        assert_eq!(json["total30d"], 2);
        assert_eq!(json["updatedAt"], 5);
        assert_eq!(json["elapsedMs"], 6);
        let agent = &json["agents"][0];
        assert_eq!(agent["agentId"], "codex");
        assert_eq!(agent["displayName"], "Codex");
        assert_eq!(agent["todayTokens"], 1);
        assert_eq!(agent["tokens30d"], 2);
        assert_eq!(agent["sessions30d"], 3);
        assert_eq!(agent["lastActiveTs"], 4);
        assert_eq!(agent["available"], true);
    }

    #[test]
    fn parses_utc_offset() {
        assert_eq!(parse_utc_offset("+0800"), Some(28800));
        assert_eq!(parse_utc_offset("-0530"), Some(-19800));
        assert_eq!(parse_utc_offset("+0000"), Some(0));
        assert_eq!(parse_utc_offset("0800"), None);
        assert_eq!(parse_utc_offset(""), None);
    }

    #[test]
    fn parses_iso_timestamps() {
        // 1970-01-01T00:00:00Z 是 epoch 原点
        assert_eq!(parse_iso_secs("1970-01-01T00:00:00.000Z"), Some(0));
        // 2026-08-12T04:26:31Z
        let ts = parse_iso_secs("2026-08-12T04:26:31.928Z").unwrap();
        assert_eq!(
            ts,
            days_from_civil(2026, 8, 12) * DAY_SECS + 4 * 3600 + 26 * 60 + 31
        );
        // 闰日
        assert_eq!(
            parse_iso_secs("2024-02-29T00:00:00Z"),
            Some(days_from_civil(2024, 2, 29) * DAY_SECS)
        );
        assert_eq!(parse_iso_secs("bad"), None);
    }

    #[test]
    fn finds_substrings() {
        assert!(contains_sub(
            b"{\"type\":\"token_count\"}",
            b"\"token_count\""
        ));
        assert!(!contains_sub(b"{\"type\":\"other\"}", b"\"token_count\""));
        // 首字节反复命中但整体不匹配时不应误判
        assert!(!contains_sub(b"aaaab", b"aaac"));
        assert!(contains_sub(b"aaaab", b"aaab"));
        assert!(!contains_sub(b"ab", b"abc"));
    }

    #[test]
    fn codex_diffs_cumulative_totals() {
        // total_token_usage 是会话内累计值：三条事件应得到 100 / 50 / 30 的增量，而非 100/150/180。
        let day = "2026-08-12";
        let content = format!(
            "{}\n{}\n{}\n{}\n",
            r#"{"type":"session","id":"x"}"#,
            codex_line(day, "T01:00:00", 100),
            codex_line(day, "T02:00:00", 150),
            codex_line(day, "T03:00:00", 180),
        );
        let path = tmp_file("codex-diff.jsonl", &content);
        let daily = parse_codex(&path);
        let total: u64 = daily.values().sum();
        assert_eq!(total, 180, "差分之和应等于最终累计值");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn codex_handles_counter_reset_and_missing_events() {
        // compact 后累计值回退，回退那条按新值整体计入。
        let day = "2026-08-12";
        let content = format!(
            "{}\n{}\n{}\n",
            codex_line(day, "T01:00:00", 500),
            codex_line(day, "T02:00:00", 40), // 回退
            codex_line(day, "T03:00:00", 90),
        );
        let path = tmp_file("codex-reset.jsonl", &content);
        let total: u64 = parse_codex(&path).values().sum();
        assert_eq!(total, 500 + 40 + 50);
        let _ = std::fs::remove_file(path);

        // 完全没有 token_count 事件的文件不应 panic，返回空。
        let path = tmp_file("codex-empty.jsonl", "{\"type\":\"session\"}\n");
        assert!(parse_codex(&path).is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn codex_buckets_by_local_day() {
        // 跨天的两条事件必须落到不同日桶里，而不是压到同一天。
        let content = format!(
            "{}\n{}\n",
            codex_line("2026-08-11", "T01:00:00", 100),
            codex_line("2026-08-12", "T01:00:00", 300),
        );
        let path = tmp_file("codex-days.jsonl", &content);
        let daily = parse_codex(&path);
        assert_eq!(daily.len(), 2);
        assert_eq!(daily.values().sum::<u64>(), 300);
        let _ = std::fs::remove_file(path);
    }

    fn codex_line(day: &str, time: &str, total: u64) -> String {
        format!(
            r#"{{"timestamp":"{day}{time}.000Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"total_tokens":{total}}}}}}}}}"#
        )
    }

    #[test]
    fn claude_sums_usage_fields() {
        let content = concat!(
            r#"{"type":"assistant","timestamp":"2026-08-12T07:49:35.664Z","message":{"usage":{"input_tokens":2,"output_tokens":703,"cache_creation_input_tokens":9959,"cache_read_input_tokens":14100}}}"#,
            "\n",
            // user 消息即便带 usage 也应忽略
            r#"{"type":"user","timestamp":"2026-08-12T07:49:36.000Z","message":{"usage":{"input_tokens":999}}}"#,
            "\n",
        );
        let path = tmp_file("claude.jsonl", content);
        let daily = parse_claude(&path);
        assert_eq!(daily.values().sum::<u64>(), 2 + 703 + 9959 + 14100);
        assert_eq!(daily.len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pi_prefers_total_tokens_then_falls_back() {
        let content = concat!(
            r#"{"type":"message","timestamp":"2026-08-11T02:58:35.953Z","message":{"role":"assistant","timestamp":1786417115953,"usage":{"totalTokens":1234}}}"#,
            "\n",
            // 缺 totalTokens 时按分项求和
            r#"{"type":"message","timestamp":"2026-08-11T03:00:00.000Z","message":{"role":"assistant","usage":{"input":10,"output":20,"cacheRead":30,"cacheWrite":40}}}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-08-11T03:01:00.000Z","message":{"role":"user","usage":{"totalTokens":999}}}"#,
            "\n",
        );
        let path = tmp_file("pi.jsonl", content);
        let daily = parse_pi(&path);
        assert_eq!(daily.values().sum::<u64>(), 1234 + 100);
        let _ = std::fs::remove_file(path);
    }

    /// 手动跑：`cargo test -- --ignored --nocapture real_machine_snapshot`
    /// 对照本机真实日志检查量级与耗时，CI 上不跑（依赖本地是否装了这些 Agent）。
    #[test]
    #[ignore]
    fn real_machine_snapshot() {
        let cold = collect_usage();
        for a in &cold.agents {
            println!(
                "{:<12} available={:<5} today={:>14} 30d={:>14} sessions={:>4}",
                a.agent_id, a.available, a.today_tokens, a.tokens_30d, a.sessions_30d
            );
        }
        println!("total today={} 30d={}", cold.total_today, cold.total_30d);
        println!("cold  elapsed = {} ms", cold.elapsed_ms);
        let warm = collect_usage();
        println!("warm  elapsed = {} ms", warm.elapsed_ms);
        assert_eq!(cold.total_30d, warm.total_30d, "缓存命中不应改变结果");
    }

    #[test]
    fn missing_root_reports_unavailable() {
        let spec = FileAgentSpec {
            agent_id: "codex",
            display_name: "Codex",
            root: PathBuf::from("/definitely/not/a/real/path/xyzzy"),
            depth: 3,
            keep: is_jsonl,
            parse: parse_codex,
        };
        let mut cache = UsageCache::default();
        let usage = collect_file_agent(&spec, &mut cache);
        assert!(!usage.available);
        assert_eq!(usage.tokens_30d, 0);
    }

    #[test]
    fn cache_hit_skips_reparse() {
        let day_now = day_index(now_secs());
        let mut cache = UsageCache::default();
        let dir = std::env::temp_dir().join("tmuxdeck-usage-cache-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("a.jsonl");
        std::fs::write(&path, "{}\n").unwrap();

        let meta = std::fs::metadata(&path).unwrap();
        // 预置一条与磁盘 mtime/size 一致的缓存，值刻意与真实解析结果（空）不同。
        cache.files.insert(
            path.to_string_lossy().to_string(),
            FileStat {
                mtime: mtime_secs(&meta),
                size: meta.len(),
                daily: HashMap::from([(day_now.to_string(), 777)]),
            },
        );

        let spec = FileAgentSpec {
            agent_id: "pi",
            display_name: "Pi",
            root: dir.clone(),
            depth: 2,
            keep: is_jsonl,
            parse: parse_pi,
        };
        let usage = collect_file_agent(&spec, &mut cache);
        assert_eq!(usage.today_tokens, 777, "mtime+size 未变时应直接用缓存值");

        // size 变化后必须重新解析，缓存值被覆盖。
        std::fs::write(&path, "{}\n{}\n").unwrap();
        let usage = collect_file_agent(&spec, &mut cache);
        assert_eq!(usage.today_tokens, 0, "文件变化后应重新解析");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
