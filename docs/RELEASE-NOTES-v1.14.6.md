## TmuxDeck v1.14.6 release notes

### Managed Claude plugin repair

- Detect existing Managed Claude installations whose root is missing the `.claude-plugin`, `.mcp.json`, or Monitor surface required by `--plugin-dir`.
- Stop accepting a healthy nested npm package as a substitute for the runtime plugin root.
- Mark stale installations for Repair instead of starting Claude without the Intercom MCP tools.
