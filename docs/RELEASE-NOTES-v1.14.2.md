## TmuxDeck v1.14.2 release notes

TmuxDeck v1.14.2 is a hotfix release fixing Codex MCP server configuration and handshake initialization, adding the comprehensive Connector/Adapter Contribution Guide, and verifying offline protocol integration across supported agents.

---

### Codex MCP Server Configuration & Protocol Handshake Fix

- **Direct MCP Server Entrypoint**: Aligned Codex configuration in `~/.codex/config.toml` to launch `node <managed_root>/dist/codex-server.mjs` directly with `startup_timeout_sec = 120`, avoiding interactive CLI launcher confusion.
- **Probe Identity Recognition**: Updated `probe_codex_config_toml` to recognize `node` + `dist/codex-server.mjs` as valid target configurations.
- **MCP Protocol Handshake Verification**: Added automated integration smoke tests verifying JSON-RPC `initialize` handshake execution on stdio.
- **Cross-Platform Path Resolution**: Ensured proper resolution across Apple Silicon (`/opt/homebrew/bin/`) and Intel macOS (`/usr/local/bin/`).

---

### Connector & Adapter Contribution Guide

- **Comprehensive Guidelines**: Added `Contributing a Communication Connector or Adapter` in `CONTRIBUTING.md` covering ecosystem adapters (Pi, Claude, Codex, OpenCode, Orchestrator, and future Agy).
- **Core 0.2.0 Team Manifest**: Standardized specifications for `AGENT_INTERCOM_TEAM_MANIFEST` JSON Schema, Lead/Worker topology, and environment injections.
- **Subresource Integrity & Provenance**: Mandated canonical GitHub source, exact tag/commit, and bidirectional SHA-256 / SHA-512 integrity verification.
- **PR & Issue Checklist**: Standardized contribution template with security review requirements.

---

### Verification & Test Coverage

- **190 Rust unit & integration tests** passing cleanly (including MCP initialize handshake smoke).
- **47 frontend tests** passing cleanly.
- **Production builds & bundle checks** verified.
