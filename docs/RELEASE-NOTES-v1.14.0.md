## TmuxDeck v1.14.0 release notes

TmuxDeck v1.14.0 introduces **App-Private Communication Adapters** and **Zero-Manual-Join Auto-Team** coordination across heterogeneous coding agent harnesses, delivering seamless out-of-the-box Agent Intercom collaboration without manual configuration or global npm interference.

---

### App-Private Communication Adapters (macOS MVP)

- **Isolated App-Private Managed Roots**: Adapters for Claude Code, Codex MCP, and OpenCode are staged and managed in isolated app-private directories (`~/Library/Application Support/tmuxdeck/managed/`) without polluting system-wide packages or global `node_modules`.
- **Offline Bundled Assets**:
  - **Core 0.2.0**: `@ctliz/agent-intercom-core@0.2.0` (`ctliz-agent-intercom-core-0.2.0.tgz`) providing the unified team manifest parser and error semantics.
  - **Claude Code 0.13.0-connect.1**: `@ctliz/agent-intercom-claude@0.13.0-connect.1` with bundled Claude Monitor and `--tui --safe` support.
  - **Codex MCP 0.12.0-connect.1**: `@ctliz/agent-intercom-codex@0.12.0-connect.1` supporting MCP tool endpoints and dynamic discovery.
  - **OpenCode Plugin 0.12.0-connect.1**: `@ctliz/agent-intercom-opencode@0.12.0-connect.1` bundled with the exact `@opencode-ai/plugin@1.18.18` SDK and complete offline 26-package dependency closure (`opencode-sdk-closure.tgz`).
- **Pi Coding Agent**: Canonical fixed Git reference:
  ```bash
  pi install git:github.com/ctliz/agent-intercom-pi@v0.12.0-connect.1
  ```
- **Consent & Plan Execution**: Frontend introduces `AdapterConsentModal` and `check_workspace_adapters` / `apply_workspace_install_plan` transactional IPC for interactive user confirmation before applying adapter installations or repairs.
- **Atomic Rollback & Staging Protection**: All installation and update transactions use strict staging validation (`0700` dirs / `0600` files), verifying immutable marker digests and SHA-256 package checksums before atomic promotion.

---

### Zero-Manual-Join Auto-Team (Phase A & Phase B)

- **Explicit Team Manifest**: Workspaces generate isolated, permission-locked JSON manifests under `~/.config/tmuxdeck/teams/team_{uuid}.json` conforming to the `tmuxdeck.team.v1` schema.
- **Lead & Worker Roles**:
  - **Lead (Pane 1 / Slot 1)**: Initiates workspace coordination and receives manager status.
  - **Workers (Panes 2..N / Slots 2..N)**: Automatically associate with the Lead without requiring manual join commands or channel subscriptions.
  - In `CreateWorkspaceModal`, users can designate any pane agent as the Lead with one click (`Set as Lead`).
- **Fail-Closed macOS MVP Guard**: Team manifest operations enforce `cfg!(target_os = "macos")` to prevent invalid Windows host `%APPDATA%` paths inside WSL tmux.
- **Lead Guard on Kill**: Destroying the Lead pane while teammate panes remain active is blocked with `ERR_KILL_LEAD_NOT_ALLOWED` to preserve team hierarchy integrity.
- **Native Workspace Rollback**: Rollback upon workspace launch failures cleans up all allocated slot sessions and surfaces failures under `ERR_TEAM_ROLLBACK`.

---

### Verification & Test Coverage

- **184 Rust unit & integration tests** passing cleanly with full mock and offline staging suites.
- **45 frontend tests** validating modal workflows, zero scope leakage, and bilingual translations (`en` / `zh`).
- **TypeScript & Vite build** passing with zero bundle warnings.
