# pi-intercom wire protocol reference

> This document was originally reconstructed from `nicobailon/pi-intercom`, and is now updated against Agent Intercom protocol v3's `types.ts`, `broker.ts`, and `broker/framing.ts`, including the Pi maintenance release `v0.10.1-tmuxdeck.1`. **Written down so we never have to re-derive it.**
>
> Implementation lives in `src-tauri/src/intercom.rs`; the verification script is `scripts/intercom-probe.mjs`.

---

## Transport

| Platform | Transport |
|---|---|
| macOS / Linux | Unix domain socket |
| Windows | named pipe (TmuxDeck does not implement it yet) |

Socket path:

```
$PI_CODING_AGENT_DIR/intercom/broker.sock   (when that env var is set)
~/.pi/agent/intercom/broker.sock            (default)
```

**The broker's lifecycle is not ours to manage**: it is launched automatically by the first intercom session and exits on its own 5 seconds after the last session disconnects. So "socket doesn't exist" is the norm, not an error — callers should degrade to the `send-keys` channel rather than trying to start the broker.

---

## Framing

```
┌────────────────┬─────────────────────────┐
│ 4-byte big-endian length │ UTF-8 JSON (length applies to this segment) │
└────────────────┴─────────────────────────┘
```

Single-frame cap is **1 MiB**; the side exceeding it should error and disconnect. Note that TCP/UDS split and coalesce packets, so the reader must reassemble — both `intercom-probe.mjs` and `intercom.rs` implement this.

---

## Client → broker

| `type` | Key fields | Notes |
|---|---|---|
| `register` | `protocol: "pi-intercom"`, `version: 3`, `session` (see below), `sessionId?`, `stateId?` | the first thing after connecting; a version mismatch disconnects |
| `unregister` | `preserveAsks?` | graceful exit |
| `list` | `requestId` | request the session list; returned asynchronously via `sessions` |
| `send` | `to`, `message` | `to` can be a session name or session ID |
| `message_received` | `deliveryId` | the receiver confirms this delivery after durable enqueue |
| `message_rejected` | `deliveryId`, `code`, `reason` | receiver rejects a conflicting delivery |
| `presence` | `status?`, `name?`, `model?` … | update own status |
| `cancel_message` / `cancel_ask` | `messageId` | recall |
| `extension_publish` / `extension_state_commit` | `namespace` … | extension bus; unused by TmuxDeck |

### The `session` field of `register`

```jsonc
{
  "name": "me",          // other sessions address by this
  "cwd": "/path",        // display metadata
  "model": "human",      // "human" so others can see at a glance this is a person, not an agent
  "pid": 12345,          // key for pane association: walk up the parent chain to match pane_pid
  "startedAt": 1754870400000,
  "lastActivity": 1754870400000,
  "status": "idle"
}
```

> `cwd` / `model` / `pid` / `status` are all **display metadata, not authentication**. The broker's trust boundary is "same OS user", not a cryptographic principal.

---

## Broker → client

| `type` | Key fields | Notes |
|---|---|---|
| `registered` | `sessionId`, `protocol`, `version` | registration succeeded; you get your own session ID |
| `sessions` | `requestId`, `sessions[]` | the response to `list` |
| `message` | `deliveryId`, `from`, `message` | a message arrived; you must ACK by `deliveryId` after handling |
| `presence_update` | `session` | some session's status changed |
| `session_joined` / `session_left` | `session` / `sessionId` | went online / offline |
| `delivery_accepted` | `messageId`, `deliveryId` | broker accepted it and is awaiting receiver confirmation |
| `delivered` | `messageId`, `deliveryId` | receiver confirmed durable receipt |
| `delivery_failed` | `messageId`, `code`, `reason`, `retryable` | delivery failed |
| `error` | `error` | broker error |
| `message_control` / `extension_*` | — | not consumed by TmuxDeck |

**Inbound parsing must tolerate unknown `type`**: the upstream protocol is evolving (the cross-harness branch is at v3, with several new frame types). `intercom.rs` therefore dispatches manually rather than using serde's internally tagged enums — unknown types can simply be ignored without failing deserialization of the whole connection.

---

## SessionInfo

```jsonc
{
  "id": "20d43841…",     // stable session ID, the trustworthy addressing key
  "name": "planner",     // duplicates allowed; sending to a duplicate name fails, switch to id
  "cwd": "/projects/api",
  "model": "claude-sonnet-4",
  "pid": 12345,
  "startedAt": 1754870400000,
  "lastActivity": 1754870400000,
  "status": "thinking",  // see below
  "contextPct": 43       // context usage percent; may be absent
}
```

### status — the factual source for four-state detection

| Value | Meaning |
|---|---|
| `idle` | idle, can receive input |
| `thinking` | model is generating |
| `tool:<name>` | running some tool |
| absent / other | unknown (don't guess) |

Auto-reported by each session at pi lifecycle events.

> This single item eliminates all need for the "poll capture-pane + hash-compare content + silence heuristic" machinery. Do not re-implement that.

---

## Message

```jsonc
{
  "id": "m-1",
  "timestamp": 1754870400000,
  "replyTo": "m-0",       // reply to some message; the receiver matches the corresponding ask with this
  "expectsReply": true,   // the other side is asking, blocked waiting ← highest-priority signal
  "content": {
    "text": "Need your confirmation",
    "attachments": [
      { "type": "snippet", "name": "auth.ts", "language": "typescript", "content": "…" }
    ]
  }
}
```

`attachments.type` values: `file` / `snippet` / `context`.

**`expectsReply: true` is the only signal that should trigger a push on the phone** — it means an agent is blocked waiting on you to reply, not merely sending a notification.

---

## Delivery semantics

The broker owns addressing, the ask edges, and delivery state; each Harness adapter owns durable enqueue and injecting into the target session at a safe moment. So **don't re-implement a delivery-timing judgment in TmuxDeck to avoid interrupting an agent** — just `send`. That's the core reason intercom beats `send-keys` shoving characters in blindly (the latter gets swallowed or interrupts a thinking TUI).

v3 delivery is two-phase: `delivery_accepted` only means the broker accepted it; the receiver must send `message_received { deliveryId }` after processing the `message`, and only then does the sender see `delivered`. The business `message.id` cannot substitute for `deliveryId`.

### A reply must come from the session that received the ask

The broker validates `replyTo` at the **sessionId level** (`broker.ts`: `replyEdge.to !== currentId` returns `delivery_failed`):

- Opening a new connection and re-registering (even with the same name) gets a new sessionId and **cannot reply to the same ask**;
- A reply can only be sent on **the connection that received the ask** (`intercom.rs`'s `reply()` does exactly this — same persistent connection, same sessionId);
- Tested: an independent process sending with `replyTo` is rejected with `Reply target does not match the pending ask`.

Impact on the phone: TmuxDeck must hold a single persistent connection and always reply through it; it cannot open a fresh connection per reply.

---

## Differences between the two branches

The current verified Pi adapter on this machine is the GitHub-only maintenance release **`@dataforxyz/agent-intercom-pi` `0.10.1-tmuxdeck.1`**, tag [`v0.10.1-tmuxdeck.1`](https://github.com/ctliz/agent-intercom-pi/releases/tag/v0.10.1-tmuxdeck.1), commit `452b63f11d50dcdbbcf8485eb04d19928bbbfb13`. It is based on upstream Pi `v0.10.0` (`85c118453a15b3631b2a1eb289b66a65d1ac6ab2`) and tracks the fixes upstream in [agent-intercom-pi#20](https://github.com/dataforxyz/agent-intercom-pi/issues/20).

No global Codex, Claude, or OpenCode adapter package was detected in the same verification, so do not infer that all harness adapters are installed or share one package version. TmuxDeck's optional Managed Claude adapter is separately pinned and installed under TmuxDeck's config directory on macOS; Standard Claude, Codex, and OpenCode adapters remain independently managed. All participants that share a broker must still speak compatible protocol v3.

The table below keeps the historical differences from the original `nicobailon/pi-intercom` (pi-only), for troubleshooting older environments:

| | Original | Cross-harness |
|---|---|---|
| Supported agents | pi only | Pi, Codex, Claude Code, OpenCode |
| Runtime files | `broker.sock` `broker.pid` `config.json` | plus `broker.owner`, `broker-asks.json`, `inbox/`, `outbox/` |
| Delivery persistence | none (only pi session history) | persistent inbox/outbox + ACK + offline replay |
| `ask` semantics | client hard-blocks for 10 minutes | soft-wait 30s then async; a late reply within 10 minutes is fine |
| Tool shape | single `intercom({action})` | split into `intercom_send` / `_ask` / `_reply` / … |
| License | MIT | AGPL-3.0-or-later |

**Protocol migration is all-or-nothing**: mixing incompatible protocol generations splits clients into mutually invisible broker "islands". For a protocol-v3 maintenance update, run `/reload` in every Pi session and restart companion adapters so the shared broker can restart cleanly. Package version strings do not have to be identical when the adapters are independently verified as protocol-v3 compatible.

The Pi maintenance adapter defaults discovery and name/ID-prefix routing to the canonical current workspace, supports explicit `scope: "machine"`, and accepts an exact full session ID as the intentional cross-workspace route. These are client-side filtering and fail-closed routing semantics. They are **not** wire authorization, broker isolation, credentials, or a new security boundary; the protocol-v3 broker remains machine-global for the same OS user, and other adapters may still present a machine-global roster.

> For the phone scenario the cross-harness version is clearly the better fit: when you're away from the desk, a hard 10-minute blocking `ask` is a bad semantic.

### License note

Implementing a client yourself against the wire protocol **does not constitute a derivative work**; `intercom.rs` is an independent implementation and copies no upstream source. If you later need to modify the upstream adapter itself, the AGPL applies.

---

## Upstream

- [nicobailon/pi-intercom](https://github.com/nicobailon/pi-intercom) (MIT, pi-only)
- [dataforxyz/agent-intercom-pi](https://github.com/dataforxyz/agent-intercom-pi) (AGPL, cross-harness)
- [ctliz/agent-intercom-pi v0.10.1-tmuxdeck.1](https://github.com/ctliz/agent-intercom-pi/releases/tag/v0.10.1-tmuxdeck.1) (GitHub-only Pi maintenance release based on upstream v0.10.0)
