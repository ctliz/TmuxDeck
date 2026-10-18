# scripts

Scripts for development and verification. Not shipped in release packages.

## `update-plugins`

Requires Node.js 24+, npm, Python 3 and tar. Operates only on bundled resources and a temporary npm prefix/cache; no global installations or user adapter configs are changed.

```sh
scripts/update-plugins                   # query npm latest, pack and regenerate
scripts/update-plugins --check           # CI: compare npm latest + verify frozen/generated artifacts
scripts/update-plugins --check --offline # verify the existing lock without network
```

`src-tauri/resources/plugins.lock.json` records versions, archive size/SHA-256/SHA-512 SRI, immutable file digests, Core tree digest, and the OpenCode dependency closure. Its `recommended.pi` entry tracks npm latest for `@ctliz/pi-intercom`; `--check` reports version drift, but Pi is never packed into the app or installed automatically. The script generates `commands/plugins.generated.rs`, Tauri resource references and `resources/README.md`. Rust consumes the generated constants; JS package probes use the same target versions rather than hardcoded strings.

The OpenCode closure is rebuilt when Core, the OpenCode adapter or SDK versions change. It omits optional native addons and npm `.bin` links. The app extracts this verified complete snapshot and promotes its package lock directly, so installation works with an empty npm cache. Required entrypoints are checked before accepting an upstream layout change; review upstream API/permission changes and run `cargo build`, `cargo test` and `npm run build` after every update. `--check` never rewrites the repository and exits nonzero for drift.

---

## `intercom-probe.mjs`

Verifies that TmuxDeck can join the pi-intercom broker as a "human adapter".
**Run this before writing any intercom-dependent feature.**

The protocol implementation aligns with the upstream `types.ts` and `broker/framing.ts`; details in [`docs/REFERENCE-intercom-protocol.md`](../docs/REFERENCE-intercom-protocol.md).

### Prerequisites

At least one pi session with `pi-intercom` installed must be running — the broker is launched by it, and the broker shuts itself down 5 seconds after the last session exits.

```sh
ls ~/.pi/agent/intercom/broker.sock   # exists → broker is running
```

### Usage

```sh
# Register and stay resident: list all online sessions, print received messages
node scripts/intercom-probe.mjs

# Send one message and exit
node scripts/intercom-probe.mjs send <target-session-name-or-id> "message text"

# While resident, auto-reply to asks (verifies the full ask→reply path, avoiding blocking the waiter for 10 minutes)
PROBE_AUTOREPLY=1 node scripts/intercom-probe.mjs
```

> **Why an ask can't be answered standalone**: the broker enforces "a reply must come from the session that received the ask" (sessionId-level identity; `broker.ts` rejects `replyEdge.to !== currentId`). An independent process re-registers as a new session and cannot reply to the same ask — a reply can only happen on the same connection. The real TmuxDeck uses one persistent connection (`intercom.rs`'s `reply()`), matching semantics; the probe auto-replies on the same connection via `PROBE_AUTOREPLY=1` to exercise the same path.

### Verification checklist

| # | Step | Pass criterion |
|---|---|---|
| 1 | `node scripts/intercom-probe.mjs` | prints `✓ registered sessionId=…` |
| 2 | Observe the session list | every pi session has a status (`idle` / `thinking` / `tool:…`) |
| 3 | In any pi session run `intercom({ action: "list" })` | the list shows `tmuxdeck-probe` |
| 4 | In pi run `intercom({ action: "send", to: "tmuxdeck-probe", message: "hi" })` | the probe prints `📨 from …` |
| 5 | In pi switch to `action: "ask"` (probe running with `PROBE_AUTOREPLY=1`) | the probe prints `⚠ the other side is waiting for a reply (ask)` and auto-replies to release the wait |
| 6 | `node scripts/intercom-probe.mjs send <pi-session-name> "received"` | prints `✓ delivered`, and the message appears in the pi session |

Passing #2 proves you do **not** need to implement state detection yourself;
passing #4 and #5 proves the notification path works; #6 proves the phone-reply path works.

If all three are green, every assumption in `src-tauri/src/bridge.rs` holds.

### Common results

| Symptom | Cause |
|---|---|
| `✗ cannot find broker socket` | no pi session running, or the broker exited due to idleness |
| Session list only shows yourself | other pi sessions don't have `pi-intercom` installed, or it wasn't `/reload`ed after install |
| Can't see Claude Code / Codex | this machine has the pi-only original; cross-harness requires migrating to the `dataforxyz` family |
| `✗ delivery failed: …` | target name duplicated or nonexistent — use the session ID when names collide |
