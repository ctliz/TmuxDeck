# TmuxDeck v1.4 Activity Status and Last-Active Time PRD

> Goal: let users see at a glance "is my workspace still running, and when was it last active", removing the anxiety that "closing the window = losing the work". **Minimal: zero new dependencies, two fields.**

---

## 1. Background

A user works in a workspace for a while, then closes the terminal window. Not knowing that tmux sessions are persistent, they worry "unsaved, work lost". In fact the work never gets lost — the user just **can't see it**.

The fix is not adding a "save" button (that would be fake — tmux is already saving), but having the card **show the workspace's activity status**: seeing "active 3 minutes ago" tells the user everything is still there.

---

## 2. Technical foundation (verified)

tmux natively exposes two fields, and `get_tmux_sessions` already uses the same `-F` format mechanism:

```
#{session_attached}   → 1/0 (whether a client is connected)
#{session_activity}   → last-active Unix timestamp (includes pane output activity)
```

Tested: an attached session's activity timestamp differs from the current time by only a few seconds. ✅

---

## 3. Data-model change (Rust)

`TmuxSession` gains two fields:

```rust
pub struct TmuxSession {
    // ...existing fields...
    pub attached: bool,          // existing: whether a client is connected
    pub last_active_ts: i64,     // new: last-active Unix timestamp (session_activity)
}
```

`get_tmux_sessions`'s `-F` format appends `#{session_activity}`; the parser reads the timestamp from `parts[5]`. **On parse failure, fall back to 0** (shown as "unknown"); don't block the whole list.

> ⚠️ Note: the current `-F` format is `#{session_id}|#{session_name}|#{session_windows}|#{session_attached}|#{session_created}`,
> after appending it becomes `...|#{session_attached}|#{session_created}|#{session_activity}`, so **the parts indexes must be adjusted accordingly**.

---

## 4. Frontend display (cards)

### 4.1 Three-state activity (replaces the current two-dot scheme)

| State | Determination | Display |
|---|---|---|
| 🟢 **In use** | `attached == true` | green dot + breathing animation (existing) |
| 🟡 **Active in background** | `attached == false` and last-active < 10 minutes | yellow dot, "active X minutes ago" |
| ⚪ **Idle** | `attached == false` and last-active ≥ 10 minutes, or unknown | gray dot, "Idle" |

> The determination lives in the frontend (`last_active_ts` + current time); the backend only passes the raw timestamp and does no conversion.
> The 10-minute threshold is v1.4's default and adjustable.

### 4.2 Last-active copy

Reuse the existing "X seconds/minutes/hours/days ago" conversion from v1.0 (`created_at` already did this); new i18n keys:

```
card.lastActive      → en: "Active {time} ago"    zh: "最后活跃 {time} 前"
card.lastActive_now  → en: "Active just now"      zh: "刚刚活跃"
card.idle            → en: "Idle"                 zh: "空闲"
```

The time portion (X minutes ago, etc.) is passed as a variable, reusing the existing conversion.

### 4.3 Card layout

The card header status area changes from the current "single dot + creation time" to:

```
[dot] project name                  [Rename] [Delete]
2 windows · 4 panes   ·   active 3 minutes ago
```

- Attached sessions (in use): show "In use", don't show last-active (meaningless while actively using)
- Active-in-background / idle: show last-active time

---

## 5. Incidental: top stats bar

The "running" count is currently counted by `attached`. v1.4 keeps that as-is — "running" means currently in use; active-in-background isn't "running", so the semantics stay unambiguous.

---

## 6. Acceptance criteria

1. Attached session: green dot + breathing animation, no last-active time shown
2. Just-detached session (<10 min): yellow dot, shows "active X minutes ago"
3. Long-detached session (≥10 min): gray dot, shows "active X hours ago" or "Idle"
4. Time conversion correct: seconds/minutes/hours/days gradients right
5. English copy correct (i18n bilingual complete)
6. No last-active data → shows "Idle" / "unknown", no crash
7. macOS `npm run tauri build` + CI dual-platform pass
8. Zero Chinese residue (per v1.2 standard)

---

## 7. Explicitly out of scope

- ❌ proactively pushing "workspace active" notifications to the user
- ❌ backend scheduled tasks / heartbeat mechanisms (tmux's activity is sufficient; no extra polling)
- ❌ "save" button or manual snapshot features (misunderstands the product)
- ❌ tmux session recovery after machine reboot (tmux-resurrect-style plugins are a large effort; separate initiative)
- ❌ configurable threshold (10 minutes hardcoded in v1.4; revisit when needed)
