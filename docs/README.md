# Docs index

## Where to start

| You are | Read first |
|---|---|
| A user | [README](../README.md) · [Simplified Chinese](../README.zh-CN.md) |
| Looking to change code | [CONTRIBUTING](../CONTRIBUTING.md) → [ARCHITECTURE](./ARCHITECTURE.md) |
| Curious what's in flight | [ROADMAP](./ROADMAP.md) |
| Picking up the v1.12 conversation bridge | [PRD-v1.12](./PRD-v1.12-conversation-bridge.md) → [DECISIONS-v1.12](./DECISIONS-v1.12.md) → [DESIGN-v1.13](./DESIGN-v1.13-transcript-source.md) → [DESIGN-v1.14](./DESIGN-v1.14-transport-security.md) |

## Engineering docs

| Doc | Contents |
|---|---|
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Module map, data flows, two easy-to-miss key implementations |
| [GUIDE-cross-harness-agent-intercom.md](./GUIDE-cross-harness-agent-intercom.md) | Cross-harness Intercom install, naming and collaboration guide for Agent Intercom protocol v4, including broker-enforced workspace scoping, Pi `@ctliz/pi-intercom@0.12.1`, and Managed Claude 0.13.0-connect.1 |
| [REFERENCE-intercom-protocol.md](./REFERENCE-intercom-protocol.md) | Agent Intercom wire protocol reference (protocol v4) |
| [DESIGN-v1.13-transcript-source.md](./DESIGN-v1.13-transcript-source.md) | Conversation content source design (Claude Code JSONL preferred + fallback) |
| [DESIGN-v1.14-transport-security.md](./DESIGN-v1.14-transport-security.md) | Mobile transport and security design |
| [ROADMAP.md](./ROADMAP.md) | Schedule and progress, maintained by product |
| [SIGNING-DECISION.md](./SIGNING-DECISION.md) | Why we are not code-signing yet |
| [WINDOWS-VERIFICATION-v1.7.0.md](./WINDOWS-VERIFICATION-v1.7.0.md) | Windows on-device acceptance checklist |

## Decision log

| Doc | Contents |
|---|---|
| [DECISIONS-v1.12.md](./DECISIONS-v1.12.md) | The five rejected approaches for mobile access and why — **read before proposing a new one** |
| [PRIOR-ART-agent-bus.md](./PRIOR-ART-agent-bus.md) | Survey of existing solutions: why not build our own agent bus |

## Feature PRDs

Ordered by version number. Writing a PRD before starting a feature is this project's documentation discipline.

| PRD | Topic |
|---|---|
| [v1.1](./PRD-v1.1.md) | Initial product definition |
| [v1.2](./PRD-v1.2-i18n.md) | Internationalization (en / zh-CN) |
| [v1.3](./PRD-v1.3-windows.md) | Windows support (WSL) |
| [v1.4](./PRD-v1.4-activity.md) | Activity timestamps |
| [v1.5](./PRD-v1.5-preview.md) | Pane content preview |
| [v1.6](./PRD-v1.6-liquid-glass.md) | Liquid glass visual design |
| [v1.7](./PRD-v1.7-tray.md) | Menu bar persistence |
| [v1.8](./PRD-v1.8-terminal-icons.md) | Terminal icons |
| [v1.9](./PRD-v1.9-card-header.md) | Card header simplification |
| [v1.10](./PRD-v1.10-pane-mgmt.md) | Pane-level management |
| [v1.11](./PRD-v1.11-focus-existing.md) | Duplicate-open prevention |
| [v1.12](./PRD-v1.12-conversation-bridge.md) | **Conversation bridge: intercom integration + multi-conversation mobile access** |

## macOS E2E safety rules

Real-app E2E tests **must not** quit TmuxDeck via AppleScript or LaunchServices by bundle ID, e.g. `tell application id "com.ctliz.tmuxdeck" to quit`. That path can trigger a Dock coalition sweep, which kills the tmux and Agent processes inside the app's coalition along with it.

Test instances may only be terminated by the **exact PID recorded at launch**: send `SIGTERM` to that PID and wait for it to exit; only after a timeout and a re-check confirming it is still the same test PID may you send `SIGKILL` to that exact PID. Do not use bundle ID, app name, `pkill`, `killall`, or process-name matching. Tests must also not auto-restore or launch user apps on completion, unless that run's acceptance explicitly authorizes it.

## Release notes

[v1.5.0](./RELEASE-NOTES-v1.5.0.md) · [v1.6.0](./RELEASE-NOTES-v1.6.0.md) ·
[v1.7.0](./RELEASE-NOTES-v1.7.0.md) · [v1.7.1](./RELEASE-NOTES-v1.7.1.md) ·
[v1.7.2](./RELEASE-NOTES-v1.7.2.md) · [v1.8.0](./RELEASE-NOTES-v1.8.0.md) ·
[v1.9.0](./RELEASE-NOTES-v1.9.0.md) · [v1.9.1](./RELEASE-NOTES-v1.9.1.md) ·
[v1.9.2](./RELEASE-NOTES-v1.9.2.md) · [v1.9.3](./RELEASE-NOTES-v1.9.3.md) ·
[v1.9.4](./RELEASE-NOTES-v1.9.4.md) · [v1.10.0](./RELEASE-NOTES-v1.10.0.md) ·
[v1.11.0](./RELEASE-NOTES-v1.11.0.md) · [v1.11.1](./RELEASE-NOTES-v1.11.1.md) ·
[v1.12.0](./RELEASE-NOTES-v1.12.0.md) · [v1.13.0](./RELEASE-NOTES-v1.13.0.md) ·
[v1.14.0](./RELEASE-NOTES-v1.14.0.md) · [v1.14.1](./RELEASE-NOTES-v1.14.1.md) ·
[v1.14.2](./RELEASE-NOTES-v1.14.2.md) · [v1.14.3](./RELEASE-NOTES-v1.14.3.md) ·
[v1.14.4](./RELEASE-NOTES-v1.14.4.md) · [v1.14.5](./RELEASE-NOTES-v1.14.5.md) ·
[v1.14.6](./RELEASE-NOTES-v1.14.6.md) · [v1.14.7](./RELEASE-NOTES-v1.14.7.md) · [v1.14.8](./RELEASE-NOTES-v1.14.8.md) · [v1.14.9](./RELEASE-NOTES-v1.14.9.md) · [v1.14.10](./RELEASE-NOTES-v1.14.10.md) · [v1.14.11](./RELEASE-NOTES-v1.14.11.md)
