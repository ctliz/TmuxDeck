# TmuxDeck v1.6 Liquid Glass UI Rework PRD

> Goal: move from "heavy dark tech style" to "macOS Liquid Glass minimal style":
> remove the entire top area, make the search box semi-transparent and centered, and turn the new-workspace entry into the first dashed card in the card list.
> Pure frontend rework; zero backend changes.

---

## 1. Background and problem

The current UI is "heavy dark tech style":
- A big header at the top (logo + title + environment indicator + refresh + new button)
- A search/stats bar beneath it
- Two stacked bars eat a lot of vertical space; visually heavy

User feedback: not clean enough, looks too heavy.

---

## 2. Target design (Liquid Glass)

Overall tone: **frosted glass (backdrop-blur) + semi-transparency + fades + smooth transitions**, referencing macOS Tahoe (26)'s Liquid Glass design language.

### 2.1 Top area: remove all of it

Delete:
- header (logo / title / version / subtitle)
- environment status indicator (tmux/terminals/agents counts)
- refresh button
- new button (moves to the card list, see 2.3)
- search/stats bar

**Keep the function, not the form:**
- Refresh → still 4s polling + manual pull-to-refresh? **v1.6 drops the manual refresh** (auto-polling is enough; minimal), or put a tiny refresh button to the right of the search box (optional, see 2.2)
- Environment indicator → appears only on the hard-block guide page when tmux is missing (already exists); not shown in the normal state
- Stats → kept in the search box placeholder or tooltip; doesn't occupy permanent space

### 2.2 Search box: semi-transparent, centered

```
┌──────────────────────────────────────────┐
│                    🔍                    │   ← centered, narrow, semi-transparent
└──────────────────────────────────────────┘
```

- Fixed width ~240-320px, vertically centered at the top of the content area
- Style: `backdrop-blur-xl bg-white/10 border border-white/15 rounded-full`
- No label, no outer frame; just a floating pill input
- Slight scale-up + highlight on focus (transition)
- placeholder: `t("search.placeholder")`

### 2.3 New-workspace entry: first card in the list

The first card in the card grid is the "New workspace" entry:

```
┌─ ─ ─ ─ ─ ─ ─ ┐   ┌──────────────┐   ┌──────────────┐
│      +        │   │  project-a   │   │  project-b   │
│   New workspace│   │  ...         │   │  ...         │
└─ ─ ─ ─ ─ ─ ─ ┘   └──────────────┘   └──────────────┘
   dashed border + plus   real card         real card
```

- Style: `border-2 border-dashed border-white/20` + semi-transparent background + large plus icon
- hover: border brightens + faded arrow/text hint
- Click → opens the existing "New workspace" Modal (no new form)
- **Always first** (even when cards exist); excluded from search filtering
- **Empty-state handling:** when `filteredSessions.length === 0`, **don't show the full-screen empty state**; show only the "new card" + one small hint line (`empty.hint`). The old "create now" button logic merges into the new card.

### 2.4 Card visual upgrade (liquid glass)

The existing card goes from `bg-slate-900/80 border-slate-800` to:

```css
bg-white/10 backdrop-blur-xl border border-white/15 rounded-2xl
shadow-lg shadow-black/5 hover:shadow-xl hover:bg-white/15
transition-all duration-300
```

- Three-state dot, live preview, agent-name highlight, etc. — **all logic preserved, only the skin changes**
- Background: global change from `bg-slate-950` to **gradient + dark frosted-glass base**:
  `bg-gradient-to-br from-slate-900 via-slate-950 to-indigo-950/60`
  (the dark base is what gives the white translucent glass its contrast)

### 2.5 Transitions and animations

- Card enter: `animate-fade-in-up` (fade in + move up 8px, 300ms ease-out)
- Card delete/refresh: transition animation (`transition-opacity`)
- Search filtering: cards fade out/in rather than vanishing instantly
- Modal: existing `backdrop-blur-sm` upgraded to Liquid Glass style + fade-in scale
- All hover/active states: `transition-all duration-200`

New CSS (index.css):

```css
@keyframes fade-in-up {
  from { opacity: 0; transform: translateY(8px); }
  to   { opacity: 1; transform: translateY(0); }
}
```

---

## 3. Component structure changes

| Current | After rework |
|---|---|
| `<header>` (logo/title/environment/buttons) | **deleted** |
| search+stats bar `<div>` | standalone centered search box + stats into placeholder/tooltip |
| card grid | dashed "new" card inserted first |
| card style | liquid glass |

**i18n:** new key:
- `search.hint` (optional, tooltip shows stats, e.g. en: "3 workspaces · 1 running")

---

## 4. Acceptance criteria

1. **No** persistent bars at the top (header/search bar); content starts at the viewport top
2. Search box semi-transparent, rounded, centered; focus has a transition animation
3. First card in the grid is the dashed "New workspace" card; clicking opens the existing Modal
4. Cards and Modal all use the liquid-glass style (semi-transparent + frosted + thin border)
5. Cards have a fade-in animation; search filtering has a transition (not instant disappearance)
6. Zero functional regression: create/open/delete/rename/live-preview/three-state all work
7. macOS build + CI dual-platform pass
8. i18n three-way alignment script passes (new keys bilingual complete)

---

## 5. Explicitly out of scope

- ❌ changing any backend logic / registry / commands
- ❌ dark/light theme toggle (v1.6 does dark liquid glass only)
- ❌ custom theme colors / background images
- ❌ window transparency passthrough (Tauri transparent windows are a separate engineering effort: tauri.conf.json + platform config; separate initiative)
- ❌ sidebar / navigation rework
- ❌ mobile responsive layout adjustments (desktop app; keep desktop-first)
- ❌ manual refresh button (auto 4s polling is enough) — if acceptance shows it's truly needed, add a tiny one beside the search box

---

## 6. Effort estimate

| Item | Estimate |
|---|---|
| delete header/search bar + rework layout | 0.5 day |
| liquid-glass card styles + new card | 0.5 day |
| animations/transitions + Modal skin | 0.5 day |
| **Total** | **about 1.5 person-days** |
