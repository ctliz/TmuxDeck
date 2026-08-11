# TmuxDeck v1.8 Terminal Icons Quick-Open PRD

> Goal: change the card bottom from an "Open (Ghostty) button + dropdown" to **a row of terminal brand icons** — one real icon per installed terminal; click to open the session with that terminal.
> Principle: icons must be real (brand logos), not generic lucide icons.

---

## 1. Background and problem

Today the card bottom has a single "Open (Ghostty)" button; with multiple terminals, a dropdown appears on hover.
Problems:
- Switching terminals takes two clicks (open dropdown + select)
- The button occupies a whole row; visually heavy
- You always have to guess "which one is currently selected"

Goal: a row of terminal icons — **icon as recognition, click to open**.

---

## 2. Icon source (real brand icons)

### 2.1 Option A (primary): extract icns from installed apps (runtime)

Every macOS app bundle has a real icon:
```
/Applications/Ghostty.app/Contents/Resources/icon.icns
/Applications/iTerm.app/Contents/Resources/AppIcon.icns
/Applications/kitty.app/Contents/Resources/AppIcon.icns
...
```

**Backend changes:**
- `detect_environment()`'s `ToolInfo` gains an `icon_path: Option<String>` field
- When an installed terminal is detected, also locate its icns path. **Note: filenames differ per terminal and aren't fixed** (Ghostty is actually `Ghostty.icns`, not `icon.icns`), so **don't hardcode** — scan all `.icns` files in the `Resources/` directory and take the first (or one matching `AppIcon`/`icon` keywords):
  ```rust
  fn find_app_icon(app_path: &Path) -> Option<String> {
      let res = app_path.join("Contents/Resources");
      std::fs::read_dir(res).ok()?.flatten()
          .find(|e| e.path().extension().map(|x| x == "icns").unwrap_or(false))
          .map(|e| e.path().to_string_lossy().to_string())
  }
  ```
- When no icns is found, `icon_path = None` (frontend falls back to bundled resources)

**Frontend rendering of icns:** the Tauri frontend can't load `.icns` via a plain `<img>` (browsers don't support that format).
**Conversion must happen in the backend:** new command `get_terminal_icon(terminal_id) -> Vec<u8>`, internally using `iconutil` or `sips` to convert icns → PNG:
```sh
sips -s format png icon.icns --out /tmp/tmuxdeck-icon.png   # built into macOS
```
Returns PNG bytes; the frontend converts to base64 for display.

### 2.2 Option B (fallback): bundled brand icon resources

Pack each terminal's official logo (SVG/PNG) into the project's `public/terminal-icons/`, independent of the local machine.
Source: the projects' GitHub repos (Ghostty's logo.svg, kitty's logo, etc.).
**Used when:** a terminal is detected but its icns path isn't found / Option A's conversion fails.

> A primary, B fallback. A guarantees "the real local icon"; B is the safety net.

---

## 3. Frontend changes

### 3.1 Card bottom: icon row

```
┌──────────────────────────────────┐
│  🖥  ▶   ⬛   ⬛      ← installed terminal icon row │
│  (default terminal highlighted border, hover scales) │
└──────────────────────────────────┘
```

- One small icon per installed terminal (20-24px rounded)
- **Default terminal** (config's default_terminal): highlighted (border or background); the rest transparent
- Click icon → `open_session(session.name, term.id)`
- When a row can't fit everything (>6), scroll or collapse into a "more" button (minimal icon row first; v1.8 treats 6 as the max)

### 3.2 Remove old UI

- Remove the "Open (Ghostty)" big button
- Remove the dropdown (`activeTerminalDropdown` + `ChevronDown` + menu div)
- Remove the `Play` icon dependency (if no longer used)

### 3.3 Interaction

- hover: icon scales up slightly (`scale-110 transition`)
- click: opens immediately + tooltip shows the terminal name
- the default terminal icon gets a "●" dot or border to mark it

---

## 4. New / changed interfaces

| Command | Change |
|---|---|
| `detect_environment` | `ToolInfo` gains `icon_path: Option<String>` |
| `get_terminal_icon(terminal_id) -> Vec<u8>` | new: icns → PNG bytes |

---

## 5. Acceptance criteria

1. With Ghostty installed locally: the card bottom shows Ghostty's real icon (extracted from the .app, not a generic icon)
2. With multiple terminals: the icon row shows each one's matching brand logo, never mixed up
3. Clicking any icon opens the session with that terminal
4. The default terminal icon has a highlight marker
5. When icns isn't found, fall back to bundled resources; no blank/broken icon
6. Terminal.app's icon also extracts and displays correctly
7. No residue of the "Open" big button or dropdown
8. macOS build + CI dual-platform pass
9. i18n three-way alignment (new copy bilingual)

---

## 6. Explicitly out of scope

- ❌ icon hover expansion to a large image / animation
- ❌ Windows terminal icons (macOS only this release; Windows is a different shape, and the user has paused the Windows side)
- ❌ custom icons (user upload/swapping)
- ❌ icon ordering settings (registry order is fine)
- ❌ showing icons for uninstalled terminals (only installed ones show, consistent with the product's "don't show invalid options" principle)

---

## 7. Effort estimate

| Item | Estimate |
|---|---|
| backend: icon_path + icns→PNG command | 0.5 day |
| frontend: icon row + delete old UI | 0.5 day |
| bundled fallback icons + verification | 0.5 day |
| **Total** | **about 1.5 person-days** |
