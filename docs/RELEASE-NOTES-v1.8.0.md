## TmuxDeck v1.8.0

This release expands TmuxDeck from a session dashboard into the foundation of a multi-agent conversation bridge, while adding direct pane organization controls and a refreshed visual identity.

### Directed conversation transport

- TmuxDeck can register with pi-intercom as the human session `me` and address agents by their exact intercom session ID.
- WebSocket request results now retain their originating connection ID: subscription snapshots and command errors return only to the requesting client instead of being broadcast.
- Conversation turns are delivered only to clients subscribed to that conversation. Global conversation and status data remains synchronized across connected clients.
- Transcript readers support structured Pi and Claude Code session records, with pane capture as a fallback.
- The phone bridge uses a loopback WebSocket server, per-launch token authentication, subscription-scoped polling, frame limits, and heartbeat handling. A complete mobile client UI remains a follow-up.

### Pane and card organization

- Drag panes within a workspace card to swap their tmux layout positions.
- Drag workspace cards to reorder the dashboard without the polling refresh resetting the chosen order.

### Visual refresh

- Replaced the application, tray, platform, and installer icon sets with the new TmuxDeck identity.

### Fixes

- Preserved the source WebSocket connection for directed replies and errors.
- Fixed workspace creation field names so `agent_id` and `terminal_id` match the Rust command contract.
- Fixed pairing output to expose the complete one-time WebSocket token.

### Install

- macOS: download the `.dmg` and drag TmuxDeck into Applications. If Gatekeeper warns on first launch, right-click and choose Open.
- Windows: download the `.exe` (NSIS) or `.msi`. Windows runs tmux and agents through WSL.
