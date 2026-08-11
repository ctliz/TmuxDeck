# TmuxDeck roadmap

> Maintained by product (tmux-producter). Check an item off and record the version when it's done; schedule changes must be updated here.
> Version convention: feature code names follow the PRD numbering (v1.x); release version numbers increment from v1.7.0 by actual releases.

## Current status

- **Latest release:** v1.8.0 (2026-08-11) — conversation-bridge backend and secure WebSocket transport, pane/card drag reordering, new icon system, targeted-communication fixes
- **Trunk:** direct pushes to main, CI (macOS + Windows build + frontend/backend tests) all green
- **Test suite:** 43 backend tests passing (plus 1 on-device test ignored) + 3 frontend tests + CI test step
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
- [ ] Full mobile UI and push entry point

**Unresolved:** the full mobile UI is not yet delivered; the secure WebSocket transport and conversation protocol are in place.

**External dependency:** this machine runs the original `nicobailon/pi-intercom` (pi-only); Claude Code / Codex are still islands. Going cross-harness requires an overall migration to the `dataforxyz` family, and it **must be all-or-nothing** (mixing old and new splits the broker). This is a user decision item.

### P1 · Windows on-device verification (deferred, schedule TBD)

- Acceptance checklist: `docs/WINDOWS-VERIFICATION-v1.7.0.md` (A env pre-check / B install / C bridging / D GUI)
- Progress: A1–A3 PASS (tmux 3.4, codex/opencode detected, wt/cmd/powershell all present); B/C/D to be scheduled
- Host: `tsiji@192.168.1.17` (access via server-deploy skill "Windows host access")
- Trigger: executed after user confirms schedule; A/B/C over SSH, D needs GUI cooperation on the Windows machine
- Completion criterion: all PASS or only D8 skipped → Windows upgraded from "compiles" to "usable on real hardware"

### P2 · Milestone candidates (awaiting user decision to launch)

| Candidate | Value | Effort estimate | Notes |
|---|---|---|---|
| Per-pane agent mixing | run different agent orchestrations in a single workspace | medium | v1.1 PRD explicitly ruled this out; demand not yet validated |
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
