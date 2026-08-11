# TmuxDeck v1.12 Conversation Bridge: Intercom Integration + Multi-Conversation on Mobile

> Goal: **talk to the agents in multiple panes at once from your phone**, and be able to hand content from A to B.
> This is not terminal emulation and not a notification inbox — it's a set of conversations that can run in parallel, one conversation object per pane.
> Transport is a persistent connection (WebSocket-shaped), not a one-way "push a notification" channel.

---

## 0. Decision record

| Item | Decision | Rationale |
|---|---|---|
| Bus | **Reuse the pi-intercom broker, don't build our own** | A cross-harness family already exists and has solved status, queued delivery, and receipts |
| TmuxDeck's role | **intercom's "human adapter"** | Every harness in the family (Pi/Codex/Claude/OpenCode) has an adapter; only "the human" is missing |
| Mobile shape | Multi-conversation client (persistent connection) | User need: enter any pane to chat, and forward across panes |
| Push channel | **Abstract layer only for now** (`Transport` trait) | Concrete integration with ntfy / Feishu / TG is undecided; don't bind early |
| ~~Four-state heuristic / silent inference~~ | **Removed** | The broker reports `idle` / `thinking` / `tool:<name>` directly — fact, not guesswork |
| ~~Self-built message bus~~ | **Reverted** | See `PRIOR-ART-agent-bus.md` |

Research basis in [`PRIOR-ART-agent-bus.md`](./PRIOR-ART-agent-bus.md).

---

## 1. Architecture

```
┌──────────────────── TmuxDeck ────────────────────┐
│                                                   │
│  React desktop UI ──invoke──┐                     │
│                             ├──▶ tmux.rs ──▶ tmux │
│  bridge.rs (conversation bridge)──────┘          │
│      │                                            │
│      ├── ConversationRegistry                     │
│      │     pane list ⊕ intercom sessions → conversation table │
│      │                                            │
│      ├── intercom.rs ──unix socket──▶ broker.sock │
│      │     registers as "me"; sends/receives directed messages, subscribes to status │
│      │                                            │
│      └── Transport (trait)──▶ mobile client       │
└───────────────────────────────────────────────────┘
```

Three data paths, each with its own source:

| Purpose | Source | Status |
|---|---|---|
| Which conversations exist, and their status | broker registry + `tmux list-panes -a` | ✅ implemented |
| me → agent | `intercom send` (preferred) / `send-keys` (fallback) | ✅ implemented |
| agent → me | `TranscriptSource` | ⚠️ see section 4; the only undecided piece |

---

## 2. Implemented (this round)

### `tmux.rs`

- `list_all_panes()` — one `list-panes -a` to get every pane's session / process / cwd
- `send_keys(pane, text, submit)` — free-form text goes through the `-l` literal channel; multi-line is sent line by line
- `send_key_name(pane, key)` — control-key **whitelist** channel (`Escape` / `C-c` / arrow keys etc.)

> The two channels are deliberately separated: without that, a "C-c" inside a message would be executed by tmux as a control key.

### `intercom.rs`

pi-intercom broker client, aligned with upstream `types.ts` and `broker/framing.ts`:

- Transport is a Unix domain socket; framing is 4-byte big-endian length + JSON, 1 MiB max per frame
- `connect()` registers as `me` (`model: "human"`, so other sessions instantly see in `list` that this is a person)
- `request_list` / `send` / `reply` / `acknowledge` / `update_presence`
- Dedicated reader thread → `mpsc::Receiver<IntercomEvent>`
- **Inbound frames dispatched by hand**: unknown types are ignored rather than erroring. The upstream protocol is evolving
  (the dataforxyz branch is already at v3); tolerating unknowns is mandatory
- No new dependencies (reuses existing serde / serde_json / dirs)

### `bridge.rs`

- `AgentKind::from_command` — recognizes the agent type from `pane_current_command`;
  **when an agent is running a tool, its process name temporarily becomes `bash`; don't downgrade kind back to Shell then**
- `ConversationRegistry` — pane table ⊕ intercom session table → unified conversation table,
  `list()` sorts with "waiting for a human" first
- **pane ↔ intercom session association**: intercom reports the agent process pid,
  tmux's `pane_pid` is usually that shell, so walk up the parent-process chain to match (max 12 levels, cycle-guarded)
- `deliver()` — with intercom, go through the broker (queues when busy, doesn't interrupt a thinking agent); otherwise fall back to send-keys
- `forward()` — cross-conversation forwarding, auto-prefixing the source
- `Transport` / `ClientEvent` / `ClientCommand` — mobile transport abstraction + `LogTransport`

---

## 3. Mobile protocol (defined; transport not yet connected)

Events (server → mobile):

```jsonc
{ "type": "conversations",   "items": [ /* Conversation[] */ ] }
{ "type": "status-changed",  "id": "%3", "status": "awaiting-human" }
{ "type": "turn",            "turn": { "conversationId": "%3", "role": "agent", "text": "…" } }
{ "type": "awaiting-human",  "id": "%3", "title": "backend", "preview": "…", "replyTo": "m-1" }
```

Commands (mobile → server):

```jsonc
{ "type": "say",     "id": "%3", "text": "continue" }
{ "type": "key",     "id": "%3", "key": "Escape" }
{ "type": "forward", "from": "%1", "to": "%3", "text": "…" }
{ "type": "refresh" }
{ "type": "subscribe",     "id": "%3" }  // enter a conversation: push only its turns
{ "type": "unsubscribe" }                  // leave the current conversation: stop pushing turns
```

**Subscription rules (new in v1.14; triage and content separated)**:

| Event | Push scope |
|---|---|
| `conversations` / `status-changed` / `awaiting-human` | **Push everything** — triage information must be fully known |
| `turn` (conversation content) | **Push only the subscribed conversation** — single active subscription; a new `subscribe` replaces the old one; `unsubscribe` clears it |

- Single active subscription: the phone views one conversation at a time; `subscribe` with a new id switches (minimal; parallel multi-view would need a subscription set, not in v1.14)
- On `subscribe`, the server immediately pushes that conversation's transcript tail once (the starting snapshot for incremental-cursor resume); otherwise the phone would see a blank when switching over
- **Transcript polling cost narrows to subscription granularity**: unsubscribed conversations run no polling (a dozen outputs don't burn bandwidth/CPU)

`Conversation.status` values: `idle` / `thinking` / `running-tool` / `awaiting-human` / `unknown`.
`awaiting-human` comes from an intercom message's `expectsReply` — **the peer is blocked waiting for you**.
That is the only signal that should trigger a push on the phone.

---

## 3.5 Tell agents "you can reach a human"

The technical chain working doesn't mean it gets used — **an agent must know the `me` address exists, and when to use it.**
This step is pure documentation, but without it the whole feature won't happen on its own.

Add to each project's `AGENTS.md` / `CLAUDE.md`:

```xml
<intercom-human>
There is an intercom session named `me` on this machine. It is a human (TmuxDeck's mobile client).

**When to reach me:**
- Stuck and unable to decide on your own (needs product judgment, needs authorization, disagreement on approach)
- Before irreversible actions (deleting data, changing production config, force push)
- Task complete, and the next direction needs a human to decide

**When not to reach me:**
- Things you can verify yourself
- Routine progress reports
- Questions another agent can answer — ask it over intercom first

**Which to use:** need an answer, use `ask` (the human gets a push); just informing, use `send`.
</intercom-human>
```

> The difference between `ask` and `send` on the phone is **whether it pushes**: `ask` means an agent is blocked
> waiting for your reply and will push; `send` just leaves an unread message in the conversation. Get agents to use
> the right one and notifications won't become noise.

---

## 4. The one undecided piece: where conversation content comes from

"Which conversations exist and their status" and "how I talk" are both solved. What remains is
**what the agent said** — we need per-turn conversation content. Three candidates:

| Approach | Feasibility | Problems |
|---|---|---|
| `capture-pane` | Implemented as fallback (`CapturePaneSource`) | Current screen only, no history; TUI redraws make content flicker; no turn boundaries |
| `pipe-pane` raw stream | Can capture all bytes | Full of cursor movement and redraw escape sequences; reconstructing "who said what" is very hard |
| **Read the agent's own structured session records** | **Recommended** | Natively clean per-turn data (e.g. Claude Code's `~/.claude/projects/**/*.jsonl`); the cost is one reader per agent, and associating a pane with its record file |

The `TranscriptSource` trait is in place; the implementation is undecided. Recommendation: option 3 as the primary path, option 1 as fallback.

> The association problem is already half-solved: `bridge.rs`'s parent-process-chain walk can tie a pane to the agent process,
> and once pid + cwd are known, locating that agent's session-record file is doable.

---

## 5. Prerequisite: local intercom version

`ls ~/.pi/agent/intercom/` shows the locally installed version is **`nicobailon/pi-intercom` original (pi-only)**,
not the `dataforxyz` cross-harness fork:

| File | Local | Original | Cross-harness version |
|---|---|---|---|
| `broker.sock` / `broker.pid` / `extension-state` | ✅ | ✅ | ✅ |
| `broker.owner` | ❌ | none | yes |
| `inbox/` `outbox/` `broker-asks.json` | ❌ | none | yes |

**Impact**: only pi sessions can join the bus right now; Claude Code / Codex remain islands
and can only go through the `send-keys` fallback. Wiring them up requires migrating wholesale to the dataforxyz family —
upstream explicitly warns that mixing old and new adapters splits the broker into mutually-invisible "islands"; **you must upgrade everything and `/reload`**.

The two versions also differ on something critical for mobile: the original's `ask` hard-blocks the client for 10 minutes,
and messages only live in the pi session's history; the new version has persistent inbox/outbox, ACK, and offline replay,
and `ask` soft-waits 30 seconds then goes async. **When you're not at your desk, the latter's semantics are clearly better.**

---

## 6. Acceptance

Verifiable already (`cargo test`):

- [ ] `send_keys` returns the corresponding error code for `%abc`, empty text, and content over 8 KiB
- [ ] `send_key_name` rejects keys outside the whitelist
- [ ] intercom frame read/write round-trips consistently; length prefix is 4-byte big-endian
- [ ] Unknown-type broker frames are ignored rather than causing errors
- [ ] `AgentKind` is not downgraded to `Shell` while an agent runs a tool
- [ ] The conversation list puts `awaiting-human` first
- [ ] After a pane disappears, the conversation table and intercom mapping are cleaned up in sync

Requires real-device verification (run `scripts/intercom-probe.mjs` first):

- [ ] Probe can connect to the broker and register successfully
- [ ] Other pi sessions can see us in their `intercom list`
- [ ] Messages pi sends us arrive; `expectsReply` is correctly recognized
- [ ] Messages we send to pi sessions are delivered
- [ ] The parent-process-chain walk correctly ties the intercom session to its pane

---

## 7. Roadmap

| Version | Content |
|---|---|
| v1.13 | Concrete `TranscriptSource` implementation (Claude Code JSONL first); conversation content wired through |
| v1.14 | `Transport`'s WebSocket implementation + mobile UI |
| v1.15 | Push channel integration (ntfy / Feishu / TG, pick one) |
| v1.16 | Desktop uses the same conversation table: cards sorted with "waiting for you" on top |
