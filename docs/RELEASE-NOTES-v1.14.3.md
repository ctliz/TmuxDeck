## TmuxDeck v1.14.3 release notes

TmuxDeck v1.14.3 is a reliability patch for macOS GUI launches and managed Agent Intercom adapters.

### Fixed

- Discover Claude, Codex, OpenCode, Pi, npm, and Node installations when Tauri is launched from Finder or another GUI context with a minimal `PATH`.
- Propagate the augmented executable `PATH` to managed adapter staging commands while preserving explicit command environments.
- Validate the npm-packaged Claude plugin layout from its installed package root.
- Correct Claude managed-root harness identity validation and invalidate adapter/environment health caches after successful installation.
- Keep managed Codex MCP configuration pointed directly at the bundled `codex-server.mjs` entrypoint.

### UI and documentation

- Preserve the panel permission-bypass safety control and terminal capability behavior.
- Improve the workspace creation modal sizing and scrolling behavior.
- Update contributor and cross-harness documentation for the Core 0.2.0 team-manifest contract.

### Verification

- Frontend tests: 47 passed.
- Cargo tests: 189 passed, 0 failed, 2 ignored.
- Production build and `git diff --check` passed.
