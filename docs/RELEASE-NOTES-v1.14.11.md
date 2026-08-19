## TmuxDeck v1.14.11 release notes

### Native Canvas Terminal (Ghostty VT)

- Embedded native `libghostty-vt` terminal core with high-performance 2D Canvas rendering and dirty-row partial patching.
- Multi-pass rendering pipeline ensuring flawless CJK full-width glyph alignment and crisp Retina display scaling.
- Integrated background translucency with frosted glassmorphism styling (`backdrop-blur-2xl`).
- Native mouse reporting, scrollback history wheel support, and bracketed paste protection.

### Workspace & Cockpit Layouts

- Three versatile layout modes:
  - **Focus**: Distraction-free single-pane terminal with tabbed switcher and drag-to-reorder tabs.
  - **List**: Sidebar conversation navigator displaying all agent panes with status badges and quick jumping.
  - **Grid**: Multi-pane synchronized grid view with titlebar drag-and-drop swapping.
- Streamlined tab creation: Add new split panes/tabs directly from the canvas header with intelligent dominant agent recommendation.
- Isolated in-app pane creation: zero extraneous external terminal popups.
- Fixed initial input buffer newline quirks to guarantee instant `/` slash-command autocomplete on launch.

### Cross-Platform Release

- Official builds for both macOS (`.dmg`) and Windows (`.exe` / `.msi`).
