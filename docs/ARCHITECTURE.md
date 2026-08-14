# TmuxDeck architecture

> For developers and agents. Users should see the [README](../README.md).
> This describes the module structure and data flows for the v1.13 architecture.

---

## Module map

```
src-tauri/src/
├── main.rs           Entry point; only calls lib::run()
├── lib.rs            Tauri Builder, tray wiring, command registration
│
├── tmux.rs           ← Core layer: the only place that shells out to the tmux CLI
├── registry.rs       Terminal / agent detection and icon resolution
├── config.rs         ~/.config/tmuxdeck/config.json read/write
├── models.rs         Shared data structures across modules
├── tray.rs           Menu bar menu construction
├── audit.rs          Kill/rename audit trail + session/pane counters
│
├── intercom.rs       ← Agent Intercom broker client (protocol v4, ctliz agent-intercom v4, Pi v0.11.0-connect.2 / Claude 0.12.0-connect.3, dataforxyz provenance)
├── claude_adapter.rs ← Managed Claude adapter management (macOS pinned 0.12.0-connect.3, --tui --safe, SHA verification, rollback)
├── scope.rs          ← Workspace scope routing boundaries and validation (scope is routing boundary, not auth)
├── bridge.rs         ← Conversation bridge: panes ⊕ intercom sessions → unified conversation model;
│                       ConversationRegistry, pane↔session association, deliver/forward, Transport trait
├── bridge_state.rs   Read-only registry/transport snapshot published into Tauri state for the desktop UI
├── transcript.rs     TranscriptSource implementations: structured session-log reading (Pi / Claude Code) + capture-pane fallback
├── connection.rs     v1.14 WebSocket connection handling: accept loop, per-connection loop, handshake,
│                     rate limiting, heartbeat, framing, subscription state — pure transport, no dialog semantics
├── engine.rs         v1.14 bridge engine: one background loop — intercom events + mobile commands
│                     + periodic refresh / transcript polling; owns registry & transport (single-threaded)
├── transport.rs      v1.14 WebSocket server (Transport impl): token auth, Host allowlist, rate limits
│
└── commands/         Thin Tauri command wrappers — no business logic
    ├── session.rs    Session-level: create / open / list / delete / rename
    ├── pane.rs       Pane-level: add / delete / capture / send input
    ├── native.rs     Native Ghostty workspace model (see "Native Ghostty workspaces" below)
    └── utils.rs      Icons, WSL path conversion, agent-command isolation
```

**Layering constraint:** `commands/` only parses arguments and translates errors; business logic belongs in `tmux.rs` / `bridge.rs`. `intercom.rs` and `bridge.rs` **do not depend on the tauri crate** — that keeps them directly unit-testable and extractable into a standalone daemon later without changes.

The frontend is componentized: `src/main.tsx` mounts `App.tsx`, which composes `src/components/` (`CardGrid`, `SessionCard`, `CreateWorkspaceModal`, `NewWorkspaceCard`, `SearchHeader`, `TmuxMissingScreen`); `src/i18n.ts` holds the en / zh-CN strings, `src/types.ts` the shared types, and `src/utils.ts` the shared helpers.

---

## Agent Intercom v4 & Workspace Scoping

Starting with v1.13, TmuxDeck integrates **Agent Intercom protocol v4** (`ctliz` ecosystem with `@dataforxyz` provenance).

### 1. Broker-enforced workspace scope
- Peer discovery (`intercom_list`) and name/prefix resolution are enforced by the broker within the active workspace scope.
- Cross-scope communication is explicitly permitted only by supplying the **exact full session ID**.
- **Scope is same-OS-user isolation, not a security principal:** Scoping partitions discovery and prevents accidental cross-talk between workspaces; it is an operational routing boundary, not a cryptographic identity or authorization mechanism. The trust boundary remains the local OS user.

### 2. Zero raw scope exposure for frontend and mobile
- TmuxDeck desktop dashboard and mobile endpoints operate with zero raw scope exposure (零原值暴露).
- The backend manages an independent scoped human client per workspace and aggregates conversation events across all workspaces into the unified `ConversationRegistry`.

### 3. Fail-closed legacy workspaces
- Workspaces created prior to v4 scoping metadata fail closed on pane additions or renames.
- To enable proper v4 scope binding, legacy workspaces should be recreated.

### 4. Coordinated upgrade for installed adapters
- When migrating protocol versions, only **currently installed adapters** need to be upgraded together.
- Open Pi sessions require `/reload`, and companion adapters (`cci`, `coi`, OpenCode) must be restarted. Uninstalled adapters require no action.

### 5. Orchestrator deployment model
- Orchestrator is an optional Linux/systemd lifecycle product, outside the Broker compatibility set; omitted on macOS where the broker is automatically socket-activated and torn down on demand.

---

## Data flows

### Desktop dashboard (pre-existing)

```
React UI (src/components/) ──invoke("get_tmux_sessions")──▶ commands/session.rs ──▶ tmux.rs ──▶ tmux CLI
```

Polled every 4 seconds. This path was unchanged in v1.12.

### Conversation bridge (added in v1.12)

```
                     ┌─────────────────┐
tmux list-panes -a ──▶                 │
                     │  bridge.rs      │──▶ Conversation[] (waiting-for-human first)
broker session registry ─▶  Registry   │
                     └─────────────────┘
```

The three paths, their sources and status:

| Purpose | Source | Status |
|---|---|---|
| Which conversations exist and their status | broker registry + `tmux list-panes -a` | implemented |
| human → agent | `intercom send` (preferred) / `send-keys` (fallback) | implemented |
| agent → human | `TranscriptSource` | **undecided, see below** |

---

## Two easy-to-miss but critical implementations

### 1. pane ↔ intercom session association

The `pid` intercom reports is the **agent process itself**, whereas tmux's `pane_pid` is usually the **shell** in the pane — the agent is typically a child of the shell, sometimes behind a wrapper script (`cci` / `coi`). They are not equal, so direct matching fails.

`bridge.rs::find_owning_pane` therefore walks up the parent-process chain (`ps -o ppid=`), up to 12 levels, stopping on cycles or pid ≤ 1.

> cwd matching was considered, but multiple panes under one directory is the norm and creates ambiguity. The parent chain is deterministic.

### 2. AgentKind debouncing

`pane_current_command` returns the **foreground process name**. When an agent runs a bash tool, that value temporarily becomes `bash` — updating `kind` from it would misclassify the agent as a plain shell.

`ConversationRegistry::refresh_panes` therefore only updates `kind` when a concrete agent is recognized; when the detection yields `Shell` / `Unknown`, the previous value is kept.

---

## Input channels: two, never merged

| Channel | Use | Implementation |
|---|---|---|
| Literal text | User messages | `send_keys()` → `tmux send-keys -l` |
| Control keys | Escape / C-c / arrow keys | `send_key_name()` → allow-list validated, then `tmux send-keys <key>` |

**They must stay separate.** Without `-l`, `tmux send-keys` parses strings like `C-c` and `Escape` as key names — a user message containing those words would be executed as control keys. The control-key channel goes through its own allow-list, which also stops send-keys from becoming a general keyboard injection point.

Multi-line text is sent line by line (line content + explicit `Enter`), because some TUIs handle a bare `\n` inconsistently.

---

## Unresolved: where conversation content comes from

"Which conversations", "what status" and "how to talk" are all wired up; what's missing is **what the agent said**.

| Approach | Status | Problem |
|---|---|---|
| `capture-pane` | implemented as fallback (`CapturePaneSource`) | current screen only, no history, flicker from TUI redraws, no turn boundaries |
| `pipe-pane` raw stream | not implemented | full of cursor-movement and redraw escape sequences; reconstructing turns is very hard |
| **Read the agent's structured session logs** | **recommended primary path** | clean per-turn data by nature; needs one reader per agent |

The `TranscriptSource` trait is in place. Association is half solved — walking up the parent chain yields the agent's pid and cwd, from which its session log files can be located.

---

## Dependency stance

v1.12 **introduced no new Rust dependencies**: the intercom client uses only `std` plus the existing `serde` / `serde_json` / `dirs`. The Unix domain socket goes through `std::os::unix::net`, and the framing is hand-written.

On Windows the broker uses named pipes; `intercom.rs` is currently gated with `#[cfg(unix)]`, and Windows returns `ERR_INTERCOM_UNSUPPORTED_PLATFORM`, automatically degrading to the send-keys channel.

---

## Related docs

- [PRD-v1.12 conversation bridge](./PRD-v1.12-conversation-bridge.md) — requirements and acceptance
- [intercom protocol reference](./REFERENCE-intercom-protocol.md) — wire protocol details
- [v1.12 decision log](./DECISIONS-v1.12.md) — rejected approaches and why
- [Existing-solution survey](./PRIOR-ART-agent-bus.md) — why we don't build our own bus
