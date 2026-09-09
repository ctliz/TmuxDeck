## TmuxDeck v1.14.16 release notes

### Apple Developer ID code signing & notarization

- Official macOS code signing using Apple Developer ID certificate with Hardened Runtime enabled.
- Automatic Apple notarization and ticket stapling for macOS DMG bundles to ensure trusted, warning-free installation.
- Configured runtime entitlements supporting JIT and executable memory while maintaining unsandboxed tmux inter-process access.

### Automatic in-app updates

- Integrated Tauri v2 auto-updater plugin with cryptographic Minisign (Ed25519) signature verification.
- Release manifests and signed update archives published automatically alongside GitHub Releases.
- Lightweight in-app update notification in the header providing download progress tracking and seamless restart to apply updates.

### Window interaction enhancements

- Added `core:window:allow-start-dragging` capability for native window dragging interactions.
