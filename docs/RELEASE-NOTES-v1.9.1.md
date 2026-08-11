## TmuxDeck v1.9.1

A patch release fixing the Ghostty "failed to launch" window that appeared when a session no longer existed.

### Fix

- **No more Ghostty error windows for vanished sessions.** Opening a session that no longer exists (for example after deleting it, then pressing Cmd+D to split in Ghostty) used to fail the attach script, making Ghostty pop up a "failed to launch the requested command" window. Two layers of defense now:
  - **Command level:** `open_session` detects a missing session up front and returns a friendly `Session no longer exists` message instead of launching a terminal at all.
  - **Script level:** the attach script now checks the session first. If the session is gone it prints a note and drops you into a shell instead of exiting with an error — so even a Ghostty split that inherits the attach command degrades gracefully.

### Install

- macOS: download the `.dmg`, drag into Applications. If Gatekeeper warns on first launch, right-click -> Open.
- Windows: download the `.exe` (NSIS) or `.msi`.
