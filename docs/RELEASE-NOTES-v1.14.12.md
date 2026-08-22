## TmuxDeck v1.14.12 release notes

### Smooth Terminal Scrolling & Precision Wheel Accumulator

- Added physical pixel wheel delta accumulator with `requestAnimationFrame` batching, delivering buttery smooth, precise trackpad scrolling without jumping or overshooting.
- Interactive scrollbar: draggable thumb, click-to-page navigation, and hover brightness feedback.
- Floating "Scroll to Bottom" (`↓ 回到最新`) quick action button when scrolled up into terminal history.

### Instant Canvas Text Selection & Clipboard Workflow

- Instant drag-to-select on terminal canvas with automatic system clipboard copy on mouse release.
- Double-click word selection and triple-click whole-line selection with copy toast indicator (`✓ 已复制 X 字符`).
- Tuned tmux copy-mode bindings (`copy-selection`) to preserve viewport scroll position upon mouse release without abruptly kicking the user back to the prompt.
- Keyboard shortcut `Cmd+C` / `Ctrl+C` copies active canvas selections without sending interrupt signals (SIGINT) to running CLI processes.

### Stability & Robustness

- Fixed terminal connection errors and augmented cross-platform PATH lookup for tmux binary and session queries.
- Official release builds published for macOS (`.dmg`) and Windows (`.exe` / `.msi`).
