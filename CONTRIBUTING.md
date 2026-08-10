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
src/App.tsx           All frontend UI (single file)
src/i18n.ts           en / zh-CN string tables
src-tauri/src/lib.rs  Backend: Tauri commands, tool registries
docs/                 Product specs (PRD-*.md) and guides
```

## Adding a terminal

Two changes in `src-tauri/src/lib.rs`:

1. Add the entry to the registry in `detect_environment()`:

   ```rust
   ("wezterm", "WezTerm", vec!["/Applications/WezTerm.app"]),
   ```

2. Add a launch branch in `open_session()`:

   ```rust
   "wezterm" => Command::new("/usr/bin/open")
       .args(["-na", "WezTerm", "--args", "start", "--", &script_path])
       .status(),
   ```

### Why there is no quoting to worry about

All terminals execute the same intermediate script `/tmp/tmuxdeck-<session>.sh`, which contains `exec tmux attach-session -t '<name>'`. Because the shell handles quoting inside the script, each terminal launch only needs to pass a script path. Do not build attach commands as inline strings when adding a terminal; follow this pattern.

## Adding an agent

Add one line to the agent registry in `detect_environment()`:

```rust
("aider", "Aider", "aider"),   // (id, display name, executable)
```

Detection uses `which` plus the `~/.nvm/versions/node/*/bin/` glob. On Windows, detection runs inside WSL via `wsl.exe`.

## Code conventions

- **Sanitize session names.** Every Tauri command that accepts a session name must call `sanitize_session_name()` first. The name is embedded in shell commands and file paths; skipping this is a command-injection vulnerability.
- **Keep it minimal.** The "explicitly out of scope" list in the PRD (see `docs/`) includes per-pane agent mixing, workspace templates, a multi-entry custom agent manager, and remote SSH. Open an issue to discuss these before submitting a PR.
- **Ask only necessary questions.** This is the core design principle: hide a row when there is only one candidate, and never show tools that are not installed.

## Before submitting a PR

- [ ] `npm run tauri build` compiles
- [ ] If you touched pane layout: confirm `tmux list-panes -s -t <name> | wc -l` matches the requested count
- [ ] If you touched session name handling: test with `a'; rm -rf ~; '` and `../../etc/passwd`
- [ ] UI text goes through the i18n tables; no hardcoded user-facing strings

## Reporting issues

Include: macOS version, `tmux -V`, which terminals and agents are installed, and steps to reproduce.
