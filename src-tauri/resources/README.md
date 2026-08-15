# Bundled Agent Intercom Adapter Resources

TmuxDeck bundles third-party source artifacts for optional macOS **App-Private Managed Agent Intercom Adapters**. App-private installations are isolated per-user under the application data directory (`~/Library/Application Support/tmuxdeck/managed/` on macOS, `~/.config/tmuxdeck/managed/` on Linux) and never overwrite, modify, or read from global npm packages or system-wide node_modules.

## Bundled Artifacts

### 1. Agent Intercom Core
- File: `ctliz-agent-intercom-core-0.2.0.tgz`
- Size: `198861` bytes
- SHA-256: `9b6c72d57a9d00679dbdedcd91a9121e1028d9e208decd3ad6f4b9ba3c204556`
- Package version: `0.2.0`
- Canonical repository: <https://github.com/ctliz/agent-intercom-core>
- Source offer / release: <https://github.com/ctliz/agent-intercom-core/releases/tag/v0.2.0>
- Based on upstream: `dataforxyz/agent-intercom-*` provenance
- License: `AGPL-3.0-or-later`

### 2. Managed Claude Intercom Adapter
- File: `ctliz-agent-intercom-claude-0.13.0-connect.1.tgz`
- Size: `246551` bytes
- SHA-256: `a766f4631d92df3dc26ee81f9bec06da38c3c09bae9ea4c6b0ef3975eeeb96ba`
- Package version: `0.13.0-connect.1`
- Canonical repository: <https://github.com/ctliz/agent-intercom-claude>
- Source offer / release: <https://github.com/ctliz/agent-intercom-claude/releases/tag/v0.13.0-connect.1>
- Based on upstream: `dataforxyz/agent-intercom-claude` provenance
- License: `AGPL-3.0-or-later`

### 3. Managed Codex Intercom Adapter
- File: `ctliz-agent-intercom-codex-0.12.0-connect.1.tgz`
- Size: `1270329` bytes
- SHA-256: `37b14553e00ed7b501cb6289319a01c1a65543c0e8fb6a87e9caf1c379ed0a14`
- Package version: `0.12.0-connect.1`
- Canonical repository: <https://github.com/ctliz/agent-intercom-codex>
- Source offer / release: <https://github.com/ctliz/agent-intercom-codex/releases/tag/v0.12.0-connect.1>
- Based on upstream: `dataforxyz/agent-intercom-codex` provenance
- License: `AGPL-3.0-or-later`

### 4. Managed OpenCode Intercom Adapter
- File: `ctliz-agent-intercom-opencode-0.12.0-connect.1.tgz`
- Size: `218861` bytes
- SHA-256: `9756cc56a54313d606e655ae46af83bdd89a29178fb74f08144672a0fda008a3`
- Package version: `0.12.0-connect.1`
- Canonical repository: <https://github.com/ctliz/agent-intercom-opencode>
- Source offer / release: <https://github.com/ctliz/agent-intercom-opencode/releases/tag/v0.12.0-connect.1>
- Based on upstream: `dataforxyz/agent-intercom-opencode` provenance
- License: `AGPL-3.0-or-later`

### 5. OpenCode Plugin SDK
- File: `opencode-ai-plugin-1.18.18.tgz`
- Size: `11985` bytes
- SHA-256: `26ac7cc2608fc63e063a0b08857c277b17d75043ad37125667275932f17b3d43`
- Package version: `1.18.18`
- Canonical package: `@opencode-ai/plugin`
- License: `MIT`

### 6. OpenCode Offline Dependency Closure
- File: `opencode-sdk-closure.tgz`
- Size: `11510485` bytes
- SHA-256: `8e1d64c90fcf4a7ed73d6d4eaa1b726f8c6a647c82e1dbeba4af6c8d04f24237`
- Contents: exact offline `node_modules` closure for the frozen OpenCode SDK package and its 26 transitive packages
- Source: local npm cache, packed and verified offline

---

## Pi Intercom Adapter

The Pi Intercom adapter is not bundled as a tarball. It is installed via its canonical Git URL and tag:
```bash
pi install git:github.com/ctliz/agent-intercom-pi@v0.12.0-connect.1
```
License: `AGPL-3.0-or-later` (<https://github.com/ctliz/agent-intercom-pi>)

---

## Integrity, Security & Rollback Guarantees

TmuxDeck verifies the SHA-256 digest of every bundled resource before extraction or staging. App-private managed installations:
1. Are staged into private directories with strict `0700` directory and `0600` file permissions;
2. Execute npm with `--ignore-scripts`, `--no-audit`, and `--no-fund`;
3. Never write to or modify global npm directories;
4. Validate the exact installed Core protocol runtime (`dist/team-manifest.js` SHA-256 `28b5e6c7b2fa583b82adc23a3dcc7389e83818c544c5ddb4a3f7701f8fd8ee27`);
5. Validate exact marker contents, digests, and JS syntax prior to activation;
6. Are activated atomically with full rollback of roots and host config files upon any staging, configuration, or verification failure.
