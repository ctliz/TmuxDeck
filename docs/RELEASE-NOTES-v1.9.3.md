## TmuxDeck v1.9.3

A patch release improving how new panes are added to Ghostty native workspaces: adding a pane now rebuilds the layout into a clean dashboard-style grid instead of piling splits sideways.

### Fixes & improvements

- **"Add pane" = current visible panes + 1.** Previously, adding a pane revived every pane you had closed in Ghostty (a 4-pane workspace with 2 closed became 5 panes). Now the new pane is added on top of what you actually see — closed panes stay closed and keep running in the background.
- **Grid-style rebuild.** The layout is rebuilt into an even grid like the dashboard preview: 2 panes side by side, 3 as 2+1, 4 as a 2x2, 6 as 2x3. No more infinite sideways splitting.
- **Split direction follows the layout.** Only clearly horizontal panes (width >= 2x height) split to the right; square-ish panes (e.g. a 2x2 grid with one pane removed) split downward instead of always to the right.

### Notes

- When adding a pane, Ghostty briefly opens the new grid window then closes the old one (~1s visual jump). Agent processes are not affected.
- Tip: `set -g mouse on` in `~/.tmux.conf` enables click-to-focus in tmux panes (Terminal.app and others).

### Install

- macOS: download the `.dmg`, drag into Applications. If Gatekeeper warns on first launch, right-click -> Open.
- Windows: download the `.exe` (NSIS) or `.msi`.
