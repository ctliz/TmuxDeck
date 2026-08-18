## TmuxDeck v1.14.8 release notes

### Pi Intercom

- Recognize `@ctliz/pi-intercom@0.12.1` as a healthy existing install. Do not add the older git source on top of it.
- Treat npm plus git Intercom entries as Repair. Repair keeps the npm package and removes the duplicate.
- Two copies register the same tools (`intercom_send`, `intercom_ask`, …) and Pi exits immediately when opened from a pane.

### OpenCode

- Stale host plugin paths that still name the current managed version are Repair, not manual migration.
- The consent dialog includes copyable CLI fallbacks and Recheck adapters.

### Join copy

- A standalone Pi can join an existing TmuxDeck workspace circle with `/intercom-join`. This does not enroll it as a Team Worker.
