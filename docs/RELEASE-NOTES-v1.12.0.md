## TmuxDeck v1.12.0 release candidate

This release candidate adds a pinned macOS Claude Intercom fallback, atomic batch pane creation, and a more capable trusted-LAN mobile conversation experience.

### Managed Claude Intercom on macOS

- TmuxDeck now bundles the pinned `agent-intercom-claude-0.10.1-tmuxdeck.1.tgz` maintenance artifact for optional offline installation. It does not download npm packages at runtime and never changes, replaces, or removes the user's global npm installation.
- Install and Repair verify the exact bundled SHA-256, reject links, device entries and unsafe archive paths, extract into staging, validate the Claude plugin → Monitor → runtime chain, and activate with rename/rollback semantics. An incomplete, modified, or conflicting installation fails closed instead of being treated as healthy.
- Managed Claude is launched by its verified absolute adapter path with `--safe`. Every newly created managed pane or native slot receives a fresh cryptographically random incarnation ID, persisted in tmux metadata for that pane/slot lifetime.
- **Use Standard Claude** is a persistent preference. Installing, repairing, or explicitly choosing Managed switches back to the managed adapter; existing global `cci` installations remain untouched.
- Bridge association remains process-tree based and additionally checks managed adapter metadata, expected incarnation ID, working directory, and agent type. Ambiguous or conflicting candidates are rejected, and full broker snapshots clear stale routes before rebuilding them.
- The random incarnation ID is routing consistency metadata, **not an authentication credential**. It must not be treated as proof of identity or authorization.

Artifact provenance, source offer, license, exact digest, and maintenance scope are recorded in [`src-tauri/resources/README.md`](../src-tauri/resources/README.md).

### Add several panes in one action

- The desktop pane menu offers counts **1, 2, and 4**, then makes one `add_panes` invocation instead of looping in the UI.
- The backend accepts counts from **1 through 6**. Standard tmux panes are created sequentially with their existing Agent and managed-adapter metadata, then tiled once.
- Batch creation is all-or-rollback: a failure removes every pane or native slot created by that request. A rollback failure is reported explicitly rather than hiding a residual partial batch.
- Ghostty native workspaces allocate consecutive slot numbers, create the full batch first, and rebuild the visible native layout only once. A failed rebuild removes the new slots and attempts to restore the prior layout.
- The original single-pane Tauri command remains available for tray and compatibility paths and reuses the same implementation with a count of one.

### Workspace-aware mobile conversations

- Conversations now carry backend-authoritative `workspaceId` and `workspaceName` fields. Native sessions use persisted workspace metadata rather than parsing the `__td_slot_` naming convention.
- The mobile list groups conversations by workspace, keeps disclosure controls even for a single workspace, promotes workspaces that need human attention, and preserves stable order among the remaining groups.
- The conversation header includes the workspace name in both visible text and its accessible label.

### Safer, smaller mobile conversation UI

- Agent messages support Markdown through pinned, inlined **marked 18.0.9**, followed by a strict **DOMPurify 3.4.13** sanitization pass.
- Raw HTML is rendered as literal text, not trusted markup. Markdown images render only their alt text, and DOMPurify remains the second defensive layer.
- The normal conversation view keeps three persistent actions: Back, More, and Send. Approved control keys live in More; Interrupt and EOF are visually separated as dangerous actions. Awaiting-human and offline states add only their relevant notice/action.
- Long-press context actions, long-content expansion, compact streaming headers, accessible workspace labels, and reconnect handling are included without restoring the previous global header inside a conversation.
- Capture-output labeling now uses the backend-authoritative `transcriptKind` field only. It does not infer transcript quality from Intercom presence.

The mobile page contains no runtime vendor fetches or duplicate vendor JavaScript files. Vendor versions, source integrity, license choice, and re-vendoring instructions are recorded in [`src-tauri/mobile/vendor/README.md`](../src-tauri/mobile/vendor/README.md); the adjacent files contain the required license texts.

### Verification

The frozen release-candidate tree passed:

- **40 frontend tests**, with no failures.
- **124 Rust tests passed and 2 environment/on-device tests ignored**, with no failures.
- TypeScript production build and `cargo check`.
- macOS Tauri application and DMG creation for Apple Silicon.
- `codesign --verify --deep --strict` for both the generated app and the app mounted read-only from the DMG.
- Exact Managed Claude artifact SHA-256 verification in the source tree, generated app, and mounted DMG:
  `a167218db5361a967fff15c750b53d82f567dc033c1691ba1265908db491ceb0`.
- Release-binary verification that the complete mobile HTML, marked 18.0.9, and DOMPurify 3.4.13 are embedded without external vendor requests.
- Existing browser audit evidence for normal, awaiting-human, offline, workspace grouping, Markdown, raw-HTML, and XSS cases.

These results do not imply that every path received live GUI acceptance. Managed/Standard switching is covered by static and automated tests, but the full **Managed → Standard → Managed** menu-click flow is not claimed as reliable live-GUI evidence. Batch pane creation and native batch rebuild passed implementation and automated tests, but were not exercised through a live Tauri invoke against a user workspace. Existing screenshots and browser audit artifacts are review evidence, not release artifacts.

### Known limitations and follow-ups

- The macOS bundle is ad-hoc signed and not notarized. Structural code-sign verification passes, but Gatekeeper assessment rejects it until a Developer ID signing and notarization flow is added.
- The Windows target was not installed or verified for this release candidate. Managed Claude Install/Repair remains macOS-only; Windows/WSL keeps Standard Claude behavior.
- Final physical-phone trusted-LAN acceptance is still pending, including firewall, reconnect, multi-conversation, and real-device interaction checks.
- Mobile access uses a mandatory pairing token on a **plaintext trusted local network**. It is not TLS and should not be exposed to an untrusted LAN or the public Internet.
- External push while the browser is suspended or closed remains out of scope.
- The bundled maintenance artifact fixes the missing Claude Monitor packaging needed by this release. Upstream Monitor warnings may still require follow-up; they are no longer a blocker for the pinned managed artifact.
- Protocol-v4 workspace-scoped discovery is not part of v1.12.0. Workspace grouping in this release uses TmuxDeck's authoritative pane/workspace metadata on the existing conversation transport.
