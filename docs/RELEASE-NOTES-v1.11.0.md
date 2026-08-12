## TmuxDeck v1.11.0

A visual and workflow update centered on the macOS tray experience, local Agent usage visibility, and smoother pane handoff.

### macOS tray panel

- Replaces the macOS native tray menu with a compact translucent workspace panel.
- Shows active workspaces, pane counts, dominant Agents, mobile connection status, and direct open actions.
- Adding a pane keeps the same explicit Agent picker and recommendation behavior as the main window.
- The panel dismisses on blur and avoids the previous periodic menu replacement that could close an open tray interaction.
- Windows keeps the native tray menu and its existing Agent picker.

### Local Agent usage

- Adds local-only 30-day token summaries for Codex, Claude Code, Pi, and OpenCode.
- Reads existing local Agent logs without network requests or content uploads.
- Parses logs on a background thread and uses a file metadata cache to keep later refreshes fast.
- Displays unavailable sources as not detected rather than reporting a misleading zero.

### Visual refresh

- Adds a shared indigo/cyan canvas treatment for the main window and tray panel.
- Uses the macOS overlay title bar with a restored drag region and traffic-light-safe spacing.
- Improves glass surfaces, pane previews, borders, and Agent picker contrast.
- Keeps platform-specific header spacing so Windows does not inherit the macOS traffic-light offset.

### Reliability

- Agent panes now return to an interactive shell after a normal exit or Ctrl+C, preserving the project directory.
- Windows CI remains on the stable Windows 2022 runner, compiles Rust tests, and completes the full Tauri build.

### Verification

- 107 backend tests: 105 passing and 2 environment/on-device tests ignored by default.
- 25 frontend tests passing.
- Frontend production build and Tauri release build pass.
- Local usage collection on a roughly 1 GB log corpus completed in about 0.6 seconds cold and 0.2 seconds warm.
