# pi-intercom wire protocol reference

> This document is updated against Agent Intercom protocol v4 (`types.ts`, `broker.ts`, and `broker/framing.ts` in the `ctliz` ecosystem with `@dataforxyz` provenance), covering Pi `v0.12.0-connect.1` and Managed Claude `0.13.0-connect.1`. **Written down so we never have to re-derive it.**
>
> Implementation lives in `src-tauri/src/intercom.rs`; the verification script is `scripts/intercom-probe.mjs`.

---

## Transport

| Platform | Transport |
|---|---|
| macOS / Linux | Unix domain socket |
| Windows | named pipe (TmuxDeck degrades gracefully to send-keys) |

Socket path:

```
$PI_CODING_AGENT_DIR/intercom/broker.sock   (when that env var is set)
~/.pi/agent/intercom/broker.sock            (default)
```

**The broker's lifecycle is not ours to manage**: it is launched automatically by the first intercom session and exits on its own 5 seconds after the last session disconnects. Callers should degrade gracefully if the socket is absent rather than attempting to force broker creation.

---

## Framing

```
┌───────────────────────────┬──────────────────────────────────────────────┐
│ 4-byte big-endian length  │ UTF-8 JSON (length applies to this segment)  │
└───────────────────────────┴──────────────────────────────────────────────┘
```

Single-frame cap is **1 MiB**; exceeding it triggers an immediate connection drop. The reader must handle TCP/UDS packet splitting and coalescing.

---

## Client → broker

| `type` | Key fields | Notes |
|---|---|---|
| `register` | `protocol: "pi-intercom"`, `version: 4`, `scopeId?`, `session` (see below), `sessionId?`, `stateId?` | First frame after connecting; `scopeId` is optional top-level scope; version mismatch causes disconnection |
| `unregister` | `preserveAsks?` | Graceful disconnection |
| `list` | `requestId` | Request active session list; broker enforces workspace scope by default |
| `send` | `to`, `message` | `to` resolves by name/prefix in-scope, or by exact full session ID cross-scope |
| `message_received` | `deliveryId` | Receiver confirms delivery after durable enqueue |
| `message_rejected` | `deliveryId`, `code`, `reason` | Receiver rejects an invalid delivery |
| `presence` | `status?`, `name?`, `model?` … | Update session status |
| `cancel_message` / `cancel_ask` | `messageId` | Recall a message |
| `extension_publish` / `extension_state_commit` | `namespace` … | Extension bus |

### The `session` field of `register`

```jsonc
{
  "name": "me",             // Display/contact name within scope
  "cwd": "/path/to/project", // Working directory
  "model": "human",         // "human" to indicate a person rather than an agent
  "pid": 12345,             // Key for pane/process association
  "startedAt": 1754870400000,
  "lastActivity": 1754870400000,
  "status": "idle"
}
```

> **Scope location & isolation boundary:** `scopeId` is strictly a top-level field on `register` and never belongs inside `session`. `scopeId` provides same-OS-user workspace isolation and routing resolution, not a cryptographic security principal. The trust perimeter is the local OS user account.
>
> **Scope encapsulation:** `scopeId` is purely broker routing metadata. It **never** enters `SessionInfo`, `list` / `sessions` response payloads, lifecycle event frames (`session_joined`, `session_left`, `presence_update`), or frontend/mobile models.

---

## Broker → client

| `type` | Key fields | Notes |
|---|---|---|
| `registered` | `sessionId`, `protocol`, `version` | Registration succeeded |
| `sessions` | `requestId`, `sessions[]` | Scoped list response (`SessionInfo[]`, no scopeId exposed) |
| `message` | `deliveryId`, `from`, `message` | Inbound message; must ACK with `message_received` |
| `presence_update` | `session` | Session status change |
| `session_joined` / `session_left` | `session` / `sessionId` | Online/offline lifecycle events |
| `delivery_accepted` | `messageId`, `deliveryId` | Broker accepted message for delivery |
| `delivered` | `messageId`, `deliveryId` | Target acknowledged durable enqueue |
| `delivery_failed` | `messageId`, `code`, `reason`, `retryable` | Delivery failed |
| `error` | `error` | Broker error |

**Tolerant parsing:** Inbound frame parsing ignores unknown `type` fields without failing deserialization of the connection.

---

## Workspace scoping and routing rules (v4)

1. **Broker-enforced scoping:** `intercom_list` returns only peers belonging to the caller's registered `scopeId`.
2. **Name and prefix resolution:** Short names and prefix-based session matching are confined to the caller's workspace scope.
3. **Cross-scope routing:** A sender communicating across scopes must provide the **exact full session ID**.
4. **Zero raw scope exposure for frontend and mobile:** Desktop and mobile control surfaces maintain zero raw scope exposure (零原值暴露); the backend manages an independent scoped human client (`me`) per workspace and aggregates conversations into the unified `ConversationRegistry`.
5. **Fail-closed legacy workspaces:** Pre-v4 workspaces without scope metadata fail closed on add/rename operations and require recreation.
6. **Coordinated upgrade for installed adapters only:** When transitioning protocol versions, only currently installed and active adapters must be upgraded together. Reload `/reload` Pi sessions and restart active companion adapters.
7. **Orchestrator:** Orchestrator is an optional Linux/systemd lifecycle product, outside the Broker compatibility set; omitted on macOS.
8. **Same-OS-user isolation:** Scope is an operational routing boundary, not a cryptographic security principal.

---

## SessionInfo

```jsonc
{
  "id": "20d43841…",        // Stable session ID; unambiguous addressing target
  "name": "planner",        // Human-readable name (unique per workspace scope)
  "cwd": "/projects/api",
  "model": "claude-sonnet-4",
  "pid": 12345,
  "startedAt": 1754870400000,
  "lastActivity": 1754870400000,
  "status": "thinking",
  "contextPct": 43
}
```

### Status values

| Value | Meaning |
|---|---|
| `idle` | Idle, ready for input |
| `thinking` | Generating output |
| `tool:<name>` | Executing a tool |
| absent / other | Unknown |

---

## Message structure

```jsonc
{
  "id": "m-1",
  "timestamp": 1754870400000,
  "replyTo": "m-0",
  "expectsReply": true,     // Priority signal indicating a blocking question
  "content": {
    "text": "Need your confirmation",
    "attachments": [
      { "type": "snippet", "name": "auth.ts", "language": "typescript", "content": "…" }
    ]
  }
}
```

---

## Upstream and provenance

- [nicobailon/pi-intercom](https://github.com/nicobailon/pi-intercom) (MIT, original pi-only implementation)
- `@dataforxyz/agent-intercom-*` (AGPL, cross-harness protocol foundation)
- `@ctliz/agent-intercom-core@0.2.0` (core protocol implementation & team-manifest kernel)
- `@ctliz/agent-intercom-pi` / [ctliz/agent-intercom-pi](https://github.com/ctliz/agent-intercom-pi) (`v0.12.0-connect.1`, protocol v4)
- `@ctliz/agent-intercom-claude` / [ctliz/agent-intercom-claude](https://github.com/ctliz/agent-intercom-claude) (`0.13.0-connect.1`, Managed Claude with `--tui --safe`)
- `@ctliz/agent-intercom-codex` (`0.12.0-connect.1`)
- `@ctliz/agent-intercom-opencode` (`0.12.0-connect.1`)
- `@ctliz/agent-intercom-orchestrator` (`0.12.0-connect.1`, optional Linux/systemd)
