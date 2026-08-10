## TmuxDeck v1.5.0

Since v1.3.0, two feature releases: workspace activity status and live pane previews.

### Workspace activity status (v1.4)

Each workspace card now shows its state at a glance:

- **Active** (green) — a terminal is attached to the session.
- **Running in background** (amber) — detached, active within the last 10 minutes.
- **Idle** (gray) — detached and quiet for 10+ minutes, with the last-activity time.

No more guessing whether a session is still alive after closing a terminal window — the dashboard tells you.

### Live pane previews (v1.5)

Preview tiles on each card now show the **tail of each pane's output in real time**, updated every 8 seconds.

- See what each agent is doing without opening a terminal — who is thinking, writing, or done.
- Output is stripped of ANSI codes and tmux chrome; only the last 5 lines are shown.
- Capture pauses automatically when the app window is hidden or minimized.

### Fixes

- Missing English i18n keys for the activity status labels (raw keys were shown in the English UI).
- Circuit breaker now recovers: a pane that failed a few times resumes capture after the next session refresh.
- Release workflow now keeps the release as a draft until published (assets upload no longer auto-publishes).

### Notes

- Windows (via WSL) support landed in v1.3.0 and is covered by the CI build on every commit.
- macOS builds are unsigned; if Gatekeeper warns on first launch, right-click → Open.

### Downloads

- macOS: `TmuxDeck_1.5.0_aarch64.dmg`
- Windows: `TmuxDeck_1.5.0_x64-setup.exe` / `TmuxDeck_1.5.0_x64_en-US.msi`
