## TmuxDeck v1.14.1 release notes

TmuxDeck v1.14.1 is a patch release improving terminal capabilities inside Tmux panes and optimizing Tray panel layout hierarchy.

---

### Terminal Capability Enhancements

- **Truecolor & Capability Environs**: Pane startup commands and window splits now inject `COLORTERM=truecolor` and inherit `TERM_PROGRAM` (with fallback to the active terminal id).
- **Session Terminal Configuration**: Every standard and native workspace session configures `focus-events on`, `extended-keys on`, `default-terminal "tmux-256color"`, and `terminal-overrides ",*:RGB"` to ensure full color, event, and modifier key support across modern terminal emulators (Ghostty, Kitty, iTerm2, WezTerm, Alacritty, VS Code).
- **Native & Standard Split Parity**: Consistent environment injection across single-session multi-pane splits, native window slots, and dynamic pane batch additions.

---

### Tray Panel Layout Optimization

- **Workspace Priority Hierarchy**: In `TrayPanel`, `SessionList` is rendered before `UsageStrip`, ensuring that active workspace navigation and session quick-actions appear immediately above resource utilization stats.

---

### Ecosystem Provenance & Protocol v4 Verification

- **Full Six-Package Provenance Alignment**: Verified online registry integrity and subresource checksums across Core `0.2.0` and downstream adapters (`@ctliz/agent-intercom-pi@0.12.0-connect.1`, `@ctliz/agent-intercom-claude@0.13.0-connect.1`, `@ctliz/agent-intercom-codex@0.12.0-connect.1`, `@ctliz/agent-intercom-opencode@0.12.0-connect.1`, `@ctliz/agent-intercom-orchestrator@0.12.0-connect.1`).

---

### Verification & Test Coverage

- **185 Rust unit & integration tests** passing cleanly.
- **46 frontend tests** validating tray component order, modal workflows, zero scope leakage, and bilingual translations (`en` / `zh`).
- **Production builds & bundle checks** verified.
