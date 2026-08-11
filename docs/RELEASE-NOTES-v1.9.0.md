## TmuxDeck v1.9.0

This release adds Ghostty-native multi-agent workspaces while preserving tmux-backed process survival, and improves bridge compatibility and startup efficiency.

### Ghostty native splits

- Ghostty 1.3+ workspaces use native terminal splits for smoother multi-agent interaction.
- Each visible agent runs in its own single-pane tmux session, so closing a Ghostty split or window detaches the view without terminating the agent.
- Supports deterministic 1, 2, 4, and 6-agent layouts, workspace restoration, adding agents, and exact slot termination.
- Workspace cards remember and display their launch terminal with one clear start/restore action; pane previews use a stable scrollable layout and the full header acts as the card drag handle.
- Other terminals continue to use the existing tmux multi-pane layout.

### Safer workspace lifecycle

- Native Agent termination targets only its exact backing tmux session.
- Destroying a native workspace terminates only the sessions belonging to that workspace.
- Native workspace rename and pane swapping are disabled until their metadata and layout semantics can be preserved safely.
- Long workspace names and internal separator-like text are handled without truncating slot targets.

### Reliability and performance

- Native Agent slots now create their tmux session, isolated identity environment, and metadata in one command queue, with clear startup-exit diagnostics.
- Newly launched Agents no longer inherit another Harness session's stable identity from the process that started tmux.
- The create dialog preserves pane, Agent, and terminal selections during background refreshes and no longer flashes a false “Creating” state.
- Updated the built-in human adapter to Agent Intercom protocol v3, including protocol negotiation and `deliveryId` acknowledgements.
- Parent-process ownership is now resolved from one shared `ps` snapshot per refresh batch instead of spawning a process for every session and parent level.
- Empty pane maps skip process inspection entirely, preventing the previous startup fork storm.

### Documentation

- Added a cross-Harness Agent Intercom guide for Pi, OpenCode, Codex, and Claude Code.
- Documented the exact-PID-only macOS App E2E shutdown policy.

### Install

- macOS: download the `.dmg` and drag TmuxDeck into Applications. If Gatekeeper warns on first launch, right-click and choose Open.
- Windows: download the `.exe` (NSIS) or `.msi`. Ghostty native splits are macOS-specific; Windows retains the legacy tmux layout.
