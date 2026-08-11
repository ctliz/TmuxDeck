## TmuxDeck v1.9.2

A patch release fixing a regression introduced in v1.9.1 that broke opening Ghostty native workspaces, and making stale terminal windows self-heal.

### Fixes

- **Ghostty native workspaces open again.** The v1.9.1 attach-script guard inlined shell logic (`if/then/else`) into the surface command, which Ghostty wraps with `exec -l` — breaking the syntax and making every native workspace fail to launch. Guards now live in script files (with a shebang), which execute correctly under `exec -l`.
- **Stale Ghostty windows self-heal.** A terminal window created before a session was deleted keeps that session's attach script path as its command. The attach script is now rewritten (as a guarded script) even when the session is gone, so pressing Cmd+D (split) in an old window degrades to a shell with a notice instead of popping up a "failed to launch" window.

### Install

- macOS: download the `.dmg`, drag into Applications. If Gatekeeper warns on first launch, right-click -> Open.
- Windows: download the `.exe` (NSIS) or `.msi`.
