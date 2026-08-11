# TmuxDeck v1.1 Product Requirements Document

> Goal: evolve from a "Ghostty + 4×Pi hardcoded tool" into a **"any terminal × any agent" tmux workspace console**.
> Principle: **minimal-first**. Zero-config usable by default, advanced options collapsed and hidden; no abstractions we won't use.

---

## 1. Current problems (v1.0)

| Problem | Manifestation |
|---|---|
| Terminal hardcoded | Ghostty only; users without it can't use the app at all |
| Agent hardcoded | `pi` only; users of Claude Code / Codex / OpenCode are excluded |
| Splits hardcoded | must be 4 splits; a solo small project doesn't need 4 agents |
| Concept leakage | UI full of "4-Pi", "Pi Ready", "Pi Agent" — the product name is held hostage by implementation details |
| Paths typed by hand | working directory requires typing an absolute path; extremely error-prone |
| No memory | every new workspace re-fills everything; no persisted defaults |

---

## 2. Core design: registry + auto-detection + remember-last

### 2.1 Three selectable dimensions

When creating a workspace, the user faces only three choices, **all with smart defaults**:

```
[project name]  [directory 📁]    ← required / optional
─────────────────────
Agent:  ( pi ) claude  codex  opencode  + custom
Splits:   1  2  ( 4 )  6
Terminal: ( Ghostty )  iTerm2  Terminal     ← only installed ones listed
```

- Parentheses = default selection (from last use; first detected on first run)
- **Only installed items show**: if Kitty isn't installed it doesn't appear; don't manufacture invalid options
- If a dimension detects only 1 candidate → the whole row is hidden (minimal: no choice, no question)

### 2.2 Terminal registry (static table on the Rust side)

Unified execution model: **write the attach command into a temp script first, then have the terminal execute the script**. This completely avoids each terminal's wildly different quote-escaping problems.

```
/tmp/tmuxdeck-<session>.sh   content: #!/bin/bash\nexec <tmux> attach-session -t '<name>'
```

| id | display name | detection path | launch method (`$S` = script path) |
|---|---|---|---|
| `ghostty` | Ghostty | `/Applications/Ghostty.app` | `open -na Ghostty --args --command=$S` |
| `iterm2` | iTerm2 | `/Applications/iTerm.app` | `osascript -e 'tell app "iTerm" to create window with default profile command "$S"'` |
| `terminal` | Terminal (System) | `/System/Applications/Utilities/Terminal.app` | `osascript -e 'tell app "Terminal" to do script "$S"'` + activate |
| `wezterm` | WezTerm | `/Applications/WezTerm.app` | `open -na WezTerm --args start -- $S` |
| `kitty` | kitty | `/Applications/kitty.app` | `open -na kitty --args $S` |
| `alacritty` | Alacritty | `/Applications/Alacritty.app` | `open -na Alacritty --args -e $S` |

> Terminal.app always exists on macOS → **guarantees TmuxDeck always has at least one usable terminal**; no more dead-end "environment not satisfied".

### 2.3 Agent registry

Detection: `which <bin>` + common paths (including nvm multi-version dirs, globbing `~/.nvm/versions/node/*/bin/<bin>`).

| id | display name | detection bin | launch command |
|---|---|---|---|
| `pi` | Pi | `pi` | `pi` |
| `claude` | Claude Code | `claude` | `claude` |
| `codex` | Codex | `codex` | `codex` |
| `opencode` | OpenCode | `opencode` | `opencode` |
| `gemini` | Gemini CLI | `gemini` | `gemini` |
| `aider` | Aider | `aider` | `aider` |
| `shell` | Plain Shell | — | `$SHELL` (**the always-available fallback**) |

**Custom:** the user can enter one free-form command in settings (e.g. `claude --model opus`), saved as an agent item. v1.1 supports only **1 custom** entry; no management list (enough for the purpose).

### 2.4 Splits

- Options `1 / 2 / 4 / 6`, uniformly `select-layout tiled`
- Every pane launches the same agent (v1.1 does not do per-pane mixing; demand not validated)
- `1` split = single-agent workspace, a real usage pattern for many

---

## 3. Config persistence

File: `~/.config/tmuxdeck/config.json` (Tauri reads/writes directly; no extra dependency)

```json
{
  "default_terminal": "ghostty",
  "default_agent": "pi",
  "default_panes": 4,
  "custom_agent": { "name": "Claude Opus", "command": "claude --model opus" },
  "recent_dirs": ["/Users/x/Desktop/TmuxDeck"]
}
```

- Write back the current selection after every successful create → carried over next time
- `recent_dirs` capped at 5, shown as quick chips under the directory input

---

## 4. UI change list

### 4.1 De-Pi-fy the copy
| Old | New |
|---|---|
| `New 4-Pi workspace` | `New workspace` |
| `4-Pi array layout` | `Splits` |
| `Pi Ready` | show the real agent name, e.g. `claude ×4` |
| `Resume session (Ghostty)` | `Open` (terminal icon tooltip says which terminal) |
| subtitle `Ghostty & Tmux 4-Pi Agent workspace console` | `tmux multi-agent workspace console` |

### 4.2 Top environment indicator
- From "three hardcoded items: Tmux / Ghostty / Pi" → "**Tmux ✓** + N terminals / M agents detected"
- Click expands a small panel listing the specifics; takes no space normally
- tmux not installed is the only **hard block**: full-screen guide `brew install tmux`, one-click copy

### 4.3 New-workspace dialog (minimal)
- Directory: **add a system folder-picker button** (`tauri-plugin-dialog`); manual typing kept
- The three option rows use **segmented chips**, not dropdowns (see everything at once, one click)
- Rows with only 1 candidate auto-hide
- Keep the "auto-config" note at the bottom, wording dynamic: `will create 4 splits, each running claude, opened with Ghostty`

### 4.4 Cards
- The pane preview grid count follows the real `panes_count` (no longer fixed at 4 cells)
- Cell highlight: `pane.command` matching any known agent bin → highlight + show agent name
- Keep: rename, destroy, open

---

## 5. Backend interface changes (Tauri commands)

```rust
// new
detect_environment() -> Environment {
  tmux: Option<String>,
  terminals: Vec<ToolInfo>,   // installed only
  agents:    Vec<ToolInfo>,   // installed only + shell fallback
}
struct ToolInfo { id: String, name: String, path: String }

load_config()  -> Config
save_config(config: Config) -> ()
pick_directory() -> Option<String>          // via dialog plugin

// reworked (was create_4pi_session / attach_session)
create_session(CreateOpts {
  name: String, dir: Option<String>,
  agent_id: String, panes: u8, terminal_id: String,
}) -> ()

open_session(name: String, terminal_id: String) -> ()

// unchanged
get_tmux_sessions() / kill_session() / rename_session()
```

**Compatibility:** v1.0's `check_env` / `create_4pi_session` / `attach_session` are removed outright; no existing-user burden.

---

## 6. Acceptance criteria

1. With **neither Ghostty nor pi installed**, the app still creates and opens workspaces normally (Terminal.app + shell fallback)
2. 3 terminals installed → the terminal row in the new-workspace dialog shows 3 chips; only 1 installed → the row is hidden
3. Select `claude` + `2` splits → tmux really has 2 panes each running a claude
4. Create once, quit the app and reopen → new-workspace defaults = last selection
5. The card makes it clear which agent this session runs and how many panes
6. The whole flow **requires zero typed paths**
7. "4-Pi" no longer appears anywhere in the UI

---

## 7. Explicitly out of scope (prevent over-design)

- ❌ per-pane mixing of different agents
- ❌ CRUD management UI for multiple custom agents
- ❌ workspace templates / saved layout presets
- ❌ remote SSH tmux
- ❌ Linux / Windows terminal registries (macOS only this release, but the registry structure must port cleanly)
- ❌ agent version detection, auto-install
