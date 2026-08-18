## TmuxDeck v1.14.10 release notes

### Mobile

- Pairing lists LAN and Tailscale addresses. Use the Tailscale QR when the phone is on the tailnet.
- Opening a workspace conversation defaults to the Lead session. Switch teammates from the compact header dropdown.
- Conversation snapshots keep the last 12 turns so long histories do not stall Safari.
- Safari keeps the composer above the keyboard instead of scrolling the input to the top of the screen.
- Refresh pairing mints a new token; closing the QR keeps the current token.

### Desktop

- The menu bar tooltip and tray panel show how many Agents are waiting for a reply.

### Packaging

- Destroying a missing native workspace returns “nothing to do” when tmux is absent, so CI/release tests no longer fail on `ENOENT`.
