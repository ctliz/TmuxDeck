# Contributing to TmuxDeck

Thanks for considering a contribution. This document is for developers; users should read the [README](README.md).

## Development setup

Requirements: Node.js, [Rust](https://www.rust-lang.org/tools/install), tmux, macOS.

```sh
git clone git@github.com:ctliz/TmuxDeck.git
cd TmuxDeck
npm install
npm run tauri dev      # dev mode with hot reload
npm run tauri build    # produce .app / .dmg
```

Artifacts are written to `src-tauri/target/release/bundle/`.

If `cargo` is not on your PATH, run `source "$HOME/.cargo/env"` first.

Stack: Tauri 2, React, TypeScript, Tailwind CSS, Rust.

## Project layout

```
src/App.tsx                    All frontend UI (single file)
src/i18n.ts                    en / zh-CN string tables

src-tauri/src/lib.rs           Tauri builder, tray wiring, command registration
src-tauri/src/tmux.rs          Core layer: the only place that shells out to tmux
src-tauri/src/registry.rs      Terminal / agent detection and icon resolution
src-tauri/src/config.rs        ~/.config/tmuxdeck/config.json
src-tauri/src/intercom.rs      pi-intercom broker client (agent bus)
src-tauri/src/bridge.rs        Conversation model: panes ⊕ intercom sessions
src-tauri/src/commands/        Thin Tauri command wrappers — no business logic

docs/                          Product specs (PRD-*.md) and guides
scripts/                       Dev-only verification scripts
```

Start with [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the data flows.

**Layering rule:** `commands/` only parses arguments and translates errors. Business
logic belongs in `tmux.rs` / `bridge.rs`. `intercom.rs` and `bridge.rs` must not depend
on the `tauri` crate — that keeps them unit-testable and extractable later.

## Adding a terminal

Two changes:

1. Add the entry to the registry in `registry.rs::detect_environment()`:

   ```rust
   ("wezterm", "WezTerm", vec!["/Applications/WezTerm.app"]),
   ```

2. Add a launch branch in `commands/session.rs::open_session()`:

   ```rust
   "wezterm" => Command::new("/usr/bin/open")
       .args(["-na", "WezTerm", "--args", "start", "--", &script_path])
       .status(),
   ```

### Why there is no quoting to worry about

All terminals execute the same intermediate script `/tmp/tmuxdeck-<session>.sh`, which contains `exec tmux attach-session -t '<name>'`. Because the shell handles quoting inside the script, each terminal launch only needs to pass a script path. Do not build attach commands as inline strings when adding a terminal; follow this pattern.

## Adding an agent

Add one line to the agent registry in `registry.rs::detect_environment()`:

```rust
("aider", "Aider", "aider"),   // (id, display name, executable)
```

Detection uses `which` plus the `~/.nvm/versions/node/*/bin/` glob. On Windows, detection runs inside WSL via `wsl.exe`.

## Contributing a Communication Connector or Adapter

TmuxDeck integrates with coding agents via **Agent Intercom Protocol v4** and **Core 0.2.0** (`@ctliz/agent-intercom-core`). We welcome community contributions and pull requests adding or improving communication adapters for coding agents.

### 1. Supported and Target Ecosystem Connectors

| Adapter Package | Target Harness | Role / Capabilities |
|---|---|---|
| `@ctliz/agent-intercom-pi` | **Pi** (`pi-coding-agent`) | Native broker integration, interactive messaging, tool calling |
| `@ctliz/agent-intercom-claude` | **Claude Code** (`claude`) | Managed MCP plugin, zero-manual-join auto-team, transcript polling |
| `@ctliz/agent-intercom-codex` | **Codex** (`codex`) | MCP server integration (`codex-server.mjs`), bridge daemon, multi-agent turns |
| `@ctliz/agent-intercom-opencode` | **OpenCode** (`opencode`) | Managed plugin, scope isolation, live event dispatch |
| `@ctliz/agent-intercom-orchestrator` | **Orchestrator** | Multi-agent coordination, peer roster management, team lifecycle |
| `@ctliz/agent-intercom-agy` | **Gemini CLI** (`agy`) | *(Planned v1.15.0)* Direct Gemini CLI bridge & messaging |

---

### 2. Provenance & Subresource Integrity Standard

All adapters packaged or managed by TmuxDeck must adhere to strict provenance rules:
- **Canonical Source**: Every adapter must originate from an authoritative GitHub repository (e.g. `https://github.com/ctliz/agent-intercom-<harness>`).
- **Exact Pinned Release**: Code must target a tagged release (e.g. `v0.12.0-connect.1`) and exact Git commit hash.
- **Bidirectional Subresource Integrity**: Pinned tarballs in `vendor/` must match their published registry counterpart with exact SHA-256 and SHA-512 hashes.
- **No Floating Semver**: Bare semver (`^`, `~`) without lockfile hash verification is forbidden.
- **Audit Matrix**: Maintain and verify provenance against the ecosystem publish matrix (`/tmp/ctliz-all-6-packages-published-matrix.json` format).

---

### 3. Core 0.2.0 Team Manifest & Auto-Team Architecture

Adapters participating in zero-manual-join Auto-Team must implement the Core 0.2.0 Team Manifest specification:
- **Environment Discovery**: The runner injects `AGENT_INTERCOM_TEAM_MANIFEST` pointing to an absolute path (e.g. `~/.config/tmuxdeck/teams/<teamId>/manifest.json`).
- **Manifest JSON Schema**:
  ```json
  {
    "schemaVersion": 1,
    "teamId": "tmuxdeck-team-00000000-0000-4000-8000-000000000001",
    "leadId": "tmuxdeck-00000000-0000-4000-8000-000000000001",
    "createdAt": "2026-08-15T12:00:00.000Z",
    "members": [
      {
        "sessionName": "lead-workspace",
        "sessionId": "tmuxdeck-00000000-0000-4000-8000-000000000001",
        "harness": "claude",
        "role": "lead",
        "paneIndex": 0,
        "slotNumber": 1,
        "cwd": "/path/to/project"
      },
      {
        "sessionName": "worker-workspace",
        "sessionId": "tmuxdeck-00000000-0000-4000-8000-000000000002",
        "harness": "codex",
        "role": "worker",
        "paneIndex": 1,
        "slotNumber": 2,
        "cwd": "/path/to/project"
      }
    ]
  }
  ```
- **Role Validation**: Workspaces require exactly one `lead`; all other members must be `worker`.
- **Environment Injections**:
  - `AGENT_INTERCOM_TEAM_MANIFEST`: Absolute path to manifest file (`0600` permissions).
  - `AGENT_INTERCOM_SESSION_ID`: Stable UUID-backed session ID.
  - `AGENT_INTERCOM_ROLE`: Either `"lead"` or `"worker"`.
  - `AGENT_INTERCOM_MANAGER_TARGET`: Target lead ID for workers.
  - `AGENT_INTERCOM_WORKSPACE_SCOPE_ID`: Scoped workspace namespace.

---

### 4. Offline Staging, Sandboxing & Filesystem Safety

Adapters must be safely managed without compromising host security:
- **Offline Staging**: Managed installations unpack bundled vendor tarballs (`vendor/*.tgz`) into an isolated `.staging.<nonce>` directory. Dynamic network access during installation is prohibited.
- **Atomic Rollback & Activation**: Directory activation uses atomic filesystem rename (`.staging.<nonce>` -> `<version>`). In the event of failure, previous versions are preserved and restored.
- **Permission Enforcement**: Managed root files must be strictly `0755` for executables/directories and `0644` for data files.
- **Anti-Traversal & Symlink Guards**: Managed roots and parent directory trees must reject symlinks (`verify_parent_not_symlink`).
- **Tamper Detection**: Staged roots compute and record immutable SHA-256 digests in `marker.json` upon installation.

---

### 5. MCP Server & Handshake Smoke Testing

Adapters exposing Model Context Protocol (MCP) or JSON-RPC capabilities (e.g. Codex, Claude):
- **MCP Server vs. CLI Launcher**: The configuration must execute the actual MCP server script (e.g. `node <managed_root>/dist/codex-server.mjs`) rather than an interactive CLI launcher.
- **Handshake Verification**: Spawning the MCP server must cleanly process JSON-RPC `initialize` on stdio:
  ```json
  {"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
  ```
  And return `jsonrpc: "2.0"`, matching request `id`, and server capabilities.
- **Cross-Architecture Path Resolution**: Runtime launchers must resolve Node and harness executables across Apple Silicon (`/opt/homebrew/bin/`), Intel macOS (`/usr/local/bin/`), NVM paths (`~/.nvm/versions/node/*/bin/`), and Linux/WSL without hardcoding.

---

### 6. Documentation & Version Integrity

- **Never Modify Historical Release Notes**: Past release notes (`docs/RELEASE-NOTES-v1.13.0.md`, `docs/RELEASE-NOTES-v1.14.0.md`) are immutable historical records.
- **New Release Notes**: Always add a new `docs/RELEASE-NOTES-vX.Y.Z.md` corresponding to the release.
- **Synchronized Versioning**: Update `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json` in lockstep.

---

### 7. Pull Request Checklist & Template for Adapters

When submitting a PR for a new or updated connector/adapter, include the following template in your PR description:

```markdown
### Connector / Adapter Contribution Details

- [ ] **Target Harness**: [e.g., Pi / Claude / Codex / OpenCode / Orchestrator / Agy]
- [ ] **Canonical Source Repository**: [URL]
- [ ] **Release Tag & Commit SHA**: `vX.Y.Z` (`<commit-hash>`)
- [ ] **Tarball Digest**: SHA-256 `<sha256>`
- [ ] **Subresource Integrity**: SHA-512 `sha512-...`
- [ ] **Core 0.2.0 Manifest Compatibility**: Verified against `AGENT_INTERCOM_TEAM_MANIFEST`
- [ ] **MCP / RPC Handshake Verification**: Verified with `initialize` JSON-RPC smoke test
- [ ] **Offline Staging Smoke**: Passed `npm pack` offline staging and permissions verification
- [ ] **Path Resolution**: Verified on Apple Silicon (`/opt/homebrew/bin`) and Intel (`/usr/local/bin`)
- [ ] **Security Review**:
  - [ ] No symlinks or path traversal in staging tree
  - [ ] Secret / token environment variables scrubbed from child processes
  - [ ] Fail-closed behavior on manifest or hash mismatch
- [ ] **All Test Gates Green**:
  - `npm test`
  - `npm run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml`
```

## Code conventions

- **Sanitize session names.** Every Tauri command that accepts a session name must call `sanitize_session_name()` first. The name is embedded in shell commands and file paths; skipping this is a command-injection vulnerability.
- **Never send free text without `-l`.** `tmux send-keys` interprets strings like `C-c` and `Escape` as key names. User text goes through `send_keys()` (which passes `-l`); control keys go through `send_key_name()`, which validates against an allow-list. Do not merge the two channels.
- **Do not guess agent state.** The intercom broker reports `idle` / `thinking` / `tool:<name>` as fact. Sessions not on the bus are `unknown` — leave them unknown rather than inferring from pane silence. See [`docs/DECISIONS-v1.12.md`](docs/DECISIONS-v1.12.md#5-polling-capture-pane-for-four-state-detection) for why the heuristic approach was removed.
- **Keep it minimal.** The "explicitly out of scope" list in the PRD (see `docs/`) includes per-pane agent mixing, workspace templates, a multi-entry custom agent manager, and remote SSH. Open an issue to discuss these before submitting a PR.
- **Ask only necessary questions.** This is the core design principle: hide a row when there is only one candidate, and never show tools that are not installed.

## Before submitting a PR

- [ ] `npm run tauri build` compiles
- [ ] If you touched pane layout: confirm `tmux list-panes -s -t <name> | wc -l` matches the requested count
- [ ] If you touched session name handling: test with `a'; rm -rf ~; '` and `../../etc/passwd`
- [ ] UI text goes through the i18n tables; no hardcoded user-facing strings

## Reporting issues

Include: macOS version, `tmux -V`, which terminals and agents are installed, and steps to reproduce.
