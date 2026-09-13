## TmuxDeck v1.14.17 release notes

### Release pipeline

- macOS releases now fail if Apple signing or notarization secrets are missing. Unsigned DMGs are no longer uploaded.
- Windows `.exe` / `.msi` publishing is paused. GitHub Releases contain the notarized Apple Silicon DMG only.
- In-app updates remain Apple Silicon (`darwin-aarch64`) only.

### Desktop security

- Enable a restrictive Content Security Policy for the Tauri WebView instead of `csp: null`.
- Split tray-panel capabilities away from updater, process, and opener permissions used by the main window.
- Keep Hardened Runtime JIT entitlements needed by the embedded terminal; drop DYLD environment variables and library-validation disablement.

### Pairing transport

- Read the live pairing token at HTTP/WebSocket handshake time so rotation invalidates in-flight connections.
- Keep pairing tokens in mobile-page memory only; do not write them to `sessionStorage`.
- Reject public IP Host headers in tests and keep MagicDNS limited to Tailscale `*.ts.net` hostnames.

### Cleanup

- Remove the unused xterm `AgentTerminal` / `ChatCockpit` UI and the `@xterm/*` dependencies.
- Align README, roadmap, and signing docs with signed/notarized macOS shipping.
