## TmuxDeck v1.8.1

This patch release makes workspace and pane termination safer and clearer, and improves dashboard drag interactions.

### Safer workspace lifecycle

- Prevented deletion of the last pane in a workspace through the pane-level action; destroying the workspace now requires the explicit workspace action.
- Serialized pane deletion checks to avoid concurrent requests bypassing the last-pane safeguard.
- Added local JSONL audit records for workspace and pane termination, including tmux counts before and after each operation.
- Added lifecycle coverage confirming that closing an attached terminal client detaches without destroying the tmux session or its panes.

### Clearer confirmations

- Workspace destruction now states exactly how many tmux panes will be terminated.
- The confirmation explains that closing the terminal window only detaches from the workspace and leaves it running in the background.
- Added localized English and Chinese messages for the new safeguards and errors.

### Drag interaction fixes

- Workspace cards now use a dedicated drag handle.
- Buttons and terminal icons no longer accidentally initiate card dragging.

### Install

- macOS: download the `.dmg` and drag TmuxDeck into Applications. If Gatekeeper warns on first launch, right-click and choose Open.
- Windows: download the `.exe` (NSIS) or `.msi`. Windows runs tmux and agents through WSL.
