# v1.14 Transport + security design: how the phone connects and why it trusts us

> PRD v1.12 defined the mobile protocol (event/command JSON), but **transport and security were left blank**. This fills them in: WebSocket server selection, listening surface, authentication, abuse prevention.
>
> **2026-08 LAN decision:** v1.14 ships trusted-LAN access first; Tailscale/VPN remains a later transport option. LAN HTTP/WS is plaintext and must only be used on a network whose members are trusted.

---

## 1. Transport selection

### 1.1 WebSocket server

- **Library:** `tokio-tungstenite` (Tauri 2 already ships a tokio runtime; no heavy new dependency).
- **Port:** dynamically allocated (`0.0.0.0:0` grabs a free port), avoiding fixed-port conflicts.
- **Listen:** bound to `0.0.0.0`; the accept loop only permits loopback, RFC1918/link-local LAN and reserved tailnet sources. Public source IPs are dropped. Host validation independently rejects public IPs and arbitrary domains.
- **Paths:** `GET /v1/?token=<token>` serves the single-file mobile SPA; `ws://<LAN-IP>:<port>/v1/ws?token=<token>` is WebSocket, subprotocol fixed at `tmuxdeck.v1`.
- **Lifecycle:** a background task started in Tauri `setup`; it keeps running with no connected clients (the conversation-table refresh still consumes intercom events); the phone client owns reconnection.

### 1.2 How the phone connects (key decision)

The desktop enumerates trusted IPv4 addresses (default-route LAN, other private interfaces, then Tailscale `100.64/10`) and exposes one QR/copy URL per host:

```
http://<LAN-or-tailnet-IP>:<dynamic-port>/v1/?token=<per-launch-token>
```

This is the minimal discovery mechanism: no multicast daemon, fixed port or second service. The desktop pairing panel is the discovery surface. The page keeps the token in memory, immediately removes it from the address bar, loads no third-party resources, and opens the same-port WebSocket.

**Security model and exposed surface:** every device on the trusted LAN can see the dynamically allocated TCP port and plaintext traffic; only holders of the 256-bit per-launch token can load the page or complete a WebSocket handshake. This prevents accidental/unauthorized use but does not provide confidentiality against a hostile LAN observer. Do not use it on guest, public, hotel or otherwise untrusted Wi-Fi. VPN/TLS can be layered later without changing the event protocol.

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
| Port | dynamically allocated, bound to `0.0.0.0` | one LAN-visible TCP port; no fixed-port conflict |
| Source IP | loopback, private/link-local LAN; tailnet ranges reserved | public sources dropped before HTTP/WS processing |
| HTTP service | same port, only `GET /v1/` and `/v1` redirect | one embedded, self-contained SPA; no filesystem serving |
| DNS rebinding | HTTP and WS both validate `Host` | arbitrary domains/public Hosts rejected |
| Plaintext | `http://` + `ws://` | **trusted LAN only**; token authenticates but does not encrypt |

---

## 3. Authentication: pairing token

### 3.1 Token generation and delivery

- Generated at every app launch as a **32-byte CSPRNG token** (`OsRng`), **never written to disk**.
- Shown in the desktop UI as a QR code + copyable text; the phone scans it and connects with the token included.
- The token exists only in: desktop memory, the QR code/clipboard, phone memory. It expires when the app exits — **no persistence, no revocation list** (no persistent credential means no revocation problem).

### 3.2 Handshake and validation

- HTTP URL: `http://host:port/v1/?token=<hex>`; WebSocket URL: `ws://host:port/v1/ws?token=<hex>`.
- On the server side, at handshake time:
  1. validate the subprotocol = `tmuxdeck.v1` (WebSocket only);
  2. validate the HTTP/WS `Host` against loopback, private/link-local IP, reserved tailnet IP or MagicDNS; arbitrary domains/public IPs are rejected — **prevents DNS rebinding**;
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
- Every command lands in the server audit log (time, source IP, pane, command type, text byte length, outcome). **Message content is never recorded** — and there is no "execute arbitrary command" interface.

### 4.3 Desktop-side linkage

- `bridge_pairing` returns `{ enabled, port, httpUrls, lanUrls, wsUrls, token, connectedClients, brokerConnected, trustedLanOnly }`.
- On authenticated WebSocket connect/disconnect, state updates and Tauri emits `mobile-clients-changed` with `{ connectedClients }`.
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

- [x] Automated: one dynamic `0.0.0.0` TCP port; public source/Host rejection; HTTP/WS token checks; fixed routes
- [x] Automated: authenticated connect/disconnect count events; pane/key/forward/subscribe validation; initial refresh snapshot; pending `ask` reply routing
- [x] Automated: frame/rate/heartbeat limits and offline subscription polling semantics remain covered
- [ ] On-device: confirm OS firewall prompt/exposure and LAN reachability from a physical phone
- [ ] An old token dies immediately after the app exits (re-scan required on restart)

On-device verification:

- [ ] iPhone on the same trusted LAN, scan to connect, multi-conversation send/receive
- [ ] Cross-pane forward A→B works and carries a source label
- [ ] When an agent `ask`s, the phone receives an `awaiting-human` notification (with reply_to)
