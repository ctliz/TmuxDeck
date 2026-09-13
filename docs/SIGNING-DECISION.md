# Code signing decision (macOS and Windows)

## Status

- **macOS.** Official releases are Developer ID signed, notarized, and stapled. GitHub Actions refuses to upload a DMG unless signing and notarization secrets are present and notarization succeeds.
- **Windows.** Installers are not published. The release workflow no longer builds `.exe` / `.msi`. Windows/WSL still compiles in CI so that code does not rot.

## Auto-update

In-app updates (Tauri updater + Minisign) are published for **Apple Silicon** (`darwin-aarch64`) only. Intel Macs can still be built from source. Windows has no updater channel while installers are paused.

## Why Windows stays unsigned / unpublished

Azure Trusted Signing is a paid, per-use service with no free tier, and it requires binding a credit card to an Azure subscription. That remains a business decision, not a technical one.

## Related

- Release notes for v1.14.16 landed signing and notarization.
- v1.14.17 makes missing Apple secrets a hard release failure, and pauses Windows assets.
