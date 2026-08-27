## TmuxDeck v1.14.15 release notes

### Smoother dashboard scrolling

- Remove per-card backdrop filters and the fixed background attachment that caused expensive WebKit repaints while scrolling.
- Keep the dashboard's glass appearance with translucent surfaces, borders, and shadows that are cheaper to composite.
- Reuse unchanged workspace and pane objects so periodic tmux polling no longer rerenders the entire dashboard.
- Defer nonessential pane preview capture and non-forced dashboard updates while the user is actively scrolling.

### Restore native terminal paste

- Let macOS `Command+V` and the platform paste shortcut reach the hidden terminal input instead of encoding them as terminal modifier keys.
- Continue routing pasted text through the terminal paste command, including bracketed-paste handling when enabled by the Agent CLI.
