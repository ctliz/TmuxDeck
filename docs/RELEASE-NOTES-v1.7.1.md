## TmuxDeck v1.7.1

A patch release fixing a critical bug: workspace creation failed on all released builds since v1.1.

### Fix

- **Workspace creation works again.** The create dialog sent `agentId` / `terminalId` while the backend expected `agent_id` / `terminal_id`, so `create_session` failed with `missing field 'agent_id'` and no workspace could be created. Field names are now aligned to snake_case, matching every other frontend/backend structure.

### Install

- macOS: download the `.dmg`, drag into Applications. If Gatekeeper warns on first launch, right-click -> Open (unsigned build).
- Windows: download the `.exe` (NSIS) or `.msi`.
