# Managed Claude Intercom resource

TmuxDeck bundles the following third-party source artifact for the optional macOS **Managed Claude Intercom** adapter:

- File: `agent-intercom-claude-0.12.0-connect.3.tgz`
- Size: `244233` bytes
- SHA-256: `f246fe19c43f2a2a487e9d86620c20e7d5686e11adb7fc632281f390c87049ad`
- Package version: `0.12.0-connect.3`
- Source fork commit: [`912a5fa99092ab7c903818a91a3a594d30afbb4d`](https://github.com/ctliz/agent-intercom-claude/commit/912a5fa99092ab7c903818a91a3a594d30afbb4d)
- Fork release/source offer: <https://github.com/ctliz/agent-intercom-claude/releases/tag/v0.12.0-connect.3>
- Based on upstream: `@dataforxyz/agent-intercom-claude` provenance
- Maintenance change: Agent Intercom protocol v4 support, packaging Claude Monitor files required for `cci --tui --safe`.
- License: `AGPL-3.0-or-later`

The archive includes its original `LICENSE`, `THIRD_PARTY_NOTICES.md`, `COPYRIGHT`, `LICENSE_TRANSITION.md`, and third-party license files. TmuxDeck preserves those files in the per-user managed installation directory.

TmuxDeck verifies the pinned archive digest before extraction, rejects links, special files and unsafe paths, validates the Claude plugin/Monitor/MCP chain and JavaScript runtime, and installs it without reading from or writing to the user's global npm installation.
