# v1.14 Transport + security design: how the phone connects and why it trusts us

> PRD v1.12 defined the mobile protocol (event/command JSON), but **transport and security were left blank**. This fills them in: WebSocket server selection, listening surface, authentication, abuse prevention. Principles follow DECISIONS-v1.12: **don't invent your own security mechanisms**; if an existing trust boundary works, use it instead of building a wheel.

---

## 1. Transport selection

### 1.1 WebSocket server

- **Library:** `tokio-tungstenite` (Tauri 2 already ships a tokio runtime; no heavy new dependency).
- **Port:** dynamically allocated (`127.0.0.1:0` grabs a free port), **avoids a fixed port that can be occupied or scanned**.
- **Listen:** bound to `127.0.0.1` only by default (local access from the desktop); phone access goes through the §2 Tailscale tunnel, so the server **is never exposed as plaintext on the LAN**.
- **Path:** `ws://127.0.0.1:<port>/v1/ws`, subprotocol fixed at `tmuxdeck.v1`.
- **Lifecycle:** a background task started in Tauri `setup`; it keeps running with no connected clients (the conversation-table refresh still consumes intercom events); the phone client owns reconnection.

### 1.2 How the phone connects (key decision)

**No `0.0.0.0` binding, no LAN plaintext HTTP** — DECISIONS-v1.12 item 1 already rejected that (token sniffable on the same subnet, poor iOS self-signed-cert experience). Instead:

```
phone ── Tailscale (WireGuard, end-to-end encrypted) ──> Mac's tailnet IP:port
        │                                               │
        └── ws://100.x.y.z:<port>/v1/ws?token=… └── server binds only 127.0.0.1
```

- The phone runs Tailscale (free on the App Store) and, once on the same tailnet, reaches the Mac's `100.x.y.z` directly.
- **The WireGuard tunnel itself provides confidentiality** (E2E encryption) — that's the existing trust boundary; we don't invent a TLS/certificate scheme.
- `ws://` + a plaintext HTTP page is safe inside the tailnet; serving the page over HTTPS (Tailscale's MagicDNS name `mac-name.tailnet-name.ts.net`) gets a CA certificate for free, further blocking man-in-the-middle within the tailnet (other devices on the same tailnet).

**Security model:** Tailscale handles "who is on the network", the token handles "who this phone client is", and the two stack. Section 3 details this.

### 1.3 Phone UI shape

- v1.14 uses a **static SPA** (a single HTML+JS, served from the same server's `/v1/`):
  - QR-code pairing: the desktop shows a QR code (containing `ws://…?token=…` or the HTTPS URL)
  - Multi-conversation view: conversation list (`conversations` event) + per-conversation message stream (`turn` event) + input box (`say`) + control keys (`key`) + forwarding (`forward`)
  - Web notification/sound only on `awaiting-human` (the single push signal, PRD §3)
- No framework, no PWA install flow; get the conversation experience working first, a native app is a later option.

---

## 2. Listening surface (attack surface list)

| Surface | Design | Rationale |
|---|---|---|
| Port | dynamically allocated, bound to loopback/Tailscale IP only | no fixed port to scan; never on the LAN |
| HTTP service | same port, same process as WS | one surface to manage; only reachable via tailnet |
| Phone entry | Tailscale tunnel, not the public internet | reuses existing identity and encryption; nothing self-built |
| DNS rebinding | WS handshake validates the `Host` header | see §4.2 |
| Plaintext | `ws://` only inside the tailnet; `https://` via MagicDNS | no listening surface outside the tailnet |

---

## 3. Authentication: pairing token

### 3.1 Token generation and delivery

- Generated at every app launch as a **32-byte CSPRNG token** (`OsRng`), **never written to disk**.
- Shown in the desktop UI as a QR code + copyable text; the phone scans it and connects with the token included.
- The token exists only in: desktop memory, the QR code/clipboard, phone memory. It expires when the app exits — **no persistence, no revocation list** (no persistent credential means no revocation problem).

### 3.2 Handshake and validation

- Connection URL: `ws://host:port/v1/ws?token=<hex>`.
- On the server side, at handshake time:
  1. validate the subprotocol = `tmuxdeck.v1`;
  2. validate the `Host` header against the allow-list (`127.0.0.1` / `localhost` / tailnet IP / MagicDNS name), else reject — **prevents DNS rebinding**;
  3. extract the `token` and compare in **constant time** (`ct_eq` from the `subtle` crate, to avoid a timing side channel);
  4. on failure, log one line and disconnect. **No distinction between "wrong token" and "no token"** — don't give attackers probing information.
- At most 5 handshake attempts per IP per 10 seconds, silently dropped beyond that (`IpAddr` buckets).

### 3.3 Multiple devices

One token allows multiple connections (e.g. two family phones); all share the same conversation table. No device-management UI — v1.14 skips it, introduce device names when needed.

---

## 4. Abuse prevention within a connection

### 4.1 Frame-level limits (hard caps enforced by the server)

| Item | Cap | Handling |
|---|---|---|
| Single frame JSON | 64 KiB | disconnect over limit |
| `text` field | 8 KiB (aligned with `send_keys`'s `MAX_SEND_TEXT_BYTES`) | reject that command |
| Inbound rate | 100 frames/sec/connection | disconnect over limit |
| No `pong` received | 60 seconds | disconnect (heartbeat 20s) |

### 4.2 Command validation (reuses the existing allow-list)

- `say.id` / `key.id` / `forward.from|to` must pass `validate_pane_id` and **exist in the ConversationRegistry** — a pane that doesn't exist is always rejected.
- `key.key` must hit the `ALLOWED_KEYS` allow-list (already in `tmux.rs`) — **the phone cannot send arbitrary key sequences**.
- On forward, `from != to`.
- Every command lands in the server log (time, source IP, pane, command type, text summary) for later auditing — **no "execute arbitrary command" interface at all**.

### 4.3 Desktop-side linkage

- On phone connect/disconnect, the desktop shows a "phone connected" status (also handy for troubleshooting).
- `ClientCommand::Refresh` triggers a full registry rescan + full resend.

---

## 5. Message flow (connecting to the phase-1 bridge)

```
phone ──say──▶ WS server ──ClientCommand::Say──▶ deliver() ──▶ intercom broker / send-keys
phone ◀──turn── WS server ◀──TranscriptSource polling ◀── registry + transcript files
phone ◀──awaiting-human── WS server ◀── intercom Message(expectsReply) ── broker
```

- Poll cadence: only runs while `has_clients()` is true (no idle spinning when the phone is offline); **polling narrows to subscription granularity**: only poll the subscribed conversations (`turn` pushed per subscription), other conversations are not polled — `status-changed` / `awaiting-human` still broadcast to everyone at no polling cost; when a pane has `awaiting-human` or a conversation in `thinking`, tighten the poll interval (500ms), otherwise 2s.
- On `subscribe`: immediately push the tail of that conversation's transcript (cursor start snapshot); `unsubscribe` stops that conversation's polling.
- Delivery of a phone `say` reply: via intercom it has a receipt (`delivered` event) → can be forwarded as `ClientEvent::Delivered` (optional); the send-keys path has no receipt, just show "sent".

---

## 6. Acceptance checklist

- [ ] Server listens only on `127.0.0.1:<dynamic port>`; `lsof` confirms no `0.0.0.0` listener
- [ ] No token / wrong token / wrong Host header are all rejected, without distinguishing the failure reason
- [ ] After connecting, the full `conversations` list arrives; desktop shows the phone online
- [ ] Phone sends `say` → the agent in the pane receives it; `key` outside the allow-list is rejected
- [ ] Phone sends a non-existent pane id → rejected
- [ ] Frame over 64 KiB / over rate / heartbeat missed → disconnected
- [ ] Server stops transcript polling while the phone is offline (`has_clients` in effect)
- [ ] An old token dies immediately after the app exits (re-scan required on restart)

On-device verification:

- [ ] iPhone + Tailscale on the same tailnet, scan to connect, multi-conversation send/receive in parallel
- [ ] Cross-pane forward A→B works and carries a source label
- [ ] When an agent `ask`s, the phone receives an `awaiting-human` notification (with reply_to)
