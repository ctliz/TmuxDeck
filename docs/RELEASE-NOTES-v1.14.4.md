## TmuxDeck v1.14.4 release notes

TmuxDeck v1.14.4 fixes managed Claude Code startup when the adapter is installed with the npm package layout.

### Fixed

- Materialize the trusted Claude plugin surface at the managed root used by `cci --plugin-dir`.
- Preserve `.claude-plugin/plugin.json`, `.mcp.json`, monitors, commands, and skills for the managed Claude runtime.
- Reject symlinked plugin files during materialization.
- Keep the npm package layout, managed runtime files, and plugin validation paths consistent.

### Verification

- Frontend tests: 47 passed.
- Cargo tests: 191 passed, 0 failed, 2 ignored.
- Offline Claude/Codex/OpenCode staging smoke tests passed.
- Production build and `git diff --check` passed.
