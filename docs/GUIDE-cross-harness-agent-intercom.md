# Cross-Harness Agent Intercom Usage Guide

> Scope: Pi, OpenCode, Codex, and Claude Code on the same machine, under the same OS user.
>
> The four adapters share Agent Intercom protocol v4, the local broker, and the runtime directory, so they can perform targeted `list` / `send` / `ask` / `reply` across harnesses. It is not a public-internet messaging service, nor a broadcast chat room.

## 1. Core model

```text
Pi ───────────┐
OpenCode ─────┼── ~/.pi/agent/intercom/broker.sock ── local broker
Codex ────────┤
Claude Code ──┘
```

- The first adapter to connect starts the broker automatically, so Pi does not have to start first.
- The broker exits on its own roughly 5 seconds after the last client disconnects.
- macOS / Linux use a Unix socket; Windows uses a named pipe by default.
- Only sessions that are installed, loaded, and successfully registered appear in listings.
- Session names are for human-readable addressing, but duplicates are allowed; the trustworthy addressing key is the **stable session ID**.
- **Broker-enforced workspace scoping (v4):** The local broker enforces workspace scope boundaries. Discovery (`intercom_list`), peer names, and short ID prefixes resolve strictly within the caller's active workspace scope.
- **Cross-scope routing:** Communicating across workspace scopes requires providing the **exact full session ID**.
- **Scope is same-OS-user isolation, not a security principal:** Workspace scoping partitions discovery and routing to avoid accidental cross-workspace interference; it is an operational routing boundary, not a cryptographic authentication or authorization boundary. The trust boundary remains the local OS user.
- **Zero raw scope exposure for frontend & mobile:** TmuxDeck desktop UI and mobile clients maintain zero raw scope exposure (零原值暴露); the backend manages an independent scoped human client per workspace and aggregates conversations into the unified view.
- **Legacy workspaces fail closed:** Workspaces created without v4 scoping metadata fail closed on pane add or rename operations and should be recreated.
- **Orchestrator deployment:** Orchestrator is an optional Linux/systemd lifecycle product, outside the Broker compatibility set; omitted on macOS.

Default shared directory:

```text
~/.pi/agent/intercom/
```

It contains `broker.sock`, `broker.pid`, `broker.owner`, `broker-asks.json`, `inbox/`, `outbox/`, and `config.json`.

## 2. Version and installation principles

All active sides must use protocol-v4-compatible `agent-intercom-*` adapters (`ctliz` ecosystem, with `@dataforxyz` provenance). Mixing incompatible protocol versions can form broker "islands" that cannot see each other.

**Installed adapters only:** Coordinated upgrades apply only to the adapters currently installed and in use on your machine. You do not need to install adapters for uninstalled harnesses.

The v4 ecosystem packages are published under the `@ctliz` npm scope (with canonical core `@ctliz/agent-intercom-core@0.2.0`, published on GitHub v0.2.0; npm registry availability required for offline lockfile checks). Canonical and recommended installation commands specify the `@connect` dist-tag (e.g. `@ctliz/agent-intercom-codex@connect`) or exact package versions. Future GA releases will advance the `latest` dist-tag.

After installing or upgrading any adapter, run `/reload` in **every still-open Pi session** and restart every companion Claude, Codex, and OpenCode adapter. This lets the old broker exit after its final client disconnects and allows one restarted compatible client to start the shared protocol-v4 broker cleanly.

### 2.1 Pi

Recommended install and update command:

```bash
pi install git:github.com/ctliz/agent-intercom-pi@v0.12.0-connect.1
```

The Git fixed-tag install is recommended for Pi (requires registry Core 0.2.0 availability for package-lock resolution). The official npm package `@ctliz/agent-intercom-pi@connect` is also published.

Provenance:

- Release tag: [`v0.12.0-connect.1`](https://github.com/ctliz/agent-intercom-pi/releases/tag/v0.12.0-connect.1)
- Upstream base: `@dataforxyz/agent-intercom-pi` provenance

After install or update, run this in every open Pi session:

```text
/reload
```

Alternatively, quit and restart Pi. Also restart companion Claude, Codex, and OpenCode adapters so all clients reconnect through a clean protocol-v4 broker lifecycle.

### 2.2 OpenCode

Install the server plugin:

```bash
mkdir -p ~/.config/opencode
cd ~/.config/opencode
npm install @ctliz/agent-intercom-opencode@connect
```

Register the server plugin in `~/.config/opencode/opencode.json`. Do not use `~` in the JSON; the absolute path is required:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "plugin": [
    "/Users/you/.config/opencode/node_modules/@ctliz/agent-intercom-opencode/dist/plugin.mjs"
  ]
}
```

Register the TUI plugin in `~/.config/opencode/tui.json` to provide `/intercom`, `/intercom-name`, `/intercom-id`, and the Alt+M / Alt+I shortcuts:

```json
{
  "$schema": "https://opencode.ai/tui.json",
  "plugin": [
    "/Users/you/.config/opencode/node_modules/@ctliz/agent-intercom-opencode/dist/tui.mjs"
  ]
}
```

Notes:

- `dist/plugin.mjs` belongs only in `opencode.json`.
- `dist/tui.mjs` belongs only in `tui.json`.
- After config or package changes, fully quit and restart OpenCode; the TUI plugin cannot be hot-reloaded the way Pi's `/reload` does.
- Plain workers need no wrapper; run `opencode` directly.

### 2.3 Codex

Install the global adapter:

```bash
npm install -g @ctliz/agent-intercom-codex@connect
```

Register the MCP server for ordinary Codex sessions:

```bash
codex mcp add codex-intercom -- codex-intercom-mcp
```

Verify:

```bash
codex mcp list
```

The package also provides:

- `codex-intercom-mcp`: tools inside ordinary Codex sessions.
- `coi`: a Codex wrapper that can be woken by messages, with Alt+M / Alt+I.
- `codex-intercom-bridge`: advanced use for publishing multiple background Codex workers.

After updating, restart ordinary Codex sessions and all `coi` workers.

### 2.4 Claude Code

On macOS, select Claude Code in TmuxDeck's **Create Workspace** modal and choose **Install Managed Adapter**. TmuxDeck installs its pinned adapter from the app bundle without contacting npm or changing any global npm package. The same control becomes **Repair Managed Adapter** if health verification fails.

The managed resource corresponds to `@ctliz/agent-intercom-claude@connect` (`0.13.0-connect.1`), bundled as `ctliz-agent-intercom-claude-0.13.0-connect.1.tgz` and published with source at <https://github.com/ctliz/agent-intercom-claude/releases/tag/v0.13.0-connect.1>, retaining original `@dataforxyz/agent-intercom-claude` provenance. It packages the Claude Monitor files required for `cci --tui --safe`. Its AGPL license and third-party notices are preserved in the installed directory; the exact artifact digest is recorded in `src-tauri/resources/README.md`.

Installation verifies the pinned SHA-256, rejects links, devices, absolute paths and `..`, stages the replacement, validates the Claude plugin → Monitor → MCP/runtime chain, and rolls back if validation or config persistence fails. A healthy existing version remains in place after a failed repair.

Each newly created managed tmux pane or Ghostty native slot explicitly passes `--safe` to Claude (running `cci --tui --safe`) and receives a cryptographically random ID such as `tmuxdeck-<random-uuid-shape>` and a readable name such as `<workspace> · Claude 01`. The ID is persisted in the existing pane/slot's tmux metadata for that worker incarnation. Recreating a worker produces a new ID. It is routing metadata and consistency evidence, not an authentication credential.

**Use Standard Claude** persists the preference and launches the independently detected `claude` binary. **Use Managed Claude** switches back without reinstalling when the managed adapter is healthy. Installing or repairing also switches back to Managed. Windows/WSL continues to use Standard Claude and does not show managed actions.

TmuxDeck does not select, modify, migrate, or delete a global `cci`. Advanced users may still launch one through a custom Agent command; custom commands are passed through unchanged.

## 3. Starting, naming, and stable identity

### 3.1 Pi

Name at launch:

```bash
pi --name <name>
```

Rename inside a session:

```text
/name <new-name>
```

The Pi adapter uses Pi's own session ID directly as the intercom session ID:

- `/name` changes only the human-readable name, not the stable ID.
- Resuming the same Pi session keeps the intercom ID.
- Creating a new Pi session, even with the same name, gets a new ID.
- To explicitly reuse an identity, resume an existing session via `pi --session <path-or-id>`; for advanced scenarios, `pi --session-id <uuid>` creates or opens a session with a specific ID.

### 3.2 OpenCode

Ordinary start:

```bash
OPENCODE_INTERCOM_NAME=<name> \
OPENCODE_INTERCOM_SESSION_ID=<stable-id> \
opencode /path/to/project
```

Resume an OpenCode conversation:

```bash
OPENCODE_INTERCOM_NAME=<name> \
OPENCODE_INTERCOM_SESSION_ID=<stable-id> \
opencode /path/to/project --session <opencode-session-id>
```

`OPENCODE_INTERCOM_SESSION_ID` is the Intercom identity; `opencode --session` refers to the OpenCode conversation. They are not the same concept.

Without a stable ID, the adapter generates a temporary ID containing the PID, which changes after a process restart.

### 3.3 Codex

For workers that must continuously receive tasks, start with `coi`:

```bash
coi \
  --name <name> \
  --id <stable-id> \
  --cwd /path/to/project
```

- `--name` is the human-readable name.
- `--id` is the stable intercom session ID.
- `coi` saves state to the shared intercom directory by default; restarting with the same `--id` continues its app-server thread.
- Only the interactive terminal started via `coi` has Alt+M / Alt+I; ordinary Codex + MCP has the tools but not these shortcuts.

Ordinary MCP sessions can also pin their identity at registration:

```bash
codex mcp add <mcp-name> \
  --env CODEX_INTERCOM_NAME=<name> \
  --env CODEX_INTERCOM_SESSION_ID=<stable-id> \
  --env CODEX_INTERCOM_MODEL=codex \
  -- codex-intercom-mcp
```

Do not let two concurrent Codex processes share one pinned ID; in a multi-worker scenario, give each worker its own `coi --id`.

### 3.4 Claude Code

Start wakeable workers with `cci` or `ccim`:

```bash
cci \
  --tui \
  --safe \
  --name <name> \
  --id <stable-id> \
  --cwd /path/to/project
```

Minimal worker:

```bash
ccim \
  --name <name> \
  --id <stable-id> \
  --cwd /path/to/project
```

- Reusing the same `--id` reuses that worker's persistent state and Claude conversation.
- The Claude conversation ID can be viewed separately with `claude --resume <session-id>`; it differs from the intercom `--id`.
- `ccim`'s woken turns use safe mode: it can still receive work and auto-reply, but cannot actively call the MCP intercom tools to contact other peers within a turn.
- For a truly interactive Claude TUI to be woken in place, use `cci --tui --safe --name ... --id ...`.

Ordinary MCP sessions can pin their identity:

```bash
claude mcp add -s user <mcp-name> \
  --env CLAUDE_INTERCOM_NAME=<name> \
  --env CLAUDE_INTERCOM_SESSION_ID=<stable-id> \
  --env CLAUDE_INTERCOM_MODEL=opus \
  -- claude-intercom-mcp
```

Concurrent Claude processes must not share one pinned ID (see [section 9](#9-stable-session-id-notes)).

## 4. Unified rename capability

A runtime rename only updates the `name` visible to other peers; **it does not change the stable intercom session ID**. Before and after a rename it remains the same contact target, and existing pending asks and ID-based addressing are unaffected.

| Harness | Rename entry for the current session | How it survives a restart |
|---|---|---|
| Pi | native `/name <new-name>`; the adapter syncs the Pi session name to Intercom automatically | resume the same Pi session |
| OpenCode | `/intercom-name` opens a rename input, or call `intercom_set_name({ name: "<new-name>" })` | keep setting `OPENCODE_INTERCOM_NAME` |
| Codex ordinary MCP session | `intercom_set_name({ name: "<new-name>" })` | keep setting `CODEX_INTERCOM_NAME`; use `--name` at launch for `coi` workers |
| Claude Code ordinary MCP session | `intercom_set_name({ name: "<new-name>" })` | keep setting `CLAUDE_INTERCOM_NAME`; use `--name` at launch for `cci` / `ccim` workers |

**How the OpenCode rename entry works:** `tui.mjs` registers the `/intercom-name` slash command and the **Rename intercom session** command-palette action. Selecting it opens a prompt titled **Rename this Intercom session**. After confirmation, the TUI plugin sends a private local control request (`{ type: "set_name", name }`) to the already-connected `plugin.mjs` server plugin. The server calls `runtime.setName`, updates the name published in broker presence, and keeps the existing stable Intercom session ID. The same runtime operation is exposed to the model as `intercom_set_name({ name: "<new-name>" })`; no second broker connection or identity is created.

Runtime renames for OpenCode, Codex, and Claude Code only affect the current process; after a restart they re-read the environment variable or wrapper argument. Background workers should be named at launch:

```bash
coi --name <name> --id <stable-id> --cwd /path/to/project
cci --tui --safe --name <name> --id <stable-id> --cwd /path/to/project
ccim --name <name> --id <stable-id> --cwd /path/to/project
```

Headless `cci` / `ccim` have no interactive console and cannot type slash commands like `/name`; they can only be named via the `--name` launch argument. Ordinary Claude MCP sessions use `intercom_set_name` to rename.

## 5. Shortcuts and command entries

| Action | Pi | OpenCode | Codex | Claude Code |
|---|---|---|---|---|
| Runtime rename | `/name <new-name>` | `/intercom-name` or `intercom_set_name` | ordinary MCP: `intercom_set_name`; `coi` prefers `--name` at launch | ordinary MCP: `intercom_set_name`; `cci` / `ccim` prefer `--name` at launch |
| Pick a peer and send | `/intercom` or Alt+M | `/intercom` or Alt+M | Alt+M in `coi` | plugin provides `/claude-intercom:intercom`; Alt+M in `cci` / `ccim` |
| Copy the exact current contact target | `/intercom-id` or Alt+I | `/intercom-id` or Alt+I | Alt+I in `coi` | plugin provides `/claude-intercom:intercom-id`; Alt+I in `cci` / `ccim` |
| List navigation | ↑ / ↓ | ↑ / ↓ | wrapper prompt flow | wrapper prompt flow |
| Send | Enter | Enter | confirm per prompt | confirm per prompt |
| Multi-line newline | Shift+Enter | Shift+Enter | handled by Codex composer | handled by Claude composer/worker |
| Cancel | Escape | Escape | Escape | Escape |

If Alt+M / Alt+I do nothing, first check whether the terminal passes Option/Alt to the app as the Meta key, then confirm you are using a shortcut-capable entry point: Codex must be `coi`, Claude must be `cci` / `ccim`, and OpenCode must have `tui.mjs` loaded.

Claude's `/claude-intercom:intercom` and `/claude-intercom:intercom-id` require the Claude plugin to be installed or loaded per session; an ordinary session that only ran `claude mcp add` still has the Intercom tools but lacks these two plugin slash commands.

The content copied by `/intercom-id` or Alt+I is cross-harness: it uses the name when unique within the workspace scope, and falls back to the stable ID when names are duplicated or when addressing cross-scope.

## 6. Agent tools: set name / list / send / ask / reply

`list`, `send`, `ask`, and `reply` use the same protocol concepts across all four adapters. Pi uses the native `/name`; OpenCode, Codex ordinary MCP, and Claude Code ordinary MCP expose `intercom_set_name` when supported.

### 6.1 Set the current human-readable name

OpenCode, Codex ordinary MCP, or Claude Code ordinary MCP sessions:

```typescript
intercom_set_name({
  name: "<new-name>"
})
```

It only updates the human-readable name; it does not change the stable session ID returned by `intercom_status({})`.

Pi uses:

```text
/name <new-name>
```

### 6.2 View connection status

```typescript
intercom_status({})
```

Confirms the current session ID, broker connection, and pending messages.

### 6.3 List peers in the current workspace

In Agent Intercom protocol v4, `intercom_list({})` is scoped to the current workspace by default and enforced by the broker:

```typescript
intercom_list({})
```

Returns the current session and connected peer sessions in the same workspace with short ID, cwd, model, and live status. Names and ID prefixes resolve within the active workspace scope. An exact full session ID is required for cross-workspace routing.

If the current worker is managed by an orchestrator, prefer viewing the team under the same manager:

```typescript
intercom_team({})
```

### 6.4 Non-blocking notification: send

```typescript
intercom_send({
  to: "<peer-name-or-id>",
  message: "Please check the retry logic in src/api/client.ts and report back when done."
})
```

`send` only waits for the broker to accept the message and the peer to confirm durable enqueue; it does not wait for the peer to finish work or reply. Suitable for task dispatch, progress, and completion notifications.

### 6.5 When you need an answer: ask

```typescript
intercom_ask({
  to: "<peer-name-or-id>",
  message: "Does this change need to stay compatible with the old error format?"
})
```

`ask` only does a finite-duration foreground wait. A timeout is not a cancel: the request turns async, and a late reply still arrives as a new message. For long tasks, do not block-wait; use `send` instead and check status afterward.

### 6.6 Reply to a received ask: reply

In the current turn triggered by the received ask:

```typescript
intercom_reply({
  message: "Must stay compatible with the old format; only add new fields."
})
```

To reply later, when multiple senders are waiting:

```typescript
intercom_pending({})

intercom_reply({
  to: "<sender-name-or-id>",
  message: "Must stay compatible with the old format; only add new fields."
})
```

`to` is the sender's name or stable ID, not a message/thread ID. The active ordinary-message batch context is preserved across provider/tool loops.

## 7. `PI_CODING_AGENT_DIR`

All four adapters read `PI_CODING_AGENT_DIR`, which replaces the default `~/.pi/agent` base directory entirely:

```bash
export PI_CODING_AGENT_DIR="$HOME/.pi/agent"
```

The actual intercom directory becomes:

```text
$PI_CODING_AGENT_DIR/intercom/
```

Rules:

1. All harnesses that should discover each other must use the **same absolute path**.
2. Different values form independent broker islands that can't see each other; this capability is only for deliberate isolation.
3. Don't set it for Pi alone and miss OpenCode, `coi`, or `cci`.
4. After changing this variable, reload/restart all existing sessions.
5. Keep it consistent across shell aliases, tmux/Ghostty launch commands, LaunchAgents, and IDE launch environments.

## 8. Reload, restart, and upgrades

| Scenario | Action |
|---|---|
| Pi extension install/update | re-run the fixed-tag `pi install` command, then run `/reload` in every open Pi session or restart Pi |
| OpenCode plugin/config update | fully quit and restart OpenCode |
| Codex MCP/package update | restart ordinary Codex sessions; restart `coi` workers reusing their original `--id` |
| Claude MCP/package update | restart ordinary Claude sessions; restart `cci` / `ccim` reusing their original `--id` |
| `PI_CODING_AGENT_DIR` change | reload/restart all installed sides |
| Broker auto-restart | clients reconnect automatically; usually no manual action needed |

**Coordinated upgrades apply to installed adapters only:** When updating protocol versions, only upgrade the adapters currently installed and configured on your machine. Reload every open Pi session with `/reload` and restart active companion adapters. Uninstalled adapters do not need to be installed.

Recommended troubleshooting order:

1. Run `intercom_status({})` on the current side.
2. Confirm all sides use the same `PI_CODING_AGENT_DIR`.
3. Confirm the adapter is loaded: Pi extension, OpenCode's two plugins, Codex/Claude MCP or wrappers.
4. Confirm Pi is installed from `git:github.com/ctliz/agent-intercom-pi@v0.12.0-connect.1`; re-run that fixed `pi install` command if repair is needed.
5. Run `/reload` in every open Pi session; fully restart Claude (`cci`), Codex (`coi`), and OpenCode companion adapters so the protocol-v4 broker can restart cleanly.
6. Run `intercom_list({})` for the current workspace. Use the exact full ID for intentional cross-workspace routing.
7. If an existing workspace fails closed on add/rename, recreate the workspace under v4.
8. Only when all clients are confirmed exited should leftover runtime files be considered for cleanup; never delete the socket during active sessions.

## 9. Stable session ID notes

1. **A name is not an identity.** Duplicate names are allowed within a scope; sending to a duplicated name fails, so use the stable ID.
2. **Don't reuse an ID concurrently.** One stable ID should be registered by only one process at a time.
3. **Restore the same identity.** Pi resumes the original session; OpenCode reuses `OPENCODE_INTERCOM_SESSION_ID`; Codex/Claude wrappers reuse `--id`.
4. **A harness conversation ID is not the intercom ID.** OpenCode `--session`, Codex thread ID, and Claude `--resume` are each harness-specific conversation identifiers.
5. **Pending asks depend on identity.** If the intercom ID changes after a restart, the reply authorization for old asks does not automatically transfer to the new identity.
6. **Copy exact targets.** Prefer Alt+I or `/intercom-id` to get a pasteable cross-harness contact.
7. **Cross-scope messaging.** Always use the exact full session ID when messaging across different workspace scopes.

## 10. Local environment verification

Verified on this machine for v1.14.0:

```text
Pi package source: git:github.com/ctliz/agent-intercom-pi@v0.12.0-connect.1 (or @ctliz/agent-intercom-pi@connect)
Pi package version: @ctliz/agent-intercom-pi 0.12.0-connect.1
Claude package:     @ctliz/agent-intercom-claude@connect (0.13.0-connect.1, --tui --safe)
Codex package:      @ctliz/agent-intercom-codex@connect (0.12.0-connect.1)
OpenCode package:   @ctliz/agent-intercom-opencode@connect (0.12.0-connect.1)
Core internal:      @ctliz/agent-intercom-core@0.2.0
Protocol:           v4 (broker-enforced workspace scoping & Zero-Manual-Join Auto-Team)
```

Attribution and provenance to original upstream `@dataforxyz/agent-intercom-*` are preserved. Official npm packages are published under the `@ctliz` scope.

## 11. Recommended workflow

```text
1. Launch and set a unique human-readable name in your workspace
2. intercom_list for current workspace peers, or intercom_team for managed coworkers
3. Dispatch tasks with send
4. Only use ask when the next step depends on the answer
5. Reply with reply from the session that received the ask
6. Use exact full session IDs for cross-workspace or high-risk operations
7. After updating installed adapters, reload/restart active sessions together
```

Related docs:

- [Intercom wire protocol reference](./REFERENCE-intercom-protocol.md)
- [Architecture](./ARCHITECTURE.md)
- [Roadmap](./ROADMAP.md)
