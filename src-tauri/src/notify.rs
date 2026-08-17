//! Desktop notifications for the single factual signal: an agent is awaiting a reply.
//!
//! The main window may be hidden while the app stays in the tray, so this lives
//! in Rust rather than the frontend. One unread notification is kept per pane.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use tauri::Emitter;
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::config::load_config;
use crate::tray::{focus_main_window, MAIN_WINDOW_LABEL};

const PREVIEW_CHARS: usize = 120;

static NOTIFIED_PANES: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static LAST_NOTIFIED_PANE: Mutex<Option<String>> = Mutex::new(None);

fn notified_panes() -> std::sync::MutexGuard<'static, Option<HashSet<String>>> {
    NOTIFIED_PANES.lock().unwrap_or_else(|e| e.into_inner())
}

fn notification_id(pane_id: &str) -> i32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    pane_id.hash(&mut hasher);
    (hasher.finish() as i32).saturating_abs().max(1)
}

fn truncate_preview(preview: &str) -> String {
    let trimmed = preview.trim();
    let mut out = String::new();
    for (i, ch) in trimmed.chars().enumerate() {
        if i >= PREVIEW_CHARS {
            out.push('…');
            break;
        }
        if ch == '\n' || ch == '\r' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn main_window_focused(app: &tauri::AppHandle) -> bool {
    use tauri::Manager;
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .and_then(|win| win.is_focused().ok())
        .unwrap_or(false)
}

fn ensure_permission(app: &tauri::AppHandle) -> bool {
    let Ok(current) = app.notification().permission_state() else {
        return false;
    };
    match current {
        PermissionState::Granted => true,
        PermissionState::Denied => false,
        _ => matches!(
            app.notification().request_permission(),
            Ok(PermissionState::Granted)
        ),
    }
}

/// Show a system notification when a pane starts waiting for a human reply.
/// No-ops when notifications are disabled, the main window is focused, or this
/// pane already has an unread notification.
pub fn maybe_notify_awaiting_human(
    app: &tauri::AppHandle,
    pane_id: &str,
    workspace_name: &str,
    title: &str,
    preview: &str,
) {
    if pane_id.is_empty() || !load_config().desktop_notifications {
        return;
    }
    if main_window_focused(app) {
        return;
    }

    {
        let mut guard = notified_panes();
        let set = guard.get_or_insert_with(HashSet::new);
        if !set.insert(pane_id.to_string()) {
            return;
        }
    }
    if let Ok(mut last) = LAST_NOTIFIED_PANE.lock() {
        *last = Some(pane_id.to_string());
    }

    if !ensure_permission(app) {
        let mut guard = notified_panes();
        if let Some(set) = guard.as_mut() {
            set.remove(pane_id);
        }
        return;
    }

    let workspace = workspace_name.trim();
    let agent = title.trim();
    let heading = if workspace.is_empty() {
        agent.to_string()
    } else if agent.is_empty() || agent == workspace {
        workspace.to_string()
    } else {
        format!("{workspace} · {agent}")
    };
    let body = truncate_preview(preview);

    let shown = app
        .notification()
        .builder()
        .title(if heading.is_empty() {
            "TmuxDeck".to_string()
        } else {
            heading
        })
        .body(body)
        .id(notification_id(pane_id))
        .extra("paneId", pane_id)
        .show();

    if shown.is_err() {
        let mut guard = notified_panes();
        if let Some(set) = guard.as_mut() {
            set.remove(pane_id);
        }
    }
}

pub fn clear_notified_pane(pane_id: &str) {
    let mut guard = notified_panes();
    if let Some(set) = guard.as_mut() {
        set.remove(pane_id);
    }
    if let Ok(mut last) = LAST_NOTIFIED_PANE.lock() {
        if last.as_deref() == Some(pane_id) {
            *last = None;
        }
    }
}

/// macOS/Windows desktop notifications activate the app but do not return a payload.
/// When the main window becomes focused, highlight the most recent waiting pane.
pub fn focus_latest_notified(app: &tauri::AppHandle) {
    let pane_id = LAST_NOTIFIED_PANE
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    if pane_id.is_empty() {
        return;
    }
    let _ = app.emit("focus-conversation", pane_id);
}

pub fn open_from_notification(app: &tauri::AppHandle) {
    focus_main_window(app, false);
    focus_latest_notified(app);
}

#[cfg(test)]
mod tests {
    use super::truncate_preview;

    #[test]
    fn preview_collapses_newlines_and_truncates() {
        assert_eq!(truncate_preview("hello\nworld"), "hello world");
        let long = "a".repeat(200);
        let out = truncate_preview(&long);
        assert!(out.ends_with('…'));
        assert_eq!(out.chars().count(), 121);
    }
}
