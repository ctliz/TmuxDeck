## TmuxDeck v1.14.9 release notes

### Grok Build and AGY

- Detect Grok Build (`grok`, including `~/.grok/bin`) and AGY (`agy`) as available workspace agents.
- Start Grok and AGY with their documented bypass flags when bypass mode is selected.
- Pass each TmuxDeck pane's generated Intercom identity to Grok and AGY plugin hosts.
- Document the external/manual `claude-intercom-mcp` plugin setup, per-pane identity requirements, and the need to poll `intercom_pending` because these MCP integrations have no wake bridge.

### Release platforms

- This release ships the Apple Silicon macOS DMG only. Windows installers remain paused.
