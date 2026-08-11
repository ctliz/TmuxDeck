# TmuxDeck v1.17 Drag-and-Drop Reordering: Swap Panes Within a Card + Reorder Cards PRD

> Goal: two levels of drag-and-drop — pane cells **within** a card can swap positions (really swapping the tmux layout),
> and whole cards can be reordered in the grid. **Panes may not be dragged onto another card**.
> Positioning: pure interaction enhancement; no change to the data model or backend semantics.

---

## 1. Interaction shape

```
┌─ card-A ──────────────┐    ┌─ card-B ──────────────┐
│ [pane1] [pane2]  ←drag→ │    │ [pane1] [pane2]        │
│ [pane3] [pane4]       │    │ [pane3] [pane4]        │
└───────────────────────┘    └───────────────────────┘
   ↕ whole-card drag reorder (within the grid)
```

- **Within a card**: drag a pane cell onto another cell of the same card → they swap (`swap-pane`; the tmux layout really swaps)
- **Whole card**: drag the card header / any blank area → reorder within the grid (pure frontend state)
- **Forbidden**: dragging a pane out of its card (visually, drop targets are limited to same-card cells; dragging outside a card does nothing)

## 2. Backend (tmux-backend)

### New `swap_pane(pane_id_a, pane_id_b) -> Result<(), String>`

```rust
#[tauri::command]
fn swap_pane(pane_id_a: String, pane_id_b: String) -> Result<(), String> {
    // 1. both ids pass validate_pane_id (reuse; format %\d+)
    // 2. run_tmux(&["swap-pane", "-s", &a, "-t", &b])
    // 3. on failure return ERR_SWAP_PANE_FAILED (including the is_no_server_err interception)
}
```

- tmux also supports swapping panes across sessions, but the **frontend forbids it**; the backend adds no extra restriction (minimal)
- i18n: add `ERR_SWAP_PANE_FAILED` in both languages

## 3. Frontend (tmux-front)

### 3.1 Dragging panes within a card

- Drag source: the pane cell (`draggable` / pointer events, **no new dependencies**, HTML5 DnD or hand-written)
- Drop target: **only other pane cells in the same card**; dragging outside the card / onto another card → no drop response (naturally forbids cross-card)
- Drop → `invoke("swap_pane", { paneIdA, paneIdB })` → on success `loadData()` (the 4s polling also refreshes naturally)
- Visuals while dragging: source cell semi-transparent, target cell highlighted
- Single-pane card: nothing draggable; no drag hint shown

### 3.2 Card-level reordering

- Drag source: the whole card
- Implementation: the frontend keeps `cardOrder: string[]` (order of session ids); when `loadData` merges, **reorder by cardOrder** so the 4s polling doesn't override the user's order; new sessions append at the end
- Persistence: **none** (back to default order after restart; a separate initiative if ever needed)

### 3.3 Boundaries

- Pane dragging doesn't conflict with the existing hover delete × or the rename input (drag only triggers on the pane cell's blank/command area)
- Click is disabled during a drag (avoid accidentally opening the session)

## 4. Acceptance

1. Dragging pane cell A to B within the same card → the two swap; the actual tmux layout swaps (`list-panes` order changes); order holds after the 4s refresh
2. Dragging a card to another position → grid reorders; order holds after the 4s polling
3. Dragging a pane over another card → no reaction at all (no swap, no error)
4. Single-pane card: no drag source
5. After adding/deleting panes: in-card order is correct; cardOrder appends new sessions at the end
6. npm run build + npm test + cargo test all green; CI on both platforms green

## 5. Explicitly out of scope

- ❌ Moving panes across cards (explicitly forbidden by the requirement)
- ❌ Persisting drag order to config (back to default on restart)
- ❌ Touch-device dragging (mobile v1.14 is a separate discussion)
- ❌ A drag library dependency (hand-written, minimal)

## 6. Effort estimate

| Item | Estimate |
|---|---|
| Backend swap_pane + validation + i18n | 0.25 day |
| Frontend pane drag + card reorder + cardOrder | 1 day |
| Verification | 0.5 day |
| **Total** | **about 1.5-2 person-days** |
