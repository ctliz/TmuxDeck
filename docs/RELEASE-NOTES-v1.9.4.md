## TmuxDeck v1.9.4

A patch release restoring workspace organization actions for Ghostty native workspaces.

### Fixes

- **Workspace card reordering works again.** Card drag-and-drop now uses a dedicated handle and remains stable for native workspace IDs.
- **Native workspace rename is supported.** Renaming updates the native slot tmux sessions and workspace metadata, then rebuilds the Ghostty workspace under the new name.
- **Native pane reordering works again.** Dragging panes swaps their persisted slot metadata and rebuilds the Ghostty grid, so the new order remains after refresh.
- **More reliable drag interactions.** Separating the card drag handle from the title and rename input avoids conflicting browser drag events.

### Verification

- Frontend build and tests pass.
- Rust command tests pass, including native slot swap validation.

### Install

- macOS: download the `.dmg`, drag it into Applications. If Gatekeeper warns on first launch, right-click -> Open.
- Windows: download the `.exe` (NSIS) or `.msi`.
