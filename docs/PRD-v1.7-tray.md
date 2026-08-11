# TmuxDeck v1.7 Tray Icon (Menu Bar Resident) PRD

> Goal: TmuxDeck lives in the macOS menu bar; without opening the main window you can view workspace status, act on the currently active session, and quickly create a new one.
> Positioning: the ultimate form of "light". Smooth = instant refresh + zero window flicker + perceivable status.
> Technical base: Tauri 2's built-in TrayIconBuilder + Menu + on_menu_event, zero new dependencies (verified).

---

## 1. Background

No matter how minimal the main window (v1.6 liquid glass), it still needs opening. A tray icon lets users, **without opening a window**:
- glance at which workspaces exist and their active status
- one-click open/operate the **currently active session**
- quickly create a new workspace

This is the evolution of the "workspace console" from an "app" to a "resident tool".

---

## 2. Menu structure (pops up on icon click)

```
──────────────────────────
● Currently active: project-alpha     ← block 1: current active session (highlighted)
   ├─ Open (Ghostty)
   ├─ Add pane
   └─ Last active 3 minutes ago
──────────────────────────
○ project-beta               ← block 2: all sessions (click to open)
○ project-gamma
＋ New workspace…             ← block 3: inline quick create
──────────────────────────
TmuxDeck main UI             ← open main window
Quit TmuxDeck
──────────────────────────
```

### Block 1: current active session

- Determination logic (last_active_ts + attached, implemented in v1.4):
  1. if a session is attached → pick it
  2. if none attached → pick the one with the most recent last_active_ts
- Display: `● session name` (filled = running, hollow = idle)
- Submenu:
  - **Open** → reuse existing `open_session(name, terminal_id)` (uses config's default_terminal)
  - **Add pane** → new command `add_pane(session_name)` (see section 4)
  - read-only line: `Last active X minutes ago` (reuses v1.4 conversion)

### Block 2: all sessions

- Sorted by "activity": attached > recently active > rest
- Each item click → `open_session` opens it
- When there are too many sessions (>8), show only the first 8 + `View all (open main UI)`

### Block 3: inline quick create

Clicking "＋ New workspace…" pops up a **native second-level menu** (submenu):
- name input… (text items in a submenu? → no, macOS native menus don't support text input)
- **Alternative:** click "New" → directly open the main window and focus the new-workspace Modal (that Modal already exists in the main window)
- Or: the submenu lists "quick create with a recent directory" items (reusing recent_dirs)

**Conclusion (PRD decision):** new-workspace goes through the main-window Modal — "inline text input" isn't realistic in a native menu (no input field), and v1.6 already made new-workspace a dashed-card entry. Tray "New" = open the main window + auto-focus the create entry.

---

## 3. Refresh and smoothness

| Item | Approach |
|---|---|
| Menu refresh | rebuild the menu in the background every 5s (Rust-side setInterval + rebuild Menu) |
| Icon state | running sessions → filled icon; all idle → hollow icon |
| Open action | `open_session` launches the terminal directly, no window pops up |
| Add pane | `add_pane` executes immediately + reflected in the next menu refresh |
| Window flicker | menu actions never focus the main window (except "New" and "Main UI") |

### Rust-side implementation notes

```rust
// create tray in lib.rs setup
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem, Submenu};

TrayIconBuilder::new()
    .icon(app.default_window_icon().unwrap().clone())
    .menu(&build_tray_menu(app)?)   // initial menu
    .on_menu_event(|app, event| {
        match event.id().as_ref() {
            "open-session" => { /* get session name, call open_session */ }
            "add-pane"     => { /* add_pane */ }
            "new-workspace"=> { /* show main window + focus create */ }
            "show-main"    => { /* show main window */ }
            "quit"         => { app.exit(0); }
            _ => {}
        }
    })
    .build(app)?;

// 5s periodic refresh
let tray = app.tray_by_id("main").unwrap();
std::thread::spawn(move || loop {
    std::thread::sleep(Duration::from_secs(5));
    let new_menu = build_tray_menu(&app_handle)?;
    tray.set_menu(Some(new_menu)).ok();
});
```

---

## 4. New backend command: `add_pane`

```rust
#[tauri::command]
fn add_pane(session_name: String) -> Result<(), String> {
    // 1. sanitize
    // 2. find the session's working directory (from config recent_dirs? No — from the tmux pane's current dir:
    //    list-panes -F '#{pane_current_path}', take the first pane's path)
    // 3. run_tmux(&["split-window", "-t", session, "-c", dir, "shell"]))
    //    —— new pane defaults to Shell (the user can start an agent from the shell)
    // 4. run_tmux(&["select-layout", "-t", session, "tiled"])
}
```

**New pane defaults to Shell** (PRD decision):
- Reason: what the user is missing is "operable space"; Shell is the most general; to start an agent, type it yourself
- Working directory inherited from the session's first pane's `pane_current_path` (new pane lands in the project directory, correct)

---

## 5. Main-window linkage

- Tray "New" / "Main UI" → `app.show()` + `window.set_focus()`
- Closing the main window does **not quit the app** (tray stays) — needs a tauri.conf.json change:
  ```json
  "app": { "windows": [{ "title": "TmuxDeck", ... }] }
  ```
  Add window event handling: on `on_window_event`'s CloseRequested, `prevent_default()` (hide instead of quit), or configure `"visibleOnAllWorkspaces"` etc. **Close = hide, tray continues.**
- First launch: show the main window (first use needs to see the UI); after that, closing keeps it resident in the tray

**Implementation notes (Tauri 2 verified):**
- `tauri.conf.json` **must** add the `app.trayIcon` config (icon reuses `icons/icon.icns`), otherwise the `tray-icon` feature isn't enabled by default and `TrayIconBuilder` won't compile
- Window-close-resident: `tauri::Builder::on_window_event` handles `CloseRequested` → `api.prevent_close()` + hide the window, don't quit the app
- Tray menu rebuild: `app.tray_by_id("main")` + `set_menu()`; the 5s poll thread must hold an `AppHandle` (`app.clone()`)

---

## 6. Acceptance criteria

1. After the app starts, a menu bar icon appears and the main window shows
2. Closing the main window → app doesn't quit; icon still there
3. Clicking the icon pops the menu: current active session + all sessions + new + main UI + quit
4. "Current active" determination correct (attached preferred, otherwise most recently active)
5. Clicking a session item → launches the terminal attach directly, no window flicker
6. Click "Add pane" → that session gains one more pane (Shell), tiled re-layout, visible in the next menu refresh
7. Icon state: running sessions → filled, all idle → hollow
8. Menu auto-refreshes every 5s; state changes (create/delete/active-switch) reflected within seconds
9. macOS build + CI dual-platform pass
10. i18n three-way alignment (new tray copy bilingual complete)

---

## 7. Explicitly out of scope

- ❌ inline text-input create (macOS native menus don't support it; goes through the main-window Modal)
- ❌ live pane preview inside the tray (menu items are text; can't render terminal content; the main window already has preview)
- ❌ custom icon animation / dynamically generated icons (static icon + light/dark variants)
- ❌ Windows tray (v1.7 is macOS tray only; Windows tray is a system tray, not a menu bar, different shape; discuss separately)
- ❌ notification push (session-completion reminders, etc.; separate initiative)
- ❌ multiple trays across displays (meaningless)

---

## 8. Effort estimate

| Item | Estimate |
|---|---|
| tray init + icon + menu construction | 0.5 day |
| 5s dynamic refresh + state icon | 0.5 day |
| add_pane command + event dispatch | 0.5 day |
| main-window close-resident + create linkage | 0.5 day |
| **Total** | **about 2 person-days** |
