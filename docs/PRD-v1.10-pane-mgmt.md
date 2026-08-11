# TmuxDeck v1.10 Pane-Level Management (Add/Delete Panes) PRD

> Goal: manage panes directly inside a card — add a pane, delete a specific pane.
> Background: after v1.9 slimmed the card header, the content area has room to operate; v1.7's add_pane already works in the tray, now it moves to cards plus delete support.
> Backend: reuse add_pane, add kill_pane.

---

## 1. Interaction shape (confirmed)

```
┌─────────────────────────────┐
│ [●] project-alpha      [✕]  │  ← ✕ = delete the whole session (existing, unchanged)
│ ┌──────┬──────┐             │
│ │ cmd  │ cmd  │×←appears on hover  │  ← each pane cell shows a small × on hover
│ ├──────┼──────┤             │
│ │ cmd  │ cmd  │             │
│ └──────┴──────┘             │
│ [+ Add pane]                │  ← small footer button, tiled re-layout
└─────────────────────────────┘
```

- **Delete**: hovering a pane preview cell reveals a small × → click → confirm dialog → delete that pane
- **Add**: small footer button on the card → new pane (default Shell, inherits directory) → tiled re-layout
- The top-right ✕ (delete session) **stays unchanged**; the two operate at different levels

---

## 2. Backend

### 2.1 Reuse `add_pane` (already exists from v1.7)

No changes needed. The logic is already correct: sanitize → take first pane's directory → split-window → tiled.

### 2.2 New `kill_pane(pane_id)`

```rust
#[tauri::command]
fn kill_pane(pane_id: String) -> Result<(), String> {
    // 1. validate pane_id format (tmux pane ids look like %1; only %\d+ allowed)
    // 2. run_tmux(&["kill-pane", "-t", &pane_id])
    // 3. return ERR_KILL_PANE_FAILED on failure
}
```

**pane_id validation**: tmux pane ids are `%number`; injection risk is low but the format must be validated
(`^%\d+$`) — you cannot reuse session-name sanitization (that's a different format).

**Note**: `kill_pane` only takes a pane_id, not a session — if the deleted pane is the last one
(the session would be destroyed along with it?), tmux behavior: killing the last pane
destroys the whole window/session. **The frontend must disable the delete button when only 1 pane remains** (see 3.3).

---

## 3. Frontend

### 3.1 Pane-cell hover delete

- Top-right of each pane preview cell, `group-hover` shows a small × (`opacity-0 group-hover:opacity-100`)
- Click → `confirm` (existing confirm pattern) → `invoke("kill_pane", { paneId })`
- After a successful delete, the 4s polling refreshes naturally (no manual refresh needed)
- i18n: reuse existing `card.destroy`? No — the semantics differ (that deletes a session).
  Add `card.killPane`: en "Kill this pane" / zh "删除此分屏"

### 3.2 Footer add button

- At the card footer (below or beside the open-icon row), a small button `[+ Pane]`
- Click → `invoke("add_pane", { sessionName })` (reuse)
- i18n: add `card.addPane`: en "Add pane" / zh "新增分屏"

### 3.3 Edge case: single-pane disabled

- When `session.panes_count <= 1`:
  - That pane's delete × is hidden (or disabled)
  - The footer add button still works (1 → 2 is a valid operation)
- Logic: `render the pane delete button only when panes_count > 1`

### 3.4 Layout

- Preview-cell hover ×: absolutely positioned top-right (`absolute top-1 right-1`); the cell needs `relative`
- Footer add button: `text-xs` small button, doesn't steal the show

---

## 4. Acceptance criteria

1. Multi-pane session: each pane cell shows × on hover; after confirm that pane is deleted and the grid re-lays out tiled
2. Single-pane session: no delete × (disabled); add still works (1→2)
3. Footer add button: clicking adds a pane (Shell), directory inherited, tiled re-layout
4. Delete-confirm copy distinguishes killPane from session destroy
5. After a pane delete, the list auto-refreshes within 4s (no manual refresh)
6. kill_pane's pane_id format validation works (invalid id errors, no panic)
7. macOS build + CI on both platforms pass
8. i18n three-way alignment (new keys in both languages)

---

## 5. Explicitly out of scope

- ❌ Pane drag reordering / resizing
- ❌ Pane renaming (tmux doesn't support pane names; skip)
- ❌ Undo delete (confirm is enough; no double-confirm/undo)
- ❌ Pane content migration (moving a pane to another session)
- ❌ Distinguishing "confirm running processes" on delete (confirm handles it uniformly)

---

## 6. Effort estimate

| Item | Estimate |
|---|---|
| Backend kill_pane + validation | 0.25 day |
| Frontend hover × + confirm + single-pane disable | 0.5 day |
| Footer add button + i18n | 0.25 day |
| **Total** | **about 1 person-day** |
