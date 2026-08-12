# TmuxDeck

*[English](README.md) · [简体中文](README.zh-CN.md)*

**Ten agents are running. Which one is waiting for you?**

TmuxDeck is a control surface for working with many AI coding agents at once. Each agent runs in its own tmux pane; TmuxDeck shows you all of them, tells you which one needs a human, and lets you answer it.

Built with [Tauri](https://tauri.app/). macOS is the primary platform; Windows works through WSL.

![TmuxDeck Dashboard](docs/assets/dashboard-en.png)

---

## At a glance

- **Orchestrate many agents at once.** Every workspace is a card; every pane shows what's running and how long it has been quiet.
- **Native Ghostty splits.** 1/2/4/6-pane grids, each agent in its own tmux session — close the window, the agent keeps working.
- **Plays well with classic setups.** Plain terminals and tmux multi-pane layouts are fully supported.
- **Runs what you already use.** Pi, Claude Code, Codex, OpenCode, Gemini CLI, Aider, custom commands, or a plain shell — detected at runtime.
- **One-click control.** Create, start, resume, kill a single agent, or destroy a whole workspace from the dashboard.
- **Lives in the menu bar.** Close the window and it keeps running — status, previews, and control stay one click away.
- **Agents find each other.** Registers on the Agent Intercom broker for cross-harness discovery, live status, and directed messaging.
- **macOS first, WSL-ready.** Built on Tauri; Windows runs through WSL.

---

## The idea

Running one agent is easy — you watch it. Running twelve is a different problem entirely.

They finish at different times. They block on questions you did not anticipate. They wait quietly, and a stalled agent looks exactly like a busy one. The work stops being *writing prompts* and starts being **triage**: out of everything running, which one needs me right now?

```
   ┌─ project-api ───────────┐   ┌─ mes-refactor ──────────┐
   │  ◐  pi        tool:bash │   │  ●  claude    thinking  │
   │  ○  pi        idle      │   │  ◐  codex     tool:edit │
   └─────────────────────────┘   └─────────────────────────┘

   ┌─ wms-migrate ───────────┐   ┌─ docs ──────────────────┐
   │  ●  pi        thinking  │   │  ▲  claude    waiting   │ ←── needs you
   │  ○  zsh                 │   │  ○  zsh                 │
   └─────────────────────────┘   └─────────────────────────┘

        ●  working      ◐  running a tool
        ○  idle         ▲  waiting for a human
```

That last card is the whole point. Everything else can wait.

---

## Three layers

```mermaid
flowchart LR
    A["<b>See</b><br/>Which one needs me?<br/><i>shipped</i>"]
    B["<b>Speak</b><br/>Answer it in one line<br/><i>v1.12, in progress</i>"]
    C["<b>Anywhere</b><br/>Even away from the desk<br/><i>planned</i>"]
    A --> B --> C
```

**See** — every session is a card, every pane shows what is running and how long it has been quiet. One click reattaches in your terminal of choice. This is what ships today.

**Speak** — a stalled agent is only useful if you can unstick it. TmuxDeck can send text into any pane, so answering an agent does not require finding its window first.

**Anywhere** — the triage problem does not stop when you leave your desk. Agents that block at 9pm sit blocked until morning unless something reaches you.

---

## Agents already talk to each other. You were the missing participant.

Coding agents are growing their own coordination layer — [Agent Intercom](https://github.com/dataforxyz/agent-intercom-pi) gives Pi, Codex, Claude Code, and OpenCode sessions a shared local broker so they can find and message each other.

What that bus has no adapter for is **the human**.

```mermaid
flowchart TB
    subgraph bus["intercom broker"]
        direction LR
        P1["pi<br/>planner"]
        P2["pi<br/>worker"]
        CC["claude<br/>reviewer"]
    end

    ME["<b>TmuxDeck</b><br/>registered as <code>me</code>"]

    P1 <--> P2
    P2 <--> CC
    bus <-->|"ask / send"| ME
    ME -.->|push| PHONE["your phone"]
```

TmuxDeck registers on that broker as a session named `me`. An agent that needs a decision addresses you the same way it would address another agent — and because the broker already tracks who is idle, who is thinking, and who is blocked waiting for a reply, **the "which one needs me" question is answered by data, not guesswork**.

> Status: trusted-LAN mobile UI available with desktop QR pairing (plaintext trusted LAN only); physical-phone acceptance and push-when-browser-closed remain pending.

---

## Features

Shipped today:

- **Session overview.** Every tmux session is a card with window count, pane count, per-pane commands, and last-activity time.
- **One-click workspace creation.** Name a session, pick a directory, choose an agent, a pane count, and a terminal. Panes are created and the terminal opens automatically.
- **Works with what you have.** Terminals and agents are detected at runtime; uninstalled ones are hidden. Terminals: Ghostty, iTerm2, WezTerm, kitty, Alacritty, system Terminal. Agents: Claude Code, Codex, OpenCode, Gemini CLI, Aider, Pi, or a plain shell.
- **Lives in the menu bar.** Close the window and TmuxDeck keeps running — open a session, add a pane, or create a workspace without reopening the main window.
- **Pane-level control.** Hover a pane preview to kill just that pane, or add 1, 2, or 4 panes in one atomic action; native workspaces rebuild their layout only once.
- **Workspace-aware mobile conversations.** The trusted-LAN mobile view groups Agents by authoritative workspace metadata and offers compact Markdown chat, awaiting-human prioritization, context actions, and safe transcript-source labeling.
- **No duplicate windows.** Clicking a session that is already open focuses its window instead of spawning another terminal.
- **Remembers your choices.** Last terminal, agent, and pane count persist to the platform config directory.
- **No setup required.** With nothing else installed, it falls back to the system terminal and your shell.

Conversation bridge foundation: directed pane input, intercom broker client, structured transcripts, unified conversation model, desktop QR pairing, and trusted-LAN mobile UI (plaintext trusted LAN only; physical-phone acceptance and offline push remain pending).

---

## Quick Start

### 1. Install Prerequisites & Agent CLIs

```bash
# Required: tmux multiplexer
brew install tmux

# Optional: AI Agent CLIs
npm install -g @earendil-works/pi-coding-agent
npm install -g @anthropic-ai/claude-code
npm install -g @openai/codex
npm install -g opencode-ai
```

### 2. Set up Agent Intercom (Optional)

Enable cross-harness discovery, live status, and direct messaging across agent sessions:

| Agent | Adapter Installation | Registration / Activation |
| :--- | :--- | :--- |
| **Pi** | `pi install npm:@dataforxyz/agent-intercom-pi` | Automatic on start (`/reload` in open sessions) |
| **Claude Code** | On macOS, install TmuxDeck's pinned Managed Adapter from the Create Workspace modal; global npm is not changed. | Choose **Use Managed** or persistently switch to **Standard Claude**. A global `cci` is left untouched and may still be used as a custom command. |
| **Codex** | `npm install -g @dataforxyz/agent-intercom-codex` | `codex mcp add codex-intercom -- codex-intercom-mcp` |
| **OpenCode** | `cd ~/.config/opencode && npm install @dataforxyz/agent-intercom-opencode` | Register `plugin.mjs` & `tui.mjs` in `opencode.json` & `tui.json`; `tui.mjs` adds `/intercom`, `/intercom-name`, and `/intercom-id` |

### 3. Use Intercom

Communicate across agent sessions using the shared broker:

- **Session discovery & messaging:** Use `intercom_list`, `intercom_send`, `intercom_ask`, and `intercom_reply` to discover and exchange messages.
- **Claude Code integration:** On macOS, TmuxDeck can install or repair its offline, pinned **Managed Claude Intercom** adapter from the Create Workspace modal. The installer verifies the bundled SHA-256, rejects unsafe archive entries, validates the Claude plugin → Monitor → runtime chain, and never modifies global npm. Each newly created managed pane or Ghostty native slot starts Claude in explicit safe permission mode and gets a cryptographically random Intercom ID that remains attached to that pane/slot for its lifetime, plus a readable workspace/pane name. This ID is routing metadata, not an authentication credential. **Use Standard Claude** is persistent; installing/repairing or choosing **Use Managed** switches back. Windows/WSL keeps Standard Claude behavior. Existing global `cci` installations are not selected as Managed, changed, or removed; use a custom Agent command if you intentionally want one. Custom commands are never rewritten.
- **OpenCode integration:** Requires registering both `plugin.mjs` (in `opencode.json`) and `tui.mjs` (in `tui.json`).
- **Rename an OpenCode Intercom session:** Run `/intercom-name`, or choose **Rename intercom session** in the command palette; the prompt is titled **Rename this Intercom session**. The model can also call `intercom_set_name({ name: "<new-name>" })`. This changes only the discoverable name, not the stable Intercom session ID.

See [docs/GUIDE-cross-harness-agent-intercom.md](docs/GUIDE-cross-harness-agent-intercom.md) for complete configuration instructions.

---

## Requirements

- macOS (Apple Silicon; Intel builds supported from source)
- [tmux](https://github.com/tmux/tmux) — `brew install tmux`

Terminals and agents are optional; the app offers only what you have installed.

## Installation

Download the latest Apple Silicon (`aarch64`) `.dmg` release from the [Releases page](https://github.com/ctliz/TmuxDeck/releases) and drag `TmuxDeck.app` into Applications.

Release builds are ad-hoc signed but not notarized. On first launch, right-click `TmuxDeck.app`, choose **Open**, and confirm. If macOS reports that the application is damaged or cannot be opened, run:

```bash
xattr -cr /Applications/TmuxDeck.app
```

## Usage

1. Open TmuxDeck.
2. Click **New Workspace**.
3. Enter a name, pick a directory, choose the agent, pane count, and terminal.
4. Click **Create**.

![New Workspace Setup](docs/assets/create-workspace-en.png)

The terminal opens attached to the new session. Closing the terminal window does not destroy the workspace — the session keeps running and can be reopened any time. Only the delete button on a card destroys a session.

## Configuration

Settings are written automatically to `~/Library/Application Support/tmuxdeck/config.json` on macOS and `%APPDATA%\tmuxdeck\config.json` on Windows.

```json
{
  "default_terminal": "ghostty",
  "default_agent": "pi",
  "default_panes": 4,
  "custom_agent": { "name": "Claude Opus", "command": "claude --model opus" },
  "recent_dirs": ["/Users/you/projects/foo"]
}
```

`custom_agent` adds a user-defined agent command to the create dialog.

## FAQ

**Do I need Ghostty or Claude Code to use TmuxDeck?**

No. The app detects what is installed and hides what is not. With nothing installed it uses the system terminal and your shell.

**Will my agents be killed if I close TmuxDeck?**

No. Workspaces live in tmux, not in the app. Closing the app or a terminal window leaves sessions running. Only the delete button destroys a session.

**Do I need Agent Intercom?**

No. Without it TmuxDeck is the dashboard described above. With it, agent status becomes exact rather than inferred, and agents can address you directly.

**Why is a terminal missing from the options?**

Only installed terminals are shown. If a category has a single candidate, the row is hidden rather than shown as a fixed choice. If you installed one in a non-standard location and it is not detected, open an issue.

**Does TmuxDeck support Linux or Windows?**

Linux is not supported yet. Windows works through WSL and ships the same installers, but macOS is the battle-tested platform — please report Windows issues on GitHub.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and code conventions, and [docs/](docs/README.md) for architecture, protocol reference, and decision records.

## Contributors

- [@ctliz](https://github.com/ctliz) — author and maintainer
- [Claude](https://claude.com/claude-code) — implementation via Claude Code

## License

[MIT](LICENSE)
