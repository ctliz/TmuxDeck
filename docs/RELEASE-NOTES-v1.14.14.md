## TmuxDeck v1.14.14 release notes

### Fix Chinese text in the embedded terminal

- Force the embedded tmux client to use UTF-8 output on macOS and Windows/WSL.
- Fix Chinese characters being rendered as underscores when TmuxDeck is launched without a UTF-8 locale, such as from Finder.
- Add regression coverage for the tmux UTF-8 attachment arguments and Ghostty Core Chinese cell rendering.
