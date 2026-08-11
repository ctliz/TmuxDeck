# v1.12 Decision log: rejected approaches

> The mobile-access feature went through five rejected designs before it took shape.
> This is recorded so the same ones are **not proposed again** — each entry says why it was rejected then, and under what conditions it is worth revisiting.

---

## 1. Embedded HTTP server + mobile web PWA

**Rejected.**

Initial design: run axum inside the Tauri process, bind `0.0.0.0:7420`, and serve a responsive web page; the phone pairs by scanning a QR code. It shipped with a pairing code, device tokens, a revocation list, CORS and DNS-rebinding protection.

Why rejected:

- What the phone actually needs is **triage** (which of ten agents is waiting for me), not another web page
- Plaintext HTTP on a LAN means tokens can be sniffed by anything on the same subnet; TLS means a miserable iOS self-signed-certificate experience
- iOS Web Push only works once a PWA is added to the home screen — a precondition users won't follow
- The whole pairing/auth/revocation scheme is a **home-grown security mechanism**, and this is an attack surface that exposes shell execution

**Revisit if:** a desktop-grade, information-dense multi-conversation interface is needed and HTTPS already exists (Tailscale or a tunnel).

---

## 2. Full terminal emulation (xterm.js + WebSocket)

**Rejected.**

Why rejected:

- SSH clients (Blink / Termius) plus Tailscale already do this today, with a better experience
- All clients of one tmux session **share the window size** — as soon as the phone attaches, the desktop terminal gets squeezed to phone width. Working around it needs grouped sessions (`tmux new-session -t <original session>`), a whole block of extra complexity
- It solves "I want to see the output", while the bottleneck is "who's waiting for me"

**Revisit if:** the phone needs to operate the TUI itself (rather than talk to the agent).

---

## 3. IM-bot notifications only (Feishu / Telegram)

**Partially kept, but not the primary form.**

A bot has real advantages as a client: outbound connections only, no open ports, push for free, works away from the desk. But it is **one-way notifications + a linear conversation** — it can't give the "run multiple conversations at once and forward between them" shape, which is what the user wants.

Current state: the `Transport` trait is already abstracted, so a bot can be plugged in as one of its implementations.

---

## 4. Build our own cross-tool agent message bus

**Reverted.**

We once planned `tmuxdeck send @backend "..."`, using the shell as the least common denominator across all agents. The idea itself was right — but **[Agent Intercom](https://github.com/dataforxyz/agent-intercom-pi) already does it**, covering the four harnesses Pi / Codex / Claude Code / OpenCode, and it also solves the parts we hadn't started: durable delivery, queuing during busy periods, delivery receipts, and message-storm rate limiting.

See [PRIOR-ART-agent-bus.md](./PRIOR-ART-agent-bus.md).

**Conclusion:** TmuxDeck does not build a bus; it becomes the one piece missing from that family — the **human / phone adapter**.

---

## 5. Polling capture-pane for four-state detection

**Rejected. Deleted entirely.**

We once designed a heuristic: poll `capture-pane` every 2 seconds, hash the content, and infer from "silence duration" whether an agent is working, waiting on another agent, or waiting on a human. Along the way we found that global silence under-reports (a busy Pi cluster masks a stuck Claude Code), so we added a "communication group" concept to narrow the scope.

**All of it was scrapped** — the broker directly reports `idle` / `thinking` / `tool:<name>` as fact, auto-reported by each session. No matter how clever, a heuristic should not guess a value that can simply be read.

Agents not on intercom (Aider, Gemini CLI, plain shell) report `unknown`. **Don't guess** — better to show unknown than to manufacture false signals with a jittery heuristic.

---

## 6. Hooks on every agent reporting events

**Not adopted, but not rejected.**

Idea: install `Notification` / `Stop` hooks for Claude Code that POST to a local endpoint on events. The signal would go from heuristic to fact, and latency from tens of seconds to sub-second.

Why not adopted: intercom already provides the same signal and **needs no changes to the user's agent configuration**. Installing hooks is invasive, and you'd have to write one per agent.

**Revisit if:** a commonly used agent still has no intercom adapter but does have a decent hook mechanism.

---

## Principles that survive

- **Rather show unknown than manufacture false signals with heuristics.** Notification value rests on trust; the cost of one false alarm outweighs the convenience of one missed alert.
- **When a fact can be read, don't guess.** This is the root reason entry 5 was overturned.
- **Don't invent your own security mechanisms.** Entry 1's pairing/token/revocation is the textbook counter-example; bots and intercom both rely on existing trust boundaries (IM accounts, same OS user).
