## TmuxDeck v1.10.0

A feature release adding trusted-LAN mobile control, mixed-Agent workspaces, and a faster desktop experience.

### Mobile control on a trusted LAN

- Pair a phone from the desktop with a QR code or copyable LAN URL.
- Use the embedded, dependency-free mobile interface to browse conversations, reply to pending questions, send approved control keys, and forward context between panes.
- HTTP and WebSocket share one dynamic LAN port and reuse the existing conversation bridge protocol.
- Token, Host, source-address, frame-size, rate-limit, path, command, and key allow-list checks protect the plaintext trusted-LAN boundary.
- Mobile command auditing records metadata and text byte length without storing message content.

### Mixed-Agent workspaces

- Choose a different detected Agent for every pane when creating a workspace.
- Add a pane from the desktop or tray menu by explicitly choosing its Agent.
- Pane-level Agent metadata is persisted in tmux, so wrappers and transient process names do not break identification.
- Legacy tmux workspaces and Ghostty native slots both support mixed Agent assignments.

### Claude Code Intercom

- TmuxDeck prefers a compatible `cci --tui` installation for Claude Code and falls back to ordinary `claude` when unavailable.
- Every Claude pane or native slot receives a stable, unique Intercom ID and readable name.
- The Agent picker clearly distinguishes `Claude Code · Intercom (cci)` from `Claude Code · Standard`.
- Upstream `cci --tui` npm monitor packaging is tracked in [agent-intercom-claude#6](https://github.com/dataforxyz/agent-intercom-claude/issues/6).

### Performance and usability

- Removed periodic executable probing that caused slow startup and recurring UI stalls.
- Cached environment detection and invalidated it immediately after configuration changes.
- Session polling no longer reloads static environment/config data every four seconds.
- Polling pauses while hidden, avoids overlapping requests, and batches pane preview updates into one render.
- Improved the mobile viewport, keyboard, safe-area, scrolling, modal, narrow-screen, and touch-target behavior across mobile browsers.

### Verification

- 93 backend tests: 92 passing and 1 on-device transcript test ignored.
- 25 frontend tests passing.
- Frontend production build and Tauri application build pass.
- Final physical-phone LAN acceptance remains pending; LAN transport is plaintext and intended only for a trusted network.

### Install

- macOS: download the `.dmg`, drag it into Applications, then right-click → Open if Gatekeeper prompts.
- Windows: download the `.exe` (NSIS) or `.msi` artifact.
