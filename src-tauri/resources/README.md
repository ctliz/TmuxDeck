# Managed Claude Intercom resource

TmuxDeck bundles the following third-party source artifact for the optional macOS **Managed Claude Intercom** adapter:

- File: `agent-intercom-claude-0.10.1-tmuxdeck.1.tgz`
- Size: `150164` bytes
- SHA-256: `a167218db5361a967fff15c750b53d82f567dc033c1691ba1265908db491ceb0`
- Package version: `0.10.1-tmuxdeck.1`
- Source fork commit: [`afcb3fe3f889c2baab784a15d2aecf7c5676c827`](https://github.com/ctliz/agent-intercom-claude/commit/afcb3fe3f889c2baab784a15d2aecf7c5676c827)
- Fork release/source offer: <https://github.com/ctliz/agent-intercom-claude/releases/tag/v0.10.1-tmuxdeck.1>
- Based on upstream: `@dataforxyz/agent-intercom-claude` `v0.10.0`
- Local maintenance change: package the Claude Monitor files required by `cci --tui`; no protocol or product feature changes.
- License: `AGPL-3.0-or-later`

The archive includes its original `LICENSE`, `THIRD_PARTY_NOTICES.md`, `COPYRIGHT`, `LICENSE_TRANSITION.md`, and third-party license files. TmuxDeck preserves those files in the per-user managed installation directory.

TmuxDeck verifies the pinned archive digest before extraction, rejects links, special files and unsafe paths, validates the Claude plugin/Monitor/MCP chain and JavaScript runtime, and installs it without reading from or writing to the user's global npm installation.
