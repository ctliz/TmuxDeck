## TmuxDeck v1.11.1

A patch release restoring Agent detection for common native Claude Code and OpenCode installations on macOS.

### Agent detection

- Detects Claude Code installed at `~/.local/bin/claude`.
- Detects OpenCode installed at `~/.opencode/bin/opencode`.
- Keeps Claude Code, Codex, and OpenCode on the same shared executable-discovery path.
- Continues to prefer a compatible Intercom-aware `cci` wrapper for Claude Code, with ordinary `claude` as the fallback.

### Why this was needed

macOS GUI applications do not reliably inherit the interactive shell `PATH`. Codex was still found through its common Homebrew location, while native Claude Code and OpenCode installers use user-local directories that TmuxDeck did not previously scan.

### Verification

- Agent-registry regression coverage includes both native installer paths.
- Backend tests pass.
- Frontend tests and production build pass.
- Tauri release build passes on macOS.
