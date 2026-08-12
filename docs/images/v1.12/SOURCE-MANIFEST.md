# v1.12 README media source manifest

All published images in this directory are screenshots of the real TmuxDeck UI.
No generative image tools, reconstructed controls, fake device frames, or
painted-over product surfaces were used.

## Live isolated product captures

The following compositions are derived from a dedicated promo fixture running
the final v1.12 desktop bundle and the bundle's live trusted-LAN mobile page:

- `desktop-hero-{en,zh}.webp`
- `desktop-workspace-actions-{en,zh}.webp`
- the Claude portion of `desktop-claude-tray-{en,zh}.webp`
- `mobile-workspaces-{en,zh}.webp`
- `mobile-chat-{en,zh}.webp`

The isolated fixture contained only the generic workspaces `Launch-Control`,
`Release-Studio`, and `Research-Lab`. Create Workspace used the generic values
`orbit-api` and `/workspace/orbit-api`. The mobile Markdown and awaiting-human
copy was injected into isolated pseudo-terminals by the fixture owner. English
and Simplified Chinese captures used the same workspace order, Agent mapping,
status state, viewport, and expanded/collapsed state.

Desktop windows were captured by exact CGWindow id. Mobile captures used an
isolated Chrome profile at a fixed 390 x 844 CSS viewport and DPR 2. Pairing
URLs and tokens were read from a mode-600 fixture file and injected through the
Chrome DevTools protocol; they were never placed on a command line or rendered
in a published image.

A fixture guard verified before and after every UI action that protected user
config/audit/cache files and default tmux state were unchanged, and that the
isolated workspace/pane identities and order remained intact.

## Tray: approved visual-only harness fallback

The Tray portion of `desktop-claude-tray-{en,zh}.webp` is a **visual-only
harness fallback**, approved during capture review. The exact PID-owned live
menu-bar item passed ownership checks and AXPress, but macOS did not expose a
PID-owned Tray panel CGWindow afterward. A second live interaction was rejected
on safety grounds.

The fallback imported the final repository components directly:

- `src/tray/TrayPanel.tsx`
- `src/tray/SessionList.tsx`
- `src/tray/UsageStrip.tsx`
- `src/tray/tray.css`
- the shared theme and i18n modules

Only Tauri's `invoke` and `listen` boundaries were aliased to deterministic,
generic fixture responses. No Tray markup or styling was copied or redrawn.
The harness itself lived outside the repository and is not shipped.

## Processing

Post-processing was limited to cropping, downsampling, composing real captures
on a neutral background, converting to sRGB WebP, and stripping metadata.
Published assets contain no real usernames, user paths, URLs, IP addresses,
tokens, pane ids, contact ids, or private messages.
