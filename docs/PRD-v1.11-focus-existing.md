# TmuxDeck v1.11 Duplicate-Open Prevention (Focus Existing Window) PRD

> Goal: when a session is already open (attached), don't spawn another terminal window — **focus the existing one**.
> Strategy: C (precise focus, needs accessibility permission) + A (fallback: activate the terminal app, no permission required).
> Core: attached → focus; not attached → open a new window as normal.

---

## 1. Background and problems

`open_session` currently unconditionally launches a new terminal window to attach. tmux allows a session to be attached by multiple clients,
so users repeatedly clicking a card spawn a pile of duplicate windows — messy and wasteful.

**Foolproofing goal**: an attached session must not open a new window; bring the user to the window that already exists.

---

## 2. Approach (C + A fallback chain)

```
click to open session
    │
    ├─ not attached ──→ open new window as normal (existing logic)
    │
    └─ attached ──→ try precise focus (C)
                          │
                          ├─ has accessibility permission → locate by window title and focus (osascript System Events)
                          └─ no permission → fall back to activating the app (A, osascript activate)
```

### 2.1 Determine attached

The backend's `get_tmux_sessions` already returns `attached`. Add a lightweight check:
```rust
fn is_session_attached(name: &str) -> bool {
    // tmux list-sessions -F '#{session_attached}' -t <name> == "1"
}
```
Or reuse existing session data (the frontend passes the attached state to open_session).

**PRD decision**: determine in the backend (`open_session` checks once internally, avoiding reliance on possibly-stale frontend state).

### 2.2 Precise focus (C, osascript System Events)

```applescript
-- locate the terminal window by title (tmux session name = terminal window title)
tell application "System Events"
    tell process "Ghostty"
        repeat with w in windows
            if name of w contains "SESSION_NAME" then
                set frontmost of process "Ghostty" to true
                perform action "AXRaise" of w
                return
            end if
        end repeat
    end tell
end tell
```

- **Requires accessibility permission** (System Events access): without it, osascript reports `-25211`
- On failure → fall back to A

### 2.3 Activate app (A, fallback)

```applescript
tell application "Ghostty" to activate
```

- No permission required
- Effect: activates the terminal app; the user sees the existing session window (sufficient when the session has a single window)

### 2.4 Terminal differences

Each terminal has a different AppleScript process name:

| Terminal | process name | activate syntax |
|---|---|---|
| ghostty | "Ghostty" | tell application "Ghostty" to activate |
| iterm2 | "iTerm2" | tell application "iTerm2" to activate |
| terminal | "Terminal" | tell application "Terminal" to activate |
| wezterm | "WezTerm" | tell application "WezTerm" to activate |
| kitty | "kitty" | tell application "kitty" to activate |
| alacritty | "Alacritty" | tell application "Alacritty" to activate |

Match by `terminal_id`; if no match, skip focus and go straight to the original logic.

### 2.5 Windows (same foolproofing; AppActivate needs no permission)

Repeated clicks on Windows also open duplicate tabs/windows (wt new tab, cmd new window, powershell new window), so **foolproofing is required**.

```powershell
# already attached → focus the existing window (by title)
(New-Object -ComObject WScript.Shell).AppActivate("<session_name>")
```

- **AppActivate activates a window by title and needs no accessibility permission** (a lower bar than macOS System Events)
- Focus failure (title mismatch / window gone) → silently return (no new window, no error)
- Branch logic: `is_session_attached` → yes → PowerShell AppActivate; no → existing new-window logic
- Windows Terminal's tab title usually contains the session name on attach and can be matched; if not, degrade silently (the user can switch themselves)

---

## 3. Frontend changes

- `open_session` command signature unchanged (`name` + `terminal_id`)
- The frontend need not know about attached state (determined in the backend)
- Click behavior unchanged: click → invoke open_session → the backend decides "focus or new window"

---

## 4. Acceptance criteria

1. Non-attached session: click → opens a new terminal window as normal (regression, existing behavior)
2. Attached + has accessibility permission: click → **no new window**, the existing window comes to front
3. Attached + no permission: click → no new window, terminal app activated (activate)
4. Click 5 times in a row: terminal window count stays the same (always 1)
5. Different terminals (if several installed): each focuses/activates correctly by its process name
6. osascript failure (terminal not installed / process missing) → graceful error or silent, no crash
7. macOS build + CI on both platforms pass

---

## 5. Explicitly out of scope

- ❌ A guide page walking users through enabling the accessibility permission (silently degrade on failure is enough)
- ❌ Permission detection (just try to execute; degrade on failure — no pre-checking, less complexity)
- ❌ Multi-window sessions (when a session has several windows, focus the first matching window)

---

## 6. Effort estimate

| Item | Estimate |
|---|---|
| Backend: attached check + focus/fallback osascript | 0.5 day |
| Frontend: no changes (signature unchanged) | 0 |
| Verify multiple terminals | 0.25 day |
| **Total** | **about 0.5-1 person-day** |
