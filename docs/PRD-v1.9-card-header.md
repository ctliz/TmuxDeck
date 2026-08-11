# TmuxDeck v1.9 Card Header Interaction Simplification PRD

> Goal: make the card header more minimal — remove the rename/delete buttons, make the name click-to-edit, keep only an × in the top-right.
> Pure frontend change; no backend logic touched.

---

## 1. Current state and goal

**Current (card header)**:
```
[●] project-alpha        [✏️][🗑]   ← hover-only rename/delete buttons
X windows · Y panes   last active X minutes ago    ← stats text line
```

**Goal**:
```
[●] project-alpha              [×]   ← name click-to-edit; only × in the top-right
```

---

## 2. Change list

### 2.1 Remove the two buttons (Edit2 / Trash2)

- Remove the hover-shown rename button (Edit2)
- Remove the hover-shown delete button (Trash2)
- Remove the `Edit2` / `Trash2` lucide imports (if no longer used)

### 2.2 Name click-to-edit

- Add a click handler to the name `<h2>` → enters inline edit (reuse the existing `isRenaming` / `renamedName` / `handleRename` logic)
- Interaction details:
  - click the name → becomes an input (existing edit UI kept)
  - Enter / blur submits (existing logic)
  - hovering the name gives an "editable" hint (e.g. `hover:underline` or cursor-text)
- Existing `sanitizeNameFrontend` (frontend name filtering) logic kept

### 2.3 Top-right × (delete entry)

- Position: top-right of the card header, replacing the old two buttons
- Behavior: click → `handleKill(session.name)` (existing delete-confirmation logic; confirm dialog kept)
- Style: small × icon, red on hover (the existing trash-hover semantics)
- **Always visible** (no longer hover-only) — user asked for "just the top-right corner"; nothing to hide
- tooltip: `card.destroy` (reuses the existing i18n key)

### 2.4 Remove the stats text line

- Remove the `{tPlural("card.windows", ...)} · {tPlural("card.panes", ...)}` part from the `<div>`
- **Note:** should the last-active time (`activityInfo.text`) stay?
  - The user said "remove the pane-stats text below" — referring to the "X windows · Y panes" line
  - The active time (v1.4's three-state copy) is valuable information — **keep it by default**, and handle it separately from the stats text: drop the stats, keep the active time
  - If the user wants even more minimalism it can go too — **the PRD keeps the active time by default** and drops the window/pane stats

### 2.5 Status dot kept

- Green dot = active (attached / active in background), gray dot = offline/idle — **existing three-state logic kept unchanged**
- Position still to the left of the name

---

## 3. Acceptance criteria

1. No Edit2/Trash2 buttons in the card header
2. Clicking the name → enters inline edit; Enter/blur submits; name filtering (sanitize) still works
3. Top-right × always visible; clicking pops the confirmation dialog (existing confirm logic); deletes after confirmation
4. Three-state dot (green/yellow/gray) kept with unchanged logic
5. "X windows · Y panes" text removed; last-active time kept
6. macOS build + CI dual-platform pass
7. i18n three-way alignment (should pass directly if no new keys)

---

## 4. Explicitly out of scope

- ❌ changing the delete-confirmation logic (confirm kept; no double-confirm/undo)
- ❌ changing the three-state dot logic (fixed in v1.4)
- ❌ name-overlong truncation interaction changes (truncate kept)
- ❌ drag ordering / card reordering

---

## 5. Effort estimate

| Item | Estimate |
|---|---|
| remove buttons + name click-to-edit + × + drop stats line | 0.5 day |
| verification | 0.5 day |
| **Total** | **about 0.5-1 person-day** |
