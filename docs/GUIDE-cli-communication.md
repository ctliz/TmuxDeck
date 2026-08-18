# TmuxDeck Multi-CLI Communication Guide

> Applies to TmuxDeck v1.14.3 and Agent Intercom Protocol v4.
>
> This guide explains how Pi, Claude Code, Codex, OpenCode, Grok, and Agy join the local broker, expose identity, and troubleshoot installation, MCP, terminal, and TUI issues.

## 1. Communication architecture

```text
Pi ───────────┐
Claude Code ──┤
Codex ────────┤
OpenCode ─────┼── local Agent Intercom broker ── Unix socket / named pipe
Grok ─────────┤
Agy ──────────┘
                     ▲
                     │
                 TmuxDeck
```

- TmuxDeck assigns a stable worker session ID to every pane or native slot and records the team manifest.
- The first successfully registered adapter starts the local broker; other CLIs connect to the same broker.
- The v4 broker scopes discovery and short-ID resolution to the active workspace.
- Names and short IDs are convenient inside one workspace. Cross-scope routing requires the exact full session ID.
- This is local same-OS-user communication, not a public messaging service or a cryptographic security boundary.
- Only installed, loaded, and successfully registered clients appear in `intercom_list`.

Default runtime directory:

```text
~/.pi/agent/intercom/
```

It normally contains the broker socket, PID/owner files, inbox/outbox data, request records, and configuration.

## 2. TmuxDeck automatic team join

When an Agent is selected in **Create Workspace**, TmuxDeck creates a team manifest and injects, per pane or slot:

- `AGENT_INTERCOM_TEAM_MANIFEST`: absolute manifest path;
- `AGENT_INTERCOM_SESSION_ID`: stable worker identity;
- `AGENT_INTERCOM_SESSION_NAME`: human-readable name;
- `AGENT_INTERCOM_ROLE`: `manager` for the lead and `worker` for other panes;
- `AGENT_INTERCOM_SCOPE_ID`: workspace scope;
- `AGENT_INTERCOM_MANAGER_TARGET` / `AGENT_INTERCOM_MANAGER_SESSION_ID`: the worker's lead target;
- harness-specific identity variables such as `CODEX_INTERCOM_*` and `CLAUDE_INTERCOM_*`.

Users do not need to run a manual join command. The command, environment, tmux metadata, and manifest must belong to the same workspace creation transaction.

### 2.1 Terminal capabilities

TmuxDeck panes use:

```text
TERM=tmux-256color
COLORTERM=truecolor
focus-events=on
extended-keys=on
terminal-overrides=*:RGB
```

Do not blindly change `TERM` to `xterm-ghostty`. tmux owns the pane terminal type. If a CLI behaves differently from a direct Ghostty launch, collect `TERM`, `TERM_PROGRAM`, dimensions, `stty -a`, and `capture-pane -p -e` together.

### 2.2 Permission bypass modes

TmuxDeck enables bypass mode by default for supported Agents and exposes a creation-time toggle to disable it:

| CLI | Default panel option | Meaning |
|---|---|---|
| Claude | `--dangerously-skip-permissions` | Skips permission prompts |
| Codex | `--dangerously-bypass-approvals-and-sandbox` | Bypasses approvals and the sandbox; highest risk |
| OpenCode | `--auto` | Automatic execution mode |
| Grok | `--permission-mode bypassPermissions` | Bypasses permission prompts |
| AGY | `--dangerously-skip-permissions` | Skips permission prompts |
| Pi / Aider / shell | No generic bypass flag | No fabricated option is injected |

These options apply only to TmuxDeck-generated default commands. They do not alter custom commands or CLIs launched directly in Ghostty.

## 3. Per-CLI communication

### 3.1 Pi

Pi loads Agent Intercom as an extension. Install only the official npm package:

```text
pi install npm:@ctliz/pi-intercom@0.12.1
```

Do not also install `git:github.com/ctliz/agent-intercom-pi`. Two copies register the same tools and Pi exits on launch.

Properties:

- The extension runs inside the Pi process;
- it uses the Core 0.2.0 v4 protocol and team manifest;
- Pi provides `/intercom`, `/name`, `/intercom-join`, and `/intercom-status`;
- after updating the extension, run this in every open Pi session:

```text
/reload
```

Without a reload or restart, an old extension can remain connected to an old broker instance.

### 3.2 Claude Code

On macOS, TmuxDeck managed Claude uses a bundled, digest-pinned Claude adapter containing:

- the Claude plugin manifest;
- monitor configuration;
- the MCP server;
- `cci` and runtime files.

A managed pane typically starts as:

```text
cci --tui --safe --id <session-id> --name <workspace> · Claude 01
```

The communication path is:

```text
Claude Code
  ├─ plugin / MCP server
  ├─ inbox monitor
  └─ local broker
```

Installation and repair verify the resource SHA-256, managed marker, plugin chain, JavaScript runtime, and monitor smoke test. Once the state is `Healthy`, the Claude chip should not continue showing an install or repair prompt.

Standard and managed Claude are separate modes:

- **Use Standard Claude** uses the detected system `claude` binary;
- **Use Managed Claude** uses TmuxDeck's app-private managed root;
- custom Agent commands are passed through unchanged.

### 3.3 Codex

Ordinary Codex communication uses an MCP server:

```text
Codex CLI
  └─ MCP client
      └─ node <managed>/dist/codex-server.mjs
          └─ local broker
```

The managed Codex configuration must invoke the bundled server directly:

```toml
[mcp_servers.codex-intercom]
command = "node"
args = ["<managed-root>/0.12.0-connect.1/dist/codex-server.mjs"]
```

Do not configure `codex-launcher.mjs` as the MCP server. The launcher is a CLI wrapper, not an MCP server; it may try to execute an incorrect `/usr/local/bin/codex` path.

A TmuxDeck Codex pane normally starts as:

```text
codex --dangerously-bypass-approvals-and-sandbox
```

A healthy MCP path should pass a JSON-RPC `initialize` handshake. If the Codex TUI appears to lack an input prompt, first distinguish:

1. an MCP handshake failure;
2. a missing interactive/bypass argument;
3. a prompt that is already present in ANSI capture;
4. a terminal identity difference between `TERM=tmux-256color` and direct Ghostty `TERM=xterm-ghostty`.

### 3.4 Grok and Agy

Grok and Agy use external, manually installed Intercom plugins through the Claude MCP bridge; TmuxDeck does not bundle, install, or configure them. Before installing, confirm that `claude-intercom-mcp` is on `PATH`:

```bash
command -v claude-intercom-mcp
```

Then install the plugin supplied by its provider:

```bash
# Grok
grok plugin install <agent-intercom-grok plugin path> --trust

# Agy
agy plugin install <agent-intercom-agy plugin path>
```

Grok's MCP child does not inherit arbitrary pane environment variables. A multi-pane Auto-Team needs an isolated per-pane MCP config with concrete identity and scope values; without it Grok uses a live-only fallback identity. AGY likewise requires its host to propagate each pane identity to its MCP child. TmuxDeck cannot wake either plugin; call `intercom_pending` proactively.

### 3.5 OpenCode

OpenCode has two plugin surfaces:

```text
opencode.json ── dist/plugin.mjs (server communication)
tui.json       ── dist/tui.mjs (TUI commands and shortcuts)
```

The communication path is:

```text
OpenCode
  ├─ plugin.mjs
  ├─ tui.mjs
  └─ local broker
```

Do not register `tui.mjs` in `opencode.json`, or `plugin.mjs` in `tui.json`. Fully quit and restart OpenCode after package or configuration changes.

Managed OpenCode uses the bundled adapter and SDK dependency closure. Installation must find Node/npm even when Tauri is launched from a GUI; TmuxDeck supplies an augmented PATH to staging subprocesses while preserving an explicitly supplied PATH.

## 4. Common communication operations

The UI command names differ slightly, but the protocol operations are shared. Grok and AGY expose their plugin-provided Intercom interface and cannot be woken, so call `intercom_pending` proactively. Grok needs a materialized per-pane MCP config before it can join a TmuxDeck Auto-Team:

| Operation | Pi | Claude | Codex | OpenCode |
|---|---|---|---|---|
| List peers | `intercom_list` / `/intercom` | MCP tool or `/claude-intercom:intercom` | MCP tool | `/intercom` / MCP tool |
| Send | `intercom_send` | `intercom_send` | `intercom_send` | `intercom_send` |
| Ask | `intercom_ask` | `intercom_ask` | `intercom_ask` | `intercom_ask` |
| Reply | `intercom_reply` | `intercom_reply` | `intercom_reply` | `intercom_reply` |
| Identify self | `intercom_whoami` | `intercom_whoami` | `intercom_whoami` | `intercom_whoami` |
| Rename | Pi `/name` | launch environment/adapter name | launch environment/adapter name | `/intercom-name` |

Recommended flow:

1. Call `intercom_list` first;
2. use a name only when it is unique;
3. use the exact full session ID for duplicate names or cross-scope routing;
4. wait for an acknowledgement or reply instead of retrying blindly.

## 5. Troubleshooting

### 5.1 A CLI is not found from TmuxDeck

Collect:

```bash
env | egrep '^(PATH|TERM|COLORTERM|TERM_PROGRAM|LANG)='
which claude
which codex
which opencode
which grok
which agy
which pi
which claude-intercom-mcp
which npm
which node
```

A GUI-launched Tauri process may not inherit the shell's PATH. TmuxDeck's adapter probe scans common Homebrew, NVM, Cargo, local, and OpenCode locations; custom locations must be added to PATH or used through a custom command.

### 5.2 `ERR_PLAN_INVALID`

Inspect the plan:

- `planId` must be `plan_` followed by 32 lowercase hexadecimal characters;
- the fingerprint must be 64 lowercase hexadecimal characters;
- `canApply` must be true before applying;
- `items` must not contain `unavailable` or `migration-required` entries.

A missing GUI PATH can make an installed Codex appear unavailable. Restart the latest Tauri process and generate a fresh plan; do not reuse an old plan.

### 5.3 The UI still says Install or Repair after installation

Inspect:

```text
~/Library/Application Support/tmuxdeck/managed/<harness>/<version>/tmuxdeck-managed.json
```

For the npm Claude layout, the plugin manifest is at:

```text
<root>/node_modules/@ctliz/agent-intercom-claude/.claude-plugin/plugin.json
```

Do not check only `<root>/.claude-plugin`. TmuxDeck invalidates health and environment caches after a successful install; fully quit and restart Tauri if an old UI process remains.

### 5.4 MCP handshake failure

Verify that the configuration starts the server directly:

```text
node <managed>/dist/<harness>-server.mjs
```

Do not use a CLI launcher as an MCP server. For Codex, send a JSON-RPC `initialize` request and inspect stderr, server path, Node version, and managed-root integrity.

### 5.5 The TUI input box is not visible

Collect, in one capture:

```bash
tmux list-panes -a -F 'session=#{session_name} pane=#{pane_id} tty=#{pane_tty} pid=#{pane_pid} #{pane_width}x#{pane_height} cmd=#{pane_current_command}'
ps eww -p <cli-pid>
stty -f <pane-tty> -a
tmux show-options -s -t <session>
tmux capture-pane -p -e -J -t <pane> -S -60
```

If `capture-pane -p -e` already contains `›`, `>`, or another prompt ANSI sequence, the input prompt has been rendered. The remaining issue is likely color, terminal capabilities, or visual layout rather than disabled stdin.

## 6. Security and provenance

- Codex bypass disables both approvals and its sandbox; use it only in trusted project directories.
- OpenCode `--auto`, Grok bypass, AGY bypass, and Claude bypass also reduce human confirmation; disable bypass when safer prompting is required.
- Custom commands are preserved; TmuxDeck does not guess dangerous flags for Pi, Aider, or shell.
- Grok and AGY plugins are external/manual only. They require `claude-intercom-mcp` on `PATH`, receive their per-pane identity from TmuxDeck, and must poll `intercom_pending` rather than rely on a wake-up.
- Managed adapters use bundled artifacts, fixed versions, and SHA-256 verification. Do not replace release resources with unauthorized registry or network packages.
- Core, Pi, Claude, Codex, OpenCode, and the Grok/Agy bridge plugins must use compatible protocol-v4 versions. Mixing an older Core or adapter can create broker islands that cannot see one another.
- Session IDs, team manifests, and broker data are local runtime routing data, not public credentials.
