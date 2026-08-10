# Contributing to TmuxDeck

Thanks for considering a contribution. This document is for developers; users should read the [README](README.md).

## Development setup

Requirements: Node.js, [Rust](https://www.rust-lang.org/tools/install), tmux, macOS.

```sh
git clone git@github.com:ctliz/TmuxDeck.git
cd TmuxDeck
npm install
npm run tauri dev      # dev mode with hot reload
npm run tauri build    # produce .app / .dmg
```

Artifacts are written to `src-tauri/target/release/bundle/`.

If `cargo` is not on your PATH, run `source "$HOME/.cargo/env"` first.

Stack: Tauri 2, React, TypeScript, Tailwind CSS, Rust.

## Project layout

```
src/App.tsx                    All frontend UI (single file)
src/i18n.ts                    en / zh-CN string tables

src-tauri/src/lib.rs           Tauri builder, tray wiring, command registration
src-tauri/src/tmux.rs          Core layer: the only place that shells out to tmux
src-tauri/src/registry.rs      Terminal / agent detection and icon resolution
src-tauri/src/config.rs        ~/.config/tmuxdeck/config.json
src-tauri/src/intercom.rs      pi-intercom broker client (agent bus)
src-tauri/src/bridge.rs        Conversation model: panes ⊕ intercom sessions
src-tauri/src/commands/        Thin Tauri command wrappers — no business logic

docs/                          Product specs (PRD-*.md) and guides
scripts/                       Dev-only verification scripts
```

Start with [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the data flows.

**Layering rule:** `commands/` only parses arguments and translates errors. Business
logic belongs in `tmux.rs` / `bridge.rs`. `intercom.rs` and `bridge.rs` must not depend
on the `tauri` crate — that keeps them unit-testable and extractable later.

## Adding a terminal

Two changes:

1. Add the entry to the registry in `registry.rs::detect_environment()`:

   ```rust
   ("wezterm", "WezTerm", vec!["/Applications/WezTerm.app"]),
   ```

2. Add a launch branch in `commands/session.rs::open_session()`:

   ```rust
   "wezterm" => Command::new("/usr/bin/open")
       .args(["-na", "WezTerm", "--args", "start", "--", &script_path])
       .status(),
   ```

### Why there is no quoting to worry about

All terminals execute the same intermediate script `/tmp/tmuxdeck-<session>.sh`, which contains `exec tmux attach-session -t '<name>'`. Because the shell handles quoting inside the script, each terminal launch only needs to pass a script path. Do not build attach commands as inline strings when adding a terminal; follow this pattern.

## Adding an agent

Add one line to the agent registry in `registry.rs::detect_environment()`:

```rust
("aider", "Aider", "aider"),   // (id, display name, executable)
```

Detection uses `which` plus the `~/.nvm/versions/node/*/bin/` glob. On Windows, detection runs inside WSL via `wsl.exe`.

## Code conventions

- **Sanitize session names.** Every Tauri command that accepts a session name must call `sanitize_session_name()` first. The name is embedded in shell commands and file paths; skipping this is a command-injection vulnerability.
- **Never send free text without `-l`.** `tmux send-keys` interprets strings like `C-c` and `Escape` as key names. User text goes through `send_keys()` (which passes `-l`); control keys go through `send_key_name()`, which validates against an allow-list. Do not merge the two channels.
- **Do not guess agent state.** The intercom broker reports `idle` / `thinking` / `tool:<name>` as fact. Sessions not on the bus are `unknown` — leave them unknown rather than inferring from pane silence. See [`docs/DECISIONS-v1.12.md`](docs/DECISIONS-v1.12.md#5-靠-capture-pane-轮询做四态判定) for why the heuristic approach was removed.
- **Keep it minimal.** The "explicitly out of scope" list in the PRD (see `docs/`) includes per-pane agent mixing, workspace templates, a multi-entry custom agent manager, and remote SSH. Open an issue to discuss these before submitting a PR.
- **Ask only necessary questions.** This is the core design principle: hide a row when there is only one candidate, and never show tools that are not installed.

## Before submitting a PR

- [ ] `npm run tauri build` compiles
- [ ] If you touched pane layout: confirm `tmux list-panes -s -t <name> | wc -l` matches the requested count
- [ ] If you touched session name handling: test with `a'; rm -rf ~; '` and `../../etc/passwd`
- [ ] UI text goes through the i18n tables; no hardcoded user-facing strings

## Reporting issues

Include: macOS version, `tmux -V`, which terminals and agents are installed, and steps to reproduce.
