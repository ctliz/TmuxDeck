# TmuxDeck roadmap

> Maintained by product (tmux-producter). Check an item off and record the version when it's done; schedule changes must be updated here.
> Version convention: feature code names follow the PRD numbering (v1.x); release version numbers increment from v1.7.0 by actual releases.

## Current status

- **Latest release:** v1.11.1 — fixes Claude Code and OpenCode detection for native installer paths
- **Release candidate:** v1.12.0 — Managed Claude fallback, atomic batch panes, and workspace-aware mobile conversations; not released yet
- **Trunk:** direct pushes to main; the v1.12.0 release-candidate tree passes local macOS frontend/backend verification, while Windows target verification remains pending
- **Test suite:** 124 backend tests passing + 2 environment/on-device ignored, and 40 frontend tests passing; physical-phone LAN acceptance remains pending
- **Quality baseline:** tmux no-server error handled (ERR_TMUX_NO_SERVER bilingual friendly prompt)

## Planning queue

### P0 · v1.12 Conversation bridge (in progress)

Mobile access. Positioning: TmuxDeck becomes pi-intercom's **"human adapter"** — the family already has Pi / Codex / Claude Code / OpenCode adapters, but no "human".

- Requirements & acceptance: `docs/PRD-v1.12-conversation-bridge.md`
- Architecture: `docs/ARCHITECTURE.md` · Protocol: `docs/REFERENCE-intercom-protocol.md`
- Decision log: `docs/DECISIONS-v1.12.md` (five rejected approaches) · `docs/PRIOR-ART-agent-bus.md`

Progress:

- [x] `tmux.rs`: `send_keys` / `send_key_name` (allow-list) / `list_all_panes`
- [x] `intercom.rs`: broker client (UDS + 4-byte big-endian framing + manual frame dispatch), no new dependencies
- [x] `bridge.rs`: conversation model, pane↔session parent-chain association, delivery routing, `Transport` abstraction
- [x] Docs landed (architecture / protocol / decisions / script notes / CONTRIBUTING sync)
- [x] **On-device verification:** all 6 items of `node scripts/intercom-probe.mjs` pass (2026-08-10, tmux-backend)
- [x] `cargo test` passes (27 items, 2026-08-10)
- [x] `TranscriptSource`: structured session-log reading for Pi / Claude Code, `capture-pane` fallback
- [x] WebSocket transport: token auth, subscription filtering, heartbeat and connection-level targeted replies
- [x] Trusted-LAN single-port HTTP+WS code: dynamic LAN pairing URLs, token/Host/source validation, embedded mobile SPA entry, client-count events, command audit
- [x] Mobile SPA code (single embedded HTML entry)
- [x] Workspace grouping from backend-authoritative `workspaceId` / `workspaceName` metadata, including native workspaces without session-name parsing (v1.12.0 RC)
- [x] Mobile conversation UI: compact actions, awaiting/offline states, context controls, Markdown rendering with raw-HTML escaping and DOMPurify sanitization, and authoritative `transcriptKind` labeling (v1.12.0 RC)
- [x] macOS pinned Managed Claude Adapter: offline SHA-verified install/repair, safe extraction, persistent Standard fallback, random pane/slot incarnation IDs, and fail-closed bridge association (v1.12.0 RC)
- [x] Atomic batch pane creation: one frontend invocation, backend count 1–6, rollback on failure, and one native layout rebuild (v1.12.0 RC)
- [ ] Physical-phone LAN acceptance: pairing, reconnect, multi-conversation, ask/reply, forward, firewall behavior (automated coverage complete; final real-device sign-off pending)
- [ ] External push entry point (works while the browser is suspended/closed)

**Security boundary:** LAN mode is plaintext and intended only for a trusted local network. Token authentication remains mandatory; VPN/TLS is a reserved extension.

**External dependency:** cross-harness operation uses the `@dataforxyz/agent-intercom-*` adapter family. The v1.12.0 release candidate bundles a pinned macOS maintenance artifact containing the Monitor packaging fix previously tracked in [agent-intercom-claude#6](https://github.com/dataforxyz/agent-intercom-claude/issues/6), so that upstream packaging issue is no longer a blocker for Managed Claude. TmuxDeck still falls back persistently to Standard Claude, and upstream Monitor warnings remain a follow-up. Protocol-v4 workspace-scoped discovery is also follow-up scope rather than part of v1.12.0.

### P1 · Windows on-device verification (deferred, schedule TBD)

- Acceptance checklist: `docs/WINDOWS-VERIFICATION-v1.7.0.md` (A env pre-check / B install / C bridging / D GUI)
- Progress: A1–A3 PASS (tmux 3.4, codex/opencode detected, wt/cmd/powershell all present); B/C/D to be scheduled
- Host: `tsiji@192.168.1.17` (access via server-deploy skill "Windows host access")
- Trigger: executed after user confirms schedule; A/B/C over SSH, D needs GUI cooperation on the Windows machine
- Completion criterion: all PASS or only D8 skipped → Windows upgraded from "compiles" to "usable on real hardware"

### P2 · Milestone candidates (awaiting user decision to launch)

| Candidate | Value | Effort estimate | Notes |
|---|---|---|---|
| Workspace templates / layout presets | reuse common layouts in one click | low-medium | same |
| macOS signing + auto-update | remove Gatekeeper warning, users auto-upgrade | medium-high | needs Apple developer account + tauri-updater |
| Split App.tsx | pay down tech debt, refactor before features grow | medium | single 987-line file; `lib.rs` already split on 2026-08-10 |

### P3 · Tech debt and continuous improvement

- [x] Split `lib.rs` into modules (tmux / registry / config / models / tray / commands, 2026-08-10)
- [x] Introduce automated tests (v1.7.0, 2026-08-10)
- [x] tmux no-server error handling (v1.7.0, 2026-08-10)
- [x] Command-line release flow (gh CLI, draft → formal release)
- [x] create_session field-naming fix (v1.7.1, 2026-08-10)
- [x] v1.7.2: Ghostty opening-session multi-instance bug (AppleScript new window, 2026-08-11)
- [ ] Terminal-launch method evaluation: potential same-class multi-instance issue with `open -na` on wezterm / kitty / alacritty (evaluate after the Ghostty fix; don't widen scope proactively)
- [ ] README wording: after Windows verification passes, update from "macOS is battle-tested" to a dual-platform statement

## Principles (following PRD conventions)

- Minimal-first: unvalidated demand doesn't enter the queue; every release has an explicit "not doing" list
- Small, fast steps: single release ≤ 2 person-days, tag at release
- Documentation discipline: write a PRD before starting a feature, always write RELEASE-NOTES at release
