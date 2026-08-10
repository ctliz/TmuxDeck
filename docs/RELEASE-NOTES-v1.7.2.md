## TmuxDeck v1.7.2

A patch release fixing the Ghostty duplicate-instance bug and shipping the single-file codebase split.

### Fix: Ghostty no longer spawns duplicate instances

Opening a session launched a **new Ghostty instance** every time (`open -na` forces a new process), so repeated clicks piled up instances and windows. Opening now uses Ghostty's native AppleScript `new window with configuration` — one instance, one new window per click, command still executed reliably.

### Refactor: codebase split (behavior unchanged)

- Rust backend split from one 1157-line `lib.rs` into focused modules: `config`, `models`, `registry`, `tmux`, `tray`, and `commands/` (session / pane / utils).
- Frontend split from one 987-line `App.tsx` into `types.ts` plus components: `SessionCard`, `CreateWorkspaceModal`, `TmuxMissingScreen`, `SearchHeader`, `NewWorkspaceCard`.
- Every file is now under 400 lines; no dependencies added, no behavior changed.

### Install

- macOS: download the `.dmg`, drag into Applications. If Gatekeeper warns on first launch, right-click -> Open (unsigned build).
- Windows: download the `.exe` (NSIS) or `.msi`.
