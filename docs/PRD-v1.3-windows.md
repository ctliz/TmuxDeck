# TmuxDeck v1.3 Windows Support PRD

> Goal: let Windows developers use TmuxDeck, **reusing the existing architecture and registry structure**.
> Core decision: **tmux runs inside WSL**, and Windows terminal shells (cmd / PowerShell / Windows Terminal) act as the entry point.
> Prerequisite: v1.2's i18n is merged, so all new copy goes through the language packs directly.

---

## 1. Background and factual constraints

**Windows has no native tmux.** This hard fact cannot be worked around and dictates the overall architecture:

- ❌ Not viable: `Command::new("tmux")` on the Windows side — the thing doesn't exist in the system
- ✅ Viable: install tmux in WSL (Windows Subsystem for Linux), bridge from the Windows side via `wsl.exe`

**Key insight: `wsl.exe` is itself the perfect bridge.**
```
wsl.exe -- tmux list-sessions -F '#{session_name}'
wsl.exe -- tmux attach-session -t myproject
```
`wsl.exe` passes argv through directly, so **attaching from the Windows side needs no script file** (the macOS `.sh` exists because of `open -na`'s quote hell).

---

## 2. Architecture

```
┌─ Windows side ──────────────────────────────┐
│  TmuxDeck (Tauri)                           │
│    ├─ cmd.exe / powershell.exe / wt.exe     │  ← terminal shells (launch entry)
│    └─ wsl.exe ──┐                           │
└─────────────────┼───────────────────────────┘
                  ▼
┌─ Inside WSL ────────────────────────────────┐
│  tmux server / each session / agents        │  ← real runtime
└─────────────────────────────────────────────┘
```

**Layered responsibilities:**
- WSL = runtime (tmux and agents both live in WSL)
- Windows = console (TmuxDeck is just a management UI)

**Agent constraint:** the agent CLIs (claude / codex / opencode / pi, etc.) must be installed **inside WSL**. The panes created by `new-session` run the WSL-side agents.

---

## 3. Backend abstraction points (all centralized in lib.rs)

### 3.1 `run_tmux(args) -> Output` — the single bridging function

```rust
// All tmux command calls go through this; internally it branches by platform:
#[cfg(target_os = "windows")]
fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("wsl.exe").arg("--").arg("tmux").args(args).output()
}
#[cfg(target_os = "macos")]
fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new(get_tmux_bin()).args(args).output()
}
```

**Change surface:** `get_tmux_sessions` / `create_session` / `kill_session` / `rename_session` / `get_session_panes` all switch from `Command::new(tmux)` to `run_tmux(...)`. Call sites unchanged; only function internals change.

**`create_session` is special:** it builds a bash script to run multiple commands. On Windows it must be reworked:

```rust
// after: call run_tmux() per step, no more /bin/bash -c string assembly
run_tmux(&["new-session", "-d", "-s", name, "-c", dir, agent_cmd])?;
for _ in 1..panes {
    run_tmux(&["split-window", "-t", name, "-c", dir, agent_cmd])?;
}
run_tmux(&["select-layout", "-t", name, "tiled"])?;
```

> **Good news:** back in v1.1 the developer already changed the splitting to a per-step loop (to fix a P1), which now happens to fit Windows naturally. **Forbidden** to revert to bash assembly.

### 3.2 Terminal launch (open_session)

| id | name | launch method |
|---|---|---|
| `wt` | Windows Terminal | `wt.exe new-tab -- wsl.exe -- tmux attach -t <name>` (`Command::new("wt.exe")` direct argv) |
| `cmd` | Command Prompt | `cmd.exe /c start cmd /k wsl.exe -- tmux attach -t <name>` |
| `powershell` | PowerShell | `powershell.exe -NoExit -Command "wsl.exe -- tmux attach -t <name>"` |

- cmd / powershell always exist on Windows → **dual fallback, never dead-ends**
- **No script file needed** (wsl.exe passes argv through; no quoting issues)

### 3.3 Config path

```rust
fn get_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    { dirs::config_dir().unwrap_or_default().join("tmuxdeck").join("config.json") } // %APPDATA%\tmuxdeck
    #[cfg(target_os = "macos")]
    { ...existing logic... }
}
```
Add the `dirs = "1"` dependency (standard practice in the Rust ecosystem; cross-platform and uniform).

### 3.4 Agent / tool detection

| Item | macOS | Windows |
|---|---|---|
| Binary detection | `which <bin>` | `wsl.exe -- which <bin>` (detect inside WSL!) |
| nvm multi-version | `~/.nvm/versions/node/*/bin/` | `wsl.exe -- bash -c 'ls ~/.nvm/versions/node/*/bin/<bin>'` |

**Key:** agents live in WSL, so **detection must also happen inside WSL**; you can't use Windows `where.exe` to probe something that only exists inside WSL.

### 3.5 Working directory (Windows-specific pain point)

`tauri-plugin-dialog` returns a **Windows path** (`C:\Users\foo`), but tmux/agents need a **WSL path** (`/mnt/c/Users/foo`).

```rust
// new: on the Windows side, convert a Windows path to a WSL path
#[cfg(target_os = "windows")]
fn to_wsl_path(win_path: &str) -> String {
    // call wsl.exe wslpath -u '<win_path>'
    Command::new("wsl.exe").arg("wslpath").arg("-u").arg(win_path)
        .output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| win_path.to_string())
}
```

**Frontend working-directory input** on Windows:
- After the folder picker selects a path → auto-convert and display the WSL path
- When typed by hand, **hint** to use `/mnt/...` format

---

## 4. Frontend changes

### 4.1 Missing-environment guide (copy per platform)

macOS missing tmux → guide `brew install tmux`
Windows missing tmux → two-step guide: `wsl --install` → `sudo apt install tmux`

New i18n key: `tmux.missing.win` (one each en/zh)

### 4.2 Terminal dropdown

- On Windows, show only wt / cmd / powershell (those installed)
- macOS logic unchanged
- **Detection results come from the backend**, so the frontend doesn't need to know about platform differences — architecturally isolated already

### 4.3 No other frontend changes

Cards, stats, and error-code translation all reuse. Reuse existing i18n keys where possible; all new keys bilingual.

---

## 5. Risks and mitigations

| Risk | Mitigation |
|---|---|
| WSL not installed | platform-specific guide copy + one-click copy of `wsl --install` |
| no tmux inside WSL | guide `sudo apt install tmux` |
| wsl.exe interactive-mode compatibility in cmd | solved by Windows 11 ConPTY; recommend upgrading Win10 22H2+ |
| wslpath conversion fails | fall back to the original path + frontend hints to type the WSL path manually |
| multiple WSL distros | v1.3 supports only the default distro (`wsl.exe --` without `-d`); no distro-selection UI |

---

## 6. Acceptance criteria

1. **On a real Windows machine**: with WSL + tmux installed, `detect_environment` lists at least one of wt/cmd/powershell
2. Create a 4-split workspace with cmd; `wsl.exe -- tmux list-panes -s -t <name> | wc -l` = 4
3. Open an existing session with Windows Terminal; attach succeeds
4. Folder picker selects `C:\Users\x\proj` → actually created at `/mnt/c/Users/x/proj`
5. Config written to `%APPDATA%\tmuxdeck\config.json`; defaults carried over after restart
6. **Full regression on macOS** (existing 7 v1.1 acceptance items + i18n acceptance)
7. When WSL is missing, the guide page shows a copyable `wsl --install` hint

> ⚠️ I cannot verify the Windows branch on real hardware from macOS. **Cross-compile**:
> `cargo build --target x86_64-pc-windows-msvc` (needs the Rust target component + linker).
> If full cross-compilation isn't possible, at minimum keep the `#[cfg(target_os = "windows")]` branches syntactically correct, and have the logic reviewed manually by peer review. Acceptance items 1–5 need to run on a Windows machine.

---

## 7. Explicitly out of scope (prevent over-design)

- ❌ WSL distro-selection UI (multi-distro users wait for v1.4)
- ❌ Git-Bash / MSYS2 / Cygwin support (tmux works there but the experience is poor; not now)
- ❌ Windows native third-party terminals beyond Windows Terminal (ConEmu / Cmder; wait for community PRs)
- ❌ cross-platform session sharing / sync (WSL and macOS are two independent worlds)
- ❌ native Linux desktop support (gnome-terminal / konsole, etc.; registry structure already port-friendly, wait for v1.4)
- ❌ probing Windows-native agent builds on Windows (claude.exe etc.) — uniformly use the WSL-side versions

---

## 8. Effort estimate

| Item | Estimate |
|---|---|
| `run_tmux` abstraction + rework 5 call sites | 0.5 day |
| `create_session` de-bash-ification | 0.5 day |
| terminal registry Windows branch + open_session | 0.5 day |
| config path `dirs` + wslpath conversion | 0.5 day |
| WSL-side detection + guide copy | 0.5 day |
| frontend tweaks + new i18n keys | 0.5 day |
| **Total** | **about 3 person-days** |
