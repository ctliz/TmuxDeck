## TmuxDeck v1.13.0 release notes

TmuxDeck v1.13.0 integrates **Agent Intercom protocol v4**, upgrades macOS Managed Claude to **`0.12.0-connect.3`**, introduces broker-enforced workspace scoping, and clarifies aggregation semantics across desktop and mobile control surfaces.

### Agent Intercom Protocol v4 & Broker-Enforced Scoping

- **Broker-enforced workspace scoping:** In protocol v4, session discovery (`intercom_list`) and name/prefix resolution are enforced by the broker within the caller's workspace scope (`scopeId`).
- **Cross-scope routing:** Communicating across workspace boundaries requires specifying the **exact full session ID**.
- **Scope is same-OS-user isolation, not a security principal:** Scoping isolates discovery and prevents inadvertent crosstalk between different workspaces; it is an operational routing boundary, not a cryptographic security principal. The trust boundary remains the local OS user on the shared broker.
- **Zero raw scope exposure for frontend & mobile:** TmuxDeck's desktop dashboard and mobile interface maintain zero raw scope exposure (零原值暴露); the backend manages an independent scoped human client (`me`) per workspace and aggregates conversations into the unified conversation registry.

### Managed Claude `0.12.0-connect.3` on macOS

- The bundled offline adapter is upgraded to `agent-intercom-claude-0.12.0-connect.3.tgz` with protocol v4 support and packaged Monitor runtimes.
- Managed Claude continues to run with `--tui --safe`.
- Pinned SHA-256 verification, safe extraction into staging, Claude plugin → Monitor → runtime validation, and rollback semantics remain strictly enforced.
- Recreating a managed pane or native slot allocates a fresh cryptographically random incarnation ID.
- **Use Standard Claude** remains a persistent preference, and existing global `cci` installations are untouched.

### Companion Adapters & Canonical Installs

- **Pi:** `pi install git:github.com/ctliz/agent-intercom-pi@v0.11.0-connect.2` (recommended fixed Git tag; npm `@ctliz/agent-intercom-pi@connect` also published). Integrates protocol v4 workspace scoping with native Pi session name synchronization.
- **Codex:** `npm install -g @ctliz/agent-intercom-codex@connect` (`0.11.0-connect.2`).
- **OpenCode:** `cd ~/.config/opencode && npm install @ctliz/agent-intercom-opencode@connect` (`0.11.0-connect.2`).
- **Claude Code:** `@ctliz/agent-intercom-claude@connect` (`0.12.0-connect.3`), bundled in TmuxDeck as the offline Managed Claude adapter.
- **Core Internal:** `@ctliz/agent-intercom-core@0.1.0`.
- **Orchestrator:** `@ctliz/agent-intercom-orchestrator@connect` (`0.11.0-connect.2`), optional for Linux/systemd lifecycle management outside the Broker compatibility set; omitted on macOS.
- Ordinary message batch context is preserved across model tool loops.

### Fail-Closed Legacy Workspace Handling

- Workspaces created prior to v4 scoping metadata fail closed on pane additions or rename operations.
- Legacy workspaces should be recreated to attach proper v4 workspace scope metadata.

### Coordinated Upgrades for Installed Adapters Only

- When updating protocol versions, users only need to coordinate upgrades across **currently installed and active adapters**. Uninstalled harness adapters do not need to be installed.
- After updating, run `/reload` in all open Pi sessions and restart active companion adapters (`cci`, `coi`, OpenCode).

### Orchestrator

- Orchestrator is an optional Linux/systemd lifecycle product, outside the Broker compatibility set; omitted on macOS where the local on-demand broker lifecycle is used.

### Package Provenance & npm Publication

- The six v4 adapter packages (`core`, `pi`, `claude`, `codex`, `opencode`, `orchestrator`) are published publicly under the `@ctliz` npm scope, with internal core `@ctliz/agent-intercom-core@0.1.0`.
- Canonical and recommended install commands specify the `@connect` dist-tag (or exact version strings); future GA releases will advance the `latest` tag.
- Attribution and provenance to the original upstream `@dataforxyz/agent-intercom-*` ecosystem are preserved.

### Verification & Compatibility

- Version bumped to `1.13.0` across project manifests (`package.json`, `package-lock.json`, `Cargo.toml`, `tauri.conf.json`).
- Compatible with existing native Ghostty split layouts, batch pane creation, and trusted-LAN mobile conversation pairing.
