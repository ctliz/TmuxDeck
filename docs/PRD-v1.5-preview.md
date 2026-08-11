# TmuxDeck v1.5 Live Pane Preview PRD

> Goal: upgrade the card's split-preview cells from "static command label" to "live output window", letting users see each agent's progress without opening the terminal.
> Technical feasibility verified (measured: a single capture-pane ~6ms).

---

## 1. Background

Today the card's split-preview cells only show `pane_current_command` (pi / node / vim) — a static label. To find out "what's going on in there", the user must open the terminal.

v1.5 makes the cells show the pane's **live output tail** (recent lines), so all four agents' progress is visible on one screen.

---

## 2. Technical foundation (verified)

### 2.1 capture-pane

tmux's native `capture-pane` grabs the pane's current on-screen content, addressed by pane_id:

```sh
tmux capture-pane -p -t %1        # %1 is the pane_id (from list-panes)
```

**Performance measured:** 100 captures took 0.58s (~6ms each) — acceptable.

### 2.2 Output noise to handle

capture-pane output contains:
- **ANSI escape sequences** (colors etc.) → must be stripped
- **tmux status-bar lines** (session info) → must be filtered
- **excess blank lines** → must be collapsed

---

## 3. Design

### 3.1 New backend command (Rust)

```rust
// capture a single pane's screen content (tail of several lines)
#[tauri::command]
fn capture_pane(pane_id: String, max_lines: usize) -> Result<String, String>
```

Implementation:
1. `tmux capture-pane -p -t <pane_id>` to get the raw output
2. **Strip ANSI escapes**: replace `\x1b\[[0-9;]*[a-zA-Z]` with empty (Rust side: `regex` crate, or a hand-written lightweight strip)
3. Filter blank/status-bar lines (separator lines starting with `───`, pure-decorative lines with lots of spaces)
4. Keep only the **tail `max_lines` lines** (e.g. 5)
5. Return the joined plain text

> If the pane no longer exists (session deleted), return an empty string, not an error.
> The `regex` crate is already in this project's dependency tree (a tauri dependency), usable directly, or hand-write the strip to avoid a new dependency.

### 3.2 Frontend

**Polling strategy (option C: all cards every 8s):**
- Reuse the existing 4s `get_tmux_sessions` poll timer, **appending** pane content capture to it
- Call `capture_pane(pane_id, 5)` for every pane of every visible session
- Frequency: in sync with the session refresh (4s) or a separate 8s — **PRD decides 8s** (each 4s tick is session metadata; 8s is content capture, avoiding amplifying overhead in lockstep)
- **Pause capture when the window loses focus** (`document.visibilityState`) — privacy + resource savings

**Rendering:**
- The preview cell shows a tail of ≤5 lines, **small monospace font** (`text-[9px] font-mono`), gray tone (`text-slate-500`)
- Overlong lines truncated (`truncate` or CSS `line-clamp`)
- Keep the current "cell highlight + agent name" logic: brighter highlight when there's content; fall back to the command name when empty
- Card hover does **not** pause cell refresh (unlike option A's "live only on hover" — we chose C)

**Structure:** the `TmuxPane` frontend type gains a `content: string` field (default empty string).

### 3.3 i18n

No new user-visible copy (preview is content, not text). If an aria-label is needed, reuse an existing key.

---

## 4. Performance budget

| Item | Magnitude |
|---|---|
| single capture-pane | ~6ms |
| typical: 10 sessions × 3 panes × capture every 8s | 30 captures / 8s ≈ **3.75 captures per second** |
| CPU usage | negligible (process overhead dominates; each <10ms) |
| focus-loss pause | no capture in background; saves further |

**Upper-limit protection:** if a pane fails capture 3 consecutive rounds (pane gone), stop capturing it until the next session refresh discovers it.

---

## 5. Privacy

Pane content may include sensitive output such as API keys and passwords, shown on the cards.

Mitigation:
- **Pause all capture when the window loses focus / is minimized** (hard requirement)
- No persistence (content exists only in memory; gone when the app quits)
- A "sensitive content mask" toggle can be added later; not this release (PRD section 7)

---

## 6. Acceptance criteria

1. The card preview cell shows the corresponding pane's live output tail (≤5 lines), staleness ≤8s
2. When the pane's output changes, the cell content follows (no manual refresh)
3. ANSI escapes / status bar / blank lines correctly filtered; no visual glitches
4. After a pane is destroyed, the cell falls back to showing the command name; no error, no hang
5. After window minimize/focus-loss, network/command calls stop (confirm via Activity Monitor or packet capture)
6. CPU usage doesn't visibly rise with 10 sessions
7. macOS build + CI dual-platform pass
8. i18n three-way alignment script passes (en/zh/App.tsx)

---

## 7. Explicitly out of scope

- ❌ live-only-on-hover (option A) — all-cards-8s polling chosen instead
- ❌ clicking a cell to attach directly to that pane (interaction extension; discuss separately)
- ❌ persisting/caching preview content to disk
- ❌ sensitive-content mask toggle
- ❌ configurable line count (5 lines hardcoded in v1.5)
- ❌ ANSI color rendering (plain text, minimal)

---

## 8. Effort estimate

| Item | Estimate |
|---|---|
| Rust: capture_pane + ANSI strip | 0.5 day |
| frontend: polling + rendering + focus-loss pause | 0.5-1 day |
| performance/privacy verification | 0.5 day |
| **Total** | **about 2 person-days** |
