# Vendored terminal core

- `libghostty-vt/` comes from `ghostty-org/ghostty` commit `c5a21edfcbc2d5b46540ad91b7980aca31f5f1f3` and is MIT licensed (`libghostty-vt/LICENSE`).
- `libghostty-vt/pkg/uucode/` is vendored from `jacobsandlund/uucode` 0.2.0 because Zig package downloads can fail behind some proxies; its license is in `libghostty-vt/pkg/uucode/LICENSE.md`.
- `src/ghostty_vt/` is adapted from `herdrdev/herdr` commit `a5c69beabfc82d9c3f9563eb821139b2e0f3e14f`, Apache-2.0 licensed (`HERDR-LICENSE-APACHE-2.0`).

The initial macOS arm64 spike includes a pinned prebuilt static library under `libghostty-vt/prebuilt/aarch64-apple-darwin/`. Other targets build from source and require Zig 0.15.2. Set `ZIG=/path/to/zig` when it is not on `PATH`; Homebrew's `zig@0.15` path is detected automatically.
