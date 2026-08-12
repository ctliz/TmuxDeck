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

All four sides should use the same generation of the `@dataforxyz/agent-intercom-*` adapters, and must not be mixed with the older pi-only `nicobailon/pi-intercom`. Mixing adapter or protocol versions across old and new can form broker "islands" that cannot see each other.

After installing or upgrading any adapter, have **all still-open sessions** do a reload/restart.

### 2.1 Pi

Install:

```bash
pi install npm:@dataforxyz/agent-intercom-pi
```

Update:

```bash
pi update --extension npm:@dataforxyz/agent-intercom-pi
```

After install or update, run this in every open Pi session:

```text
/reload
```

Alternatively, quit and restart Pi.

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
- The locally patched `tui.mjs` supports using `/intercom`, `/intercom-name`, or `/intercom-id` directly when OpenCode has just started and has no active session (i.e. the home page): the plugin automatically creates an empty session, enters it, then continues the original operation. When an active session already exists, the current session is always reused and none is created additionally.

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

Install the global adapter:

```bash
npm install -g @dataforxyz/agent-intercom-claude
```

Register the globally available MCP server for ordinary Claude Code sessions (Claude's default scope is `local`, so user scope is used explicitly here):

```bash
claude mcp add -s user claude-intercom -- claude-intercom-mcp
```

Verify:

```bash
claude mcp list
```

The package also provides:

- `claude-intercom-mcp`: tools inside ordinary Claude Code sessions.
- `cci`: an ordinary wakeable Claude worker; `cci --tui` starts the interactive Claude TUI with Intercom identity.
- `ccim`: a minimal wakeable worker, equivalent to `cci --minimal`.
- `claude-intercom-worker`: advanced use for publishing multiple background workers from one process.

When Claude Code is selected from the TmuxDeck panel, TmuxDeck first runs the discovered `cci --help` and selects `cci --tui` only when the executable succeeds and advertises `--tui`, `--id`, and `--name`. Each legacy tmux pane receives an identity such as `tmuxdeck-<workspace>-pane-01`; each Ghostty native slot receives `tmuxdeck-<workspace>-slot-01`. The displayed name is `<workspace> · Claude 01`. These values are stable when the same workspace/pane or slot is recreated. If `cci` is missing, non-executable, or incompatible, TmuxDeck silently uses the independently detected ordinary `claude` binary instead. The panel entry reads `Claude Code · Intercom (cci)` or `Claude Code · Standard`, so the selected mode is visible before creation. Custom commands are passed through unchanged.

To inspect an active `cci` identity, use `/claude-intercom:intercom-id` or Alt+I. When starting outside TmuxDeck, specify it yourself:

```bash
cci --tui --id my-stable-claude-id --name "Readable Claude name"
```

After updating, restart ordinary Claude Code sessions and all `cci` / `ccim` workers:

```bash
npm update -g @dataforxyz/agent-intercom-claude
```

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

> The unified rename entry in this section comes from a local `0.10.0` package patch and does not yet match every same-version install on the npm registry. Running `npm update` or reinstalling may overwrite the patch; until upstream releases it, re-check after an upgrade that the slash commands and the `intercom_set_name` tool still exist.

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

`list`, `send`, `ask`, and `reply` mean the same thing across all four adapters. The runtime rename tool is currently provided by the locally patched OpenCode, Codex ordinary MCP, and Claude Code ordinary MCP sessions; Pi uses the native `/name`.

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

### 6.3 List all peers

```typescript
intercom_list({})
```

Returns the current session and all connected Pi, OpenCode, Codex, and Claude Code sessions, including short ID, cwd, model, and live status.

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

`to` is the sender's name or stable ID, not a message/thread ID. Do not hand-construct `replyTo`.

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
| Pi extension install/update | run `/reload` in every open Pi session, or restart Pi |
| OpenCode plugin/config update | fully quit and restart OpenCode |
| Codex MCP/package update | restart ordinary Codex sessions; restart `coi` workers reusing their original `--id` |
| Claude MCP/package update | restart ordinary Claude sessions; restart `cci` / `ccim` reusing their original `--id` |
| `PI_CODING_AGENT_DIR` change | reload/restart all four sides |
| Broker auto-restart | clients reconnect automatically; usually no manual action needed |

Upgrade across protocol versions by updating all four sides at once. Do not delete `broker.sock`, `broker.owner`, inbox/outbox, or ask state files still in use by active sessions.

Recommended troubleshooting order:

1. Run `intercom_status({})` on the current side.
2. Confirm all sides use the same `PI_CODING_AGENT_DIR`.
3. Confirm the adapter is loaded: Pi extension, OpenCode's two plugins, Codex/Claude MCP or wrappers.
4. On OpenCode home, `/intercom`, `/intercom-name`, or `/intercom-id` should auto-create and enter an empty session; if it still asks to open a session first, fully quit and restart OpenCode and confirm `tui.json` points at the patched `dist/tui.mjs`.
5. For the Codex wrapper, first run `coi --version`; it should print the Codex version and exit. If there's no output or it unexpectedly enters a worker, check that npm's `coi` entry executes `node .../dist/coi.mjs "$@"`, then reinstall or fix the wrapper.
6. Run `/reload` on Pi; fully restart the other harnesses.
7. Run `intercom_list({})` again.
8. Only when all clients are confirmed exited should leftover runtime files be considered for cleanup; never delete the socket during active sessions.

## 9. Stable session ID notes

1. **A name is not an identity.** Duplicate names are allowed; sending to a duplicated name fails, so use the stable ID.
2. **Don't reuse an ID concurrently.** One stable ID should be registered by only one process at a time.
3. **Restore the same identity.** Pi resumes the original session; OpenCode reuses `OPENCODE_INTERCOM_SESSION_ID`; Codex/Claude wrappers reuse `--id`.
4. **A harness conversation ID is not the intercom ID.** OpenCode `--session`, Codex thread ID, and Claude `--resume` are each harness-specific conversation identifiers.
5. **Pending asks depend on identity.** If the intercom ID changes after a restart, the reply authorization for old asks does not automatically transfer to the new identity.
6. **Copy exact targets.** Prefer Alt+I or `/intercom-id` to get a pasteable cross-harness contact.
7. **Use IDs for security/high-value flows.** Names are fine for daily collaboration; releases, destructive-operation approvals, and cross-project coordination should use the stable ID directly.

## 10. Local environment verification

At the time of writing, the four adapters were installed locally at the same version:

```text
@dataforxyz/agent-intercom-pi       0.10.0
@dataforxyz/agent-intercom-opencode 0.10.0
@dataforxyz/agent-intercom-codex    0.10.0
@dataforxyz/agent-intercom-claude   0.10.0
```

Confirmed base configuration:

- Pi settings have loaded `npm:@dataforxyz/agent-intercom-pi`.
- OpenCode `opencode.json` has loaded `dist/plugin.mjs`.
- OpenCode `tui.json` has loaded `dist/tui.mjs`.
- `codex-intercom` is enabled in the Codex MCP.
- `claude-intercom` is connected in the Claude MCP.

After a version upgrade, trust the actual `package.json`, `codex mcp list`, `claude mcp list`, and `intercom_status({})`, and do not rely on this document's version numbers long-term.

OpenCode's home auto-create-empty-session capability is likewise part of the local `0.10.0` patch for now. `npm update @dataforxyz/agent-intercom-opencode` or a reinstall may overwrite it; until upstream releases it, re-check after an upgrade and fully restart OpenCode so the correct `tui.mjs` takes effect.

## 11. Recommended workflow

```text
1. Launch and set a unique human-readable name
2. intercom_list or intercom_team to confirm the target
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
