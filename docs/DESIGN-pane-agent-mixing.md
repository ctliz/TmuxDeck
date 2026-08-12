# Per-pane Agent selection: backend contract

TmuxDeck can create one workspace whose panes run different detected Agents while preserving the legacy single-Agent API.

## Creation contract

`CreateOpts` keeps the existing `agent_id` and `panes` fields and adds:

```json
{ "pane_agent_ids": ["pi", "claude", "codex", "shell"] }
```

- Missing or empty: every pane uses `agent_id` (legacy behavior).
- Non-empty: its length must equal the normalized pane count and order maps to pane/native slot 1..N.
- Every ID must exist in `detect_environment().agents`; otherwise creation returns `ERR_AGENT_NOT_FOUND|<id>`.
- A mismatched list returns `ERR_PANE_AGENT_COUNT|<expected>|<actual>`.
- Claude keeps its same-kind runtime behavior: verified `cci --tui` with per-pane/slot identity, otherwise ordinary `claude`.
- Custom commands are selected only by the `custom` ID and are not rewritten.

`add_pane(session_name, agent_id?)` accepts an optional Agent ID. Missing means `shell`, preserving tray and old caller behavior. A supplied unknown/uninstalled ID is rejected.

## Persistent metadata

Each created tmux pane/session stores `@tmuxdeck-agent=<agent-id>` as a pane option. Native slot sessions store the same option. `get_tmux_sessions` and `list_panes` expose it as optional `agent_id`; the conversation registry prefers this fact over `pane_current_command` when determining `AgentKind`.
