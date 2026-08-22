## TmuxDeck v1.14.13 release notes

### Fix Pane Survival on Terminal & Panel Close

- **Preserve Panes Across Terminals**: Fixed tmux pane zoom persistence bug where attaching via the embedded canvas terminal left sessions in a zoomed state, hiding other panes on disconnect.
- **Return to Shell Persistence**: Always ensure agent panes drop to a persistent shell (`exec "${SHELL:-/bin/sh}"`) after agent command exit or disconnect, preventing panes from vanishing unexpectedly.
- **Dynamic Active Session Synchronization**: Synchronize embedded terminal views directly with live session polling in `App.tsx` so added/swapped panes are immediately retained across panel and terminal lifecycle.

### Cross-Platform Release

- Official release builds published for macOS (`.dmg`) and Windows (`.exe` / `.msi`).
