# Cross-Harness Agent Intercom Usage Guide

> Scope: Pi, OpenCode, Codex, and Claude Code on the same machine, under the same OS user.
>
> The four adapters share Agent Intercom protocol v3, the local broker, and the runtime directory, so they can perform targeted `list` / `send` / `ask` / `reply` across harnesses. It is not a public-internet messaging service, nor a broadcast chat room.

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

Default shared directory:

```text
~/.pi/agent/intercom/
```

It contains `broker.sock`, `broker.pid`, `broker.owner`, `broker-asks.json`, `inbox/`, `outbox/`, and `config.json`.

## 2. Version and installation principles

All four sides must use protocol-v3-compatible `agent-intercom-*` adapters, and must not be mixed with the older pi-only `nicobailon/pi-intercom`. Mixing incompatible protocol versions can form broker "islands" that cannot see each other. The adapters do not need to have the same package version: the Pi maintenance release below is `v0.10.1-tmuxdeck.1`, while companion adapters may remain on their separately verified protocol-v3 versions.

After installing or upgrading any adapter, run `/reload` in **every still-open Pi session** and restart every companion Claude, Codex, and OpenCode adapter. This lets the old protocol-v3 broker exit after its final client disconnects and allows one restarted compatible client to start the shared broker cleanly.

### 2.1 Pi

Recommended install and update command:

```bash
pi install git:github.com/ctliz/agent-intercom-pi@v0.10.1-tmuxdeck.1
```

Re-run the same fixed-tag command when repairing or reconciling the install. Do not use `npm update` for this maintenance version: it is GitHub-only and is not published as an official npm update.

Provenance:

- Maintenance release: <https://github.com/ctliz/agent-intercom-pi/releases/tag/v0.10.1-tmuxdeck.1>
- Maintenance commit: [`452b63f11d50dcdbbcf8485eb04d19928bbbfb13`](https://github.com/ctliz/agent-intercom-pi/commit/452b63f11d50dcdbbcf8485eb04d19928bbbfb13)
- Upstream base: [`v0.10.0`](https://github.com/dataforxyz/agent-intercom-pi/releases/tag/v0.10.0), commit `85c118453a15b3631b2a1eb289b66a65d1ac6ab2`
- Upstream tracking issue: [dataforxyz/agent-intercom-pi#20](https://github.com/dataforxyz/agent-intercom-pi/issues/20)

After install or update, run this in every open Pi session:

```text
/reload
```

Alternatively, quit and restart Pi. Also restart the companion Claude, Codex, and OpenCode adapters so all clients reconnect through a clean protocol-v3 broker lifecycle.

### 2.2 OpenCode

Install the server plugin:

```bash
mkdir -p ~/.config/opencode
cd ~/.config/opencode
npm install @dataforxyz/agent-intercom-opencode
```

Register the server plugin in `~/.config/opencode/opencode.json`. Do not use `~` in the JSON; the absolute path is required:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "plugin": [
    "/Users/you/.config/opencode/node_modules/@dataforxyz/agent-intercom-opencode/dist/plugin.mjs"
  ]
}
```

Register the TUI plugin in `~/.config/opencode/tui.json` to provide `/intercom`, `/intercom-name`, `/intercom-id`, and the Alt+M / Alt+I shortcuts:

```json
{
  "$schema": "https://opencode.ai/tui.json",
  "plugin": [
    "/Users/you/.config/opencode/node_modules/@dataforxyz/agent-intercom-opencode/dist/tui.mjs"
  ]
}
```

Notes:

- `dist/plugin.mjs` belongs only in `opencode.json`.
- `dist/tui.mjs` belongs only in `tui.json`.
- After config or package changes, fully quit and restart OpenCode; the TUI plugin cannot be hot-reloaded the way Pi's `/reload` does.
- Plain workers need no wrapper; run `opencode` directly.
- Some separately maintained OpenCode installs include a `tui.mjs` patch that lets `/intercom`, `/intercom-name`, or `/intercom-id` create an empty session when OpenCode is on its home page. This behavior is not provided by the Pi maintenance tag and must be verified independently in the installed OpenCode package.

Update:

```bash
cd ~/.config/opencode
npm update @dataforxyz/agent-intercom-opencode
```

### 2.3 Codex

Install the global adapter:

```bash
npm install -g @dataforxyz/agent-intercom-codex
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

After updating, restart ordinary Codex sessions and all `coi` workers:

```bash
npm update -g @dataforxyz/agent-intercom-codex
```

### 2.4 Claude Code

On macOS, select Claude Code in TmuxDeck's **Create Workspace** modal and choose **Install Managed Adapter**. TmuxDeck installs its pinned adapter from the app bundle without contacting npm or changing any global npm package. The same control becomes **Repair Managed Adapter** if health verification fails.

The managed resource is `@dataforxyz/agent-intercom-claude` `0.10.1-tmuxdeck.1`, built from fork commit `afcb3fe3f889c2baab784a15d2aecf7c5676c827` and published with source at <https://github.com/ctliz/agent-intercom-claude/releases/tag/v0.10.1-tmuxdeck.1>. It is based on upstream `v0.10.0` with only the Monitor packaging fix. Its AGPL license and third-party notices are preserved in the installed directory; the exact artifact digest is recorded in `src-tauri/resources/README.md`.

Installation verifies the pinned SHA-256, rejects links, devices, absolute paths and `..`, stages the replacement, validates the Claude plugin → Monitor → MCP/runtime chain, and rolls back if validation or config persistence fails. A healthy existing version remains in place after a failed repair.

Each newly created managed tmux pane or Ghostty native slot explicitly passes `--safe` to Claude and receives a cryptographically random ID such as `tmuxdeck-<random-uuid-shape>` and a readable name such as `<workspace> · Claude 01`. The ID is persisted in the existing pane/slot's tmux metadata for that worker incarnation. Recreating a worker produces a new ID. It is routing metadata and consistency evidence, not an authentication credential.

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
- For a truly interactive Claude TUI to be woken in place, use `cci --tui --name ... --id ...`.

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
cci --name <name> --id <stable-id> --cwd /path/to/project
ccim --name <name> --id <stable-id> --cwd /path/to/project
```

Headless `cci` / `ccim` have no interactive console and cannot type slash commands like `/name`; they can only be named via the `--name` launch argument. Ordinary Claude MCP sessions use `intercom_set_name` to rename.

> The OpenCode/Codex/Claude rename entries in this section depend on their independently installed adapters and may not exist in every same-version registry package. Re-check those host-specific commands and `intercom_set_name` after changing a companion adapter; the Pi maintenance tag does not install or upgrade them.

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

In OpenCode with an existing session, `/intercom`, `/intercom-name`, and `/intercom-id` act on the current session; on home/just-started with no active session, the local patch first auto-creates and enters an empty session, then continues. If you still see `Open a session before using Intercom.`, the new `tui.mjs` is not loaded or the local patch was overwritten — fully restart OpenCode and check the `tui.json` path.

The content copied by `/intercom-id` or Alt+I is cross-harness: it uses the name when unique, and falls back to the stable ID when names are duplicated.

## 6. Agent tools: set name / list / send / ask / reply

`list`, `send`, `ask`, and `reply` use the same protocol concepts across all four adapters. Pi uses the native `/name`; OpenCode, Codex ordinary MCP, and Claude Code ordinary MCP expose `intercom_set_name` only when their independently installed adapter version provides it.

### 6.1 Set the current human-readable name

OpenCode, Codex ordinary MCP, or Claude Code ordinary MCP sessions:

```typescript
intercom_set_name({
  name: "<new-name>"
})
```

It only updates the human-readable name; it does not change the stable session ID returned by `intercom_status({})`. Persistence rules are in [Unified rename capability](#4-unified-rename-capability).

Pi uses:

```text
/name <new-name>
```

### 6.2 View connection status

```typescript
intercom_status({})
```

Confirms the current session ID, broker connection, and pending messages.

### 6.3 List peers in the current workspace or machine

Pi's maintenance adapter defaults to the current workspace: the canonical Git root, or the canonical cwd outside Git.

```typescript
intercom_list({})
```

For an intentional machine-wide view:

```typescript
intercom_list({ scope: "machine" })
```

Both forms return the current session and matching connected sessions with short ID, cwd, model, and live status. Names and ID prefixes resolve within the selected scope. An exact full session ID is checked against the machine roster first and is the explicit cross-workspace route. This filtering is Pi client behavior, not broker authorization or a protocol security boundary; the protocol-v3 broker and other harness adapters may remain machine-global.

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

`to` is the sender's name or stable ID, not a message/thread ID. Do not hand-construct `replyTo`. The Pi maintenance adapter preserves the active ordinary-message batch across provider/tool loops: for one sender it replies to that sender's latest message; for multiple senders, use the exact sender name or full session ID. Failed selection or delivery does not discard the batch context.

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

Temporary isolation example:

```bash
PI_CODING_AGENT_DIR="$HOME/.pi/agent-lab" pi --name agent-a
PI_CODING_AGENT_DIR="$HOME/.pi/agent-lab" \
  OPENCODE_INTERCOM_NAME=agent-b \
  OPENCODE_INTERCOM_SESSION_ID=<stable-id> \
  opencode /path/to/project
```

These two sessions can see each other but not the sessions under the default `~/.pi/agent/intercom`.

## 8. Reload, restart, and upgrades

| Scenario | Action |
|---|---|
| Pi extension install/update | re-run the fixed-tag `pi install` command, then run `/reload` in every open Pi session or restart Pi |
| OpenCode plugin/config update | fully quit and restart OpenCode |
| Codex MCP/package update | restart ordinary Codex sessions; restart `coi` workers reusing their original `--id` |
| Claude MCP/package update | restart ordinary Claude sessions; restart `cci` / `ccim` reusing their original `--id` |
| `PI_CODING_AGENT_DIR` change | reload/restart all four sides |
| Broker auto-restart | clients reconnect automatically; usually no manual action needed |

Upgrade across protocol versions by updating all four sides at once. For this protocol-v3 maintenance update, reload every Pi session and restart all companion adapters so the previous broker exits naturally and the shared protocol-v3 broker restarts. Do not delete `broker.sock`, `broker.owner`, inbox/outbox, or ask state files still in use by active sessions.

Recommended troubleshooting order:

1. Run `intercom_status({})` on the current side.
2. Confirm all sides use the same `PI_CODING_AGENT_DIR`.
3. Confirm the adapter is loaded: Pi extension, OpenCode's two plugins, Codex/Claude MCP or wrappers.
4. On OpenCode home, `/intercom`, `/intercom-name`, or `/intercom-id` should auto-create and enter an empty session; if it still asks to open a session first, fully quit and restart OpenCode and confirm `tui.json` points at the patched `dist/tui.mjs`.
5. For the Codex wrapper, first run `coi --version`; it should print the Codex version and exit. If there's no output or it unexpectedly enters a worker, check that npm's `coi` entry executes `node .../dist/coi.mjs "$@"`, then reinstall or fix the wrapper.
6. Confirm Pi is installed from `git:github.com/ctliz/agent-intercom-pi@v0.10.1-tmuxdeck.1`; re-run that fixed `pi install` command if repair is needed.
7. Run `/reload` in every open Pi session; fully restart Claude, Codex, and OpenCode companion adapters so the protocol-v3 broker can restart cleanly.
8. Run `intercom_list({})` for the current workspace, then `intercom_list({ scope: "machine" })` if the expected peer is in another workspace. Use the exact full ID for intentional cross-workspace routing.
9. If a reply batch is ambiguous, use `intercom_pending({})` and reply with the exact sender name or full session ID; do not pass a message/thread ID as `to`.
10. Only when all clients are confirmed exited should leftover runtime files be considered for cleanup; never delete the socket during active sessions.

## 9. Stable session ID notes

1. **A name is not an identity.** Duplicate names are allowed; sending to a duplicated name fails, so use the stable ID.
2. **Don't reuse an ID concurrently.** One stable ID should be registered by only one process at a time.
3. **Restore the same identity.** Pi resumes the original session; OpenCode reuses `OPENCODE_INTERCOM_SESSION_ID`; Codex/Claude wrappers reuse `--id`.
4. **A harness conversation ID is not the intercom ID.** OpenCode `--session`, Codex thread ID, and Claude `--resume` are each harness-specific conversation identifiers.
5. **Pending asks depend on identity.** If the intercom ID changes after a restart, the reply authorization for old asks does not automatically transfer to the new identity.
6. **Copy exact targets.** Prefer Alt+I or `/intercom-id` to get a pasteable cross-harness contact.
7. **Use IDs for security/high-value flows.** Names are fine for daily collaboration; releases, destructive-operation approvals, and cross-project coordination should use the stable ID directly.

## 10. Local environment verification

Verified on this machine at the time of writing:

```text
Pi package source: git:github.com/ctliz/agent-intercom-pi@v0.10.1-tmuxdeck.1
Pi package version: @dataforxyz/agent-intercom-pi 0.10.1-tmuxdeck.1
Pi checkout:        452b63f11d50dcdbbcf8485eb04d19928bbbfb13
Protocol:           v3
```

Pi settings load the fixed Git tag from `~/.pi/agent/git/github.com/ctliz/agent-intercom-pi`. No global Codex, Claude, or OpenCode adapter package was detected during this verification, so this document does not claim that those adapters were locally upgraded or version-unified. TmuxDeck's optional Managed Claude path is a separate pinned macOS installation described in section 2.4; Standard Claude, Codex, and OpenCode remain independently installed and must be checked in their actual host configuration.

After any adapter change, trust the actual Pi settings/package checkout, `codex mcp list`, `claude mcp list`, OpenCode plugin paths/package metadata, and `intercom_status({})`. Do not infer protocol compatibility from matching package version strings alone.

The Pi maintenance fork changes only Pi's discovery/routing and reply-batch behavior. Other adapters may still expose their existing machine-global discovery behavior. OpenCode-specific local patches, if present on another machine, must be verified independently after an OpenCode package update or reinstall.

## 11. Recommended workflow

```text
1. Launch and set a unique human-readable name
2. intercom_list for the current workspace, intercom_list with machine scope when intentional, or intercom_team for managed coworkers
3. Dispatch tasks with send
4. Only use ask when the next step depends on the answer
5. Reply with reply from the session that received the ask
6. Use stable IDs for high-risk operations, not ambiguous names
7. After updating adapters, reload/restart all four sides together
```

Related docs:

- [Intercom wire protocol reference](./REFERENCE-intercom-protocol.md)
- [v1.12 conversation bridge PRD](./PRD-v1.12-conversation-bridge.md)
- [v1.12 decision log](./DECISIONS-v1.12.md)
