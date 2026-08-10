# TmuxDeck

*[English](README.md) · [简体中文](README.zh-CN.md)*

A desktop dashboard for tmux sessions that run AI coding agents.

TmuxDeck turns your tmux sessions into a visual dashboard. Each session is a card showing what is running in every pane, when it was last active, and whether it is still live. One click reattaches to a session in your terminal of choice.

It is built with [Tauri](https://tauri.app/) and runs on macOS. Windows support is in progress.

## Why TmuxDeck

tmux is famously minimal, but that minimalism has a cost: everything lives in your head. Session names, pane layouts, which agent is where. When you manage a handful of workspaces, that is fine. When every project spawns multiple agent conversations and you have dozens going at once, finding the right one becomes the daily bottleneck.

TmuxDeck gives that back to you visually: one click to create, one glance to see everything, one click to get back in. It also lowers the barrier for people who never learned tmux commands — the dashboard is the interface, tmux stays in the background.

## Features

- **Session overview.** Every tmux session appears as a card with window count, pane count, per-pane commands, and last-activity time.
- **One-click workspace creation.** Name a session, pick a directory, choose an agent, a pane count, and a terminal. The panes are created and the terminal opens automatically.
- **Works with what you have.** Installed terminals and agents are detected at runtime; uninstalled ones are hidden. Supported terminals: Ghostty, iTerm2, WezTerm, kitty, Alacritty, and the system Terminal. Supported agents: Claude Code, Codex, OpenCode, Gemini CLI, Aider, Pi, or a plain shell.
- **Remembers your choices.** The last terminal, agent, and pane count are saved to `~/.config/tmuxdeck/config.json` and restored on the next launch.
- **No setup required.** If no third-party terminal or agent is installed, TmuxDeck falls back to the system terminal and the default shell.

## Requirements

- macOS (Apple Silicon or Intel)
- [tmux](https://github.com/tmux/tmux) — install with:

  ```sh
  brew install tmux
  ```

Terminals and agents are optional. The app detects what you have installed and offers only those options.

## Installation

Download the latest release from the [Releases page](https://github.com/ctliz/TmuxDeck/releases) and drag the `.dmg` into Applications.

If macOS warns that the app cannot be verified, right-click the app and choose Open, then confirm. This is expected for unsigned builds.

## Usage

1. Open TmuxDeck.
2. Click **New Workspace**.
3. Enter a name, pick a directory, and choose the agent, pane count, and terminal.
4. Click **Create**.

The terminal opens attached to the new session. Closing the terminal window does not destroy the workspace — the session keeps running and can be reopened from the dashboard at any time. Only the delete button on a card destroys a session.

## Configuration

Settings are stored in `~/.config/tmuxdeck/config.json` and are written automatically. You normally do not need to edit this file.

```json
{
  "default_terminal": "ghostty",
  "default_agent": "pi",
  "default_panes": 4,
  "custom_agent": { "name": "Claude Opus", "command": "claude --model opus" },
  "recent_dirs": ["/Users/you/projects/foo"]
}
```

The `custom_agent` entry adds a user-defined agent command to the create dialog.

## FAQ

**Do I need Ghostty or Claude Code to use TmuxDeck?**

No. The app detects what is installed and hides what is not. If nothing is installed, it uses the system terminal and your shell.

**Will my agents be killed if I close TmuxDeck?**

No. Workspaces live in tmux, not in the app. Closing the app or a terminal window leaves the sessions running. Only the delete button destroys a session.

**Why is a terminal missing from the options?**

Only installed terminals are shown. If a category has a single candidate, the whole row is hidden rather than shown as a fixed choice. If you installed a terminal in a non-standard location and it is not detected, open an issue.

**Does TmuxDeck support Linux or Windows?**

Currently macOS only. Windows support (via WSL) is in development.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, the terminal/agent registry, and code conventions.

## License

[MIT](LICENSE)
