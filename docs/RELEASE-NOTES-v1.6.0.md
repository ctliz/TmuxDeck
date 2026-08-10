## TmuxDeck v1.6.0

A major UI overhaul and the first release with a menu bar presence. Since v1.5.0: Liquid Glass design, a tray icon, terminal brand icons, and pane-level management.

### Liquid Glass UI

The interface was rebuilt around macOS 26-style Liquid Glass: translucent surfaces, soft blur, and smooth transitions. The heavy top bar is gone — a floating, semi-transparent search pill sits at the top, and the workspace grid starts right there.

Cards were simplified to the essentials: a status dot, a click-to-rename name, and a close button. The pane grid now shows live output tail from each pane, so you can see what every agent is doing without opening a terminal.

### Menu bar (tray) presence

TmuxDeck now lives in the menu bar. Close the window and it keeps running — click the tray icon to see the active workspace, open any session, add a pane, or create a new one, all without opening the main window.

The tray menu follows the system language (English / Simplified Chinese).

### Terminal brand icons

Each card shows the real icon of every installed terminal (extracted from the app bundle at runtime, with built-in fallbacks). One click opens the session in that terminal — no more dropdown.

### Pane-level management

Workspaces are no longer all-or-nothing. Hover a pane to kill just that pane (with confirmation), or click "Add pane" to grow the grid — each new pane inherits the session's working directory.

### Fixes

- Tray menu labels are now localized (previously hard-coded English).
- Removed dead activity-text computation and orphaned i18n keys.

### Install

- macOS: download the `.dmg`, drag into Applications. If Gatekeeper warns on first launch, right-click -> Open (unsigned build).
- Windows: download the `.exe` (NSIS) or `.msi`.
