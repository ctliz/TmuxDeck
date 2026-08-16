## TmuxDeck v1.14.5 release notes

### Claude network environment parity

- Propagate proxy and CA-bundle settings from the user's login shell into TmuxDeck-launched agents.
- Preserve Claude authentication when TmuxDeck is launched from Finder/Tauri without the shell profile environment.
- Apply the same transport environment to Standard and Managed Claude.

### Terminal rendering

- Propagate `TERMINFO` and `TERMINFO_DIRS` from the login shell.
- Preserve `TERM=tmux-256color` while allowing Codex and other TUIs to resolve Ghostty's terminal capabilities.
- Keep the existing explicit RGB, focus-event, and extended-key tmux configuration.
