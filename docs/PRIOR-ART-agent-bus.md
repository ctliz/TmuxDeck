# Survey: existing solutions for an agent bus / mobile access

> Bottom line up front: **the thing we wanted to build already exists, and it's more complete than our design.**
> `Agent Intercom` covers four harnesses — pi + Codex + Claude Code + OpenCode — sharing one local broker and one protocol. TmuxDeck should not build a bus; it should become the one piece missing from that family — **the human / phone adapter**.

Survey date: 2026-08-10

---

## 1. Agent Intercom (the decisive discovery)

A cross-harness, same-machine agent messaging system. It originated in `nicobailon/pi-intercom`; `dataforxyz` expanded it into a cross-tool family:

| Harness | Repository |
|---|---|
| Pi | `dataforxyz/agent-intercom-pi` |
| Codex | `dataforxyz/agent-intercom-codex` |
| Claude Code | `dataforxyz/agent-intercom-claude` |
| OpenCode | `dataforxyz/agent-intercom-opencode` |
| Lifecycle management | `dataforxyz/agent-intercom-orchestrator` |

**All four adapters share one broker and one protocol, messaging each other across host boundaries.** This is exactly what we meant by "cross-family communication" — and it's already live.

### It already solves precisely the hardest parts of our PRD

| Problem in the PRD | Agent Intercom's ready-made answer |
|---|---|
| Four-state detection (working / waiting on agent / waiting on human / exited) | broker auto-publishes session status: `idle` / `thinking` / `tool:<name>` |
| Communication groups, global-silence heuristics | gone — one broker, one global registry |
| "Who's waiting on whom" | `broker-asks.json` stores ask/reply edges; `intercom_pending` directly lists unresolved asks, initiators, and how long they've waited |
| Delivery timing (stuffing characters into a thinking pane gets swallowed) | durable inbox + **idle-gated delivery**: queue while busy, inject only when idle; 300ms batching |
| Delivery reliability | receiver ACKs only after atomically writing to inbox; at-least-once semantics, offline replay |
| Message storms | max 256 outstanding outbound messages per session; byte-based connection-level rate limiting |
| Addressing | session name + stable session ID; ambiguous delivery rejected on duplicate names |

`intercom_list` returns: session name, short ID, working directory, model, **live status**. This single item invalidates PRD section 2 (capture-pane polling + hash comparison + silence heuristics) in its entirety.

### Technical details (for writing an adapter)

- Transport: Unix domain socket on macOS/Linux, named pipe on Windows
- Protocol: `pi-intercom` v3, **4-byte length prefix + JSON**
- Runtime directory: `~/.pi/agent/intercom/` (or `$PI_CODING_AGENT_DIR/intercom/`)
  - `broker.sock` / `broker.pid` / `broker.owner` / `config.json`
  - `inbox/<hash>.json`, `outbox/<hash>.json`, `broker-asks.json`
- The broker starts itself on first connection and exits 5 seconds after the last session disconnects. No daemon to manage
- Tool surface: `intercom_send` (fire and forget), `intercom_ask` (blocking wait 30s, turns async on timeout), `intercom_reply`, `intercom_list`, `intercom_pending`, `intercom_status`, `intercom_team`

> The README has a crucial line: the broker's runtime instance ID mechanism exists to "prevent reconnection conflicts when the desktop Pi and a **mobile RPC host** open the same transcript simultaneously". **They already anticipated a mobile host joining the family, but no such adapter exists in it.**

### License note

`agent-intercom-pi` is **AGPL-3.0-or-later** (earlier MIT versions remain usable under their original terms). Implementing a client yourself against the wire protocol is not a derivative work, but **don't copy its source**. Write the Rust adapter against the protocol.

---

## 2. AWS Labs CAO (different shape, borrowable ideas)

`awslabs/cli-agent-orchestrator`, Apache-2.0. Supports Claude Code, Codex, Gemini, Kiro, Kimi, Copilot, OpenCode, Q CLI — **everything except pi**.

- Each agent runs in its own tmux session, exposing `handoff` / `assign` / `send_message` via MCP
- The server routes by `CAO_TERMINAL_ID`, tracking `IDLE / PROCESSING / COMPLETED / ERROR`
- `cao session send <name> "msg"` is a plain shell command — confirming that "the shell is the least common denominator of all agents"
- Ships a Web UI (`localhost:9889`)
- **Plugins can forward inter-agent messages to Discord / Slack / Telegram** — the notification path we envisioned
- Security posture matches our conclusion: localhost only + Host-header validation against DNS rebinding

**Difference from how we work:** CAO is a hierarchical model where a supervisor spawns workers; we're a peer model of "manually open a bunch of agents, then have them talk to each other". **Agent Intercom is the right shape.** But CAO's IM-forwarding plugin and state-machine naming are worth stealing.

---

## 3. Existing mobile solutions

| Project | Coverage | Notes |
|---|---|---|
| **Happy** (`slopus/happy`) | Claude Code, Codex | **Open source**, mobile + Web client, end-to-end encrypted, **push notifications for permission requests and task completion**, history still viewable while the terminal is offline. The most worth studying |
| Omnara | Claude Code, Codex | Closed-source commercial, on App Store / Play. Known shortcoming: **no system notification** when an agent needs input — you have to open the app yourself |
| VibeTunnel | generic terminal | Browser access to a Mac terminal. Replicates the terminal experience, but **no push notifications**, and no UI for answering questions / viewing diffs |

None of the three supports pi, and none offers a bus view across agent families.

---

## 4. Conclusion and recommendations

### Don't build

- ❌ **Cross-tool message bus** — Agent Intercom already exists, and its four adapters cover exactly our toolset
- ❌ **Four-state detection / silence heuristics / communication groups** — the broker's session status and ask edges are facts; nothing to guess
- ❌ **send-keys delivery queue** — intercom's durable inbox + idle-gated delivery is strictly better
- ❌ **Full mobile terminal** — Happy / VibeTunnel already exist

### Worth doing (the vacuum in the family)

> **TmuxDeck = the human / phone adapter for Agent Intercom.**

Concretely: TmuxDeck joins as the fifth adapter, connecting to `broker.sock` and registering as a session named `me`.

- Agent needs a human: `intercom_ask({ to: "me", message: "..." })` → TmuxDeck receives it → push to the phone
- Phone replies → TmuxDeck goes through `intercom_reply` → the broker handles idle-gated delivery and ACK
- Desktop dashboard: read `intercom_list` and get the real status directly, no more capture-pane polling
- `intercom_pending` is naturally "the inbox of people waiting on you" — **the inbox UI envisioned in the PRD has a ready-made data source**

**Notifications are no longer detected; they're a message addressed to `me`.** Same conclusion as before — we just don't have to build the bus.

### Kept fallback

Agents without an intercom adapter (Aider, Gemini CLI, plain shell) still need `send_keys` + silence heuristics. But it's demoted from the main path to a long-tail fallback and can be rough.

### Three things to verify before writing any code

1. Does `~/.pi/agent/intercom/broker.sock` exist on your machine, and is the protocol version v3?
2. Do the non-pi adapters (Claude Code's `cci` / Codex's `coi` wrappers) require changing how agents are launched today?
3. Connect with a minimal Rust/Node client to the broker and get a `list` through plus one `send` — **only after that works, touch a single line of TmuxDeck code**

---

## 5. Disposition of the existing PRD

| Doc | Disposition |
|---|---|
| `PRD-v1.12-mobile-server.md` | largely obsolete. Section 2 (state detection) deleted in its entirety; section 3 keeps the message and reply UI design; transport layer becomes the intercom broker |
| "build-our-own-bus" idea | reverted, replaced by implementing an intercom adapter |

---

## Sources

- [dataforxyz/agent-intercom-pi](https://github.com/dataforxyz/agent-intercom-pi)
- [nicobailon/pi-intercom](https://github.com/nicobailon/pi-intercom)
- [nicobailon/pi-messenger](https://github.com/nicobailon/pi-messenger)
- [earendil-works/pi](https://github.com/earendil-works/pi)
- [awslabs/cli-agent-orchestrator](https://github.com/awslabs/cli-agent-orchestrator)
- [slopus/happy](https://github.com/slopus/happy) · [happy.engineering](https://happy.engineering/)
- [Omnara (App Store)](https://apps.apple.com/us/app/omnara-claude-codex-mobile/id6748426727)
- [absmartly/Tmux-Orchestrator](https://github.com/absmartly/Tmux-Orchestrator)
