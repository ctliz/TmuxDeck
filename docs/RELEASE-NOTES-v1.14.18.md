## TmuxDeck v1.14.18 release notes

### Agent-intercom plugins

- Bump bundled agent-intercom plugins: Claude `0.16.0`, Codex `0.13.3`, OpenCode `0.13.0`; OpenCode plugin / SDK closure `1.19.3`. Core manifest stays at `0.2.0`.
- Track every bundled plugin version and digest in `src-tauri/resources/plugins.lock.json` instead of hand-editing several files.
- Add `scripts/update-plugins` to pull the latest npm releases and rewrite versions and checksums; `--check` only reports available updates. CI runs the check.
- Track pi-intercom at `0.14.0` in the update check and docs.

### Release pipeline

- Release builds now run `npm test`, `npm run build`, and `cargo test` before signing; any failure stops the release.

### Desktop security

- Write tmux attach scripts to a private per-user app directory with random file names, created exclusively without following symlinks, instead of predictable `/tmp/tmuxdeck-<session>.sh` paths.
- Escape special characters in session names inside attach scripts and prune scripts older than 7 days.

### Cleanup

- Rename the npm / Cargo package from `tauri-app` to `tmuxdeck`. The bundle identifier `com.ctliz.tmuxdeck` and updater configuration are unchanged; only the executable inside `TmuxDeck.app` is renamed.
