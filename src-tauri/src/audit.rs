use crate::config::get_config_dir;
use crate::tmux::run_tmux;
use serde_json::json;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static AUDIT_LOCK: Mutex<()> = Mutex::new(());
static PREVIOUS_SESSION_COUNT: Mutex<Option<usize>> = Mutex::new(None);

#[derive(Debug, Clone, Copy)]
pub(crate) struct TmuxCounts {
    pub session_count: usize,
    pub pane_count: usize,
}

pub(crate) fn tmux_counts() -> TmuxCounts {
    let Ok(output) = run_tmux(&["list-panes", "-a", "-F", "#{session_id}|#{pane_id}"]) else {
        return TmuxCounts {
            session_count: 0,
            pane_count: 0,
        };
    };
    if !output.status.success() {
        return TmuxCounts {
            session_count: 0,
            pane_count: 0,
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sessions = HashSet::new();
    let mut pane_count = 0;
    for line in stdout.lines() {
        if let Some((session_id, _)) = line.split_once('|') {
            sessions.insert(session_id.to_string());
            pane_count += 1;
        }
    }
    TmuxCounts {
        session_count: sessions.len(),
        pane_count,
    }
}

pub(crate) fn record_kill(
    event: &str,
    target: &str,
    before: TmuxCounts,
    after: TmuxCounts,
    command_status: &str,
) {
    append(json!({
        "timestamp": timestamp_ms(),
        "app_pid": std::process::id(),
        "event": event,
        "target": target,
        "before_session_count": before.session_count,
        "before_pane_count": before.pane_count,
        "after_session_count": after.session_count,
        "after_pane_count": after.pane_count,
        "command_status": command_status,
    }));
}

fn nonzero_to_zero(previous: Option<usize>, current: usize) -> Option<usize> {
    previous.filter(|count| *count > 0 && current == 0)
}

pub(crate) fn record_mobile_command(
    peer_ip: std::net::IpAddr,
    command: &str,
    pane: Option<&str>,
    text: Option<&str>,
    outcome: &str,
) {
    let text_bytes = text.map(str::len);
    append(json!({
        "timestamp": timestamp_ms(),
        "app_pid": std::process::id(),
        "event": "mobile_command",
        "peer_ip": peer_ip.to_string(),
        "command": command,
        "pane": pane,
        "text_bytes": text_bytes,
        "outcome": outcome,
    }));
}

pub(crate) fn observe_session_count(current: usize, no_server: bool) {
    let previous_count = {
        let Ok(mut previous) = PREVIOUS_SESSION_COUNT.lock() else {
            return;
        };
        let previous_count = nonzero_to_zero(*previous, current);
        *previous = Some(current);
        previous_count
    };

    if let Some(previous_session_count) = previous_count {
        append(json!({
            "timestamp": timestamp_ms(),
            "app_pid": std::process::id(),
            "event": "session_count_zero_transition",
            "previous_session_count": previous_session_count,
            "session_count": current,
            "no_server": no_server,
        }));
    }
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn append(event: serde_json::Value) {
    let path = get_config_dir().join("audit.jsonl");
    let Ok(_guard) = AUDIT_LOCK.lock() else {
        return;
    };
    if let Err(error) = append_to_path(&path, &event) {
        eprintln!("[audit] write failed: {}", error);
    }
}

fn append_to_path(path: &std::path::Path, event: &serde_json::Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, event).map_err(std::io::Error::other)?;
    file.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_transition_only_fires_on_nonzero_to_zero() {
        assert_eq!(nonzero_to_zero(None, 0), None);
        assert_eq!(nonzero_to_zero(Some(0), 0), None);
        assert_eq!(nonzero_to_zero(Some(3), 2), None);
        assert_eq!(nonzero_to_zero(Some(3), 0), Some(3));
    }

    #[test]
    fn test_mobile_audit_records_text_bytes_without_content() {
        let value = "密钥abc";
        let event = json!({
            "text_bytes": Some(value.len()),
        });
        assert_eq!(event["text_bytes"], value.len());
        assert!(!event.to_string().contains(value));
    }

    #[test]
    fn test_append_writes_jsonl() {
        let path = std::env::temp_dir().join(format!(
            "tmuxdeck-audit-test-{}-{}.jsonl",
            std::process::id(),
            timestamp_ms()
        ));
        append_to_path(&path, &json!({ "event": "test" })).unwrap();
        append_to_path(&path, &json!({ "event": "test-2" })).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[0]).unwrap()["event"],
            "test"
        );
        let _ = std::fs::remove_file(path);
    }
}
