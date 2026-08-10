## TmuxDeck v1.7.0

Since v1.6.0, five feature increments shipped in one release: a menu bar presence with full session control, real terminal brand icons, a streamlined card header, pane-level management, and duplicate-window protection. This is the largest quality-of-life release since Liquid Glass.

### Menu bar presence (v1.7)

TmuxDeck now lives in the menu bar. Close the window and it keeps running — the tray icon shows the active workspace, and the menu lets you open any session, add a pane, or create a new one without opening the main window.

- Active workspace section: opens in your default terminal, adds a pane, shows last activity.
- Full session list, sorted by activity (attached first, then most recently used), up to 8 shown with a "view all" entry.
- The tray menu follows the system language (English / Simplified Chinese) and refreshes every 5 seconds.

### Terminal brand icons (v1.8)

Each card's footer is now a row of real terminal icons (extracted from each installed app bundle at runtime, with built-in SVG fallbacks). One click opens the session in that terminal — no more dropdown, no more guessing which terminal is selected. The default terminal is highlighted.

### Streamlined card header (v1.9)

Card headers are now just a status dot, a click-to-rename name, and a close button. The hover-only rename/delete buttons and the "X windows · Y panes" statistics line are gone; the status color still tells you attached / running-in-background / idle at a glance.

### Pane-level management (v1.10)

Workspaces are no longer all-or-nothing. Hover a pane preview tile to kill just that pane (with confirmation; hidden when the session has a single pane), or click "Add pane" to grow the grid — each new pane inherits the session's working directory.

### Duplicate-window protection (v1.11)

Clicking a session that is already attached no longer spawns another terminal window. TmuxDeck focuses the existing window instead — precisely via window title on macOS (falls back to activating the terminal app without permission prompts), and via AppActivate on Windows. Click five times, you still get one window.

### Fixes

- Tray menu labels are now localized (previously hard-coded English).
- Removed dead activity-text computation and orphaned i18n keys after the header cleanup.

### Install

- macOS: download the `.dmg`, drag into Applications. If Gatekeeper warns on first launch, right-click -> Open (unsigned build).
- Windows: download the `.exe` (NSIS) or `.msi`.
