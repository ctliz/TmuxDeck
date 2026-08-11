# v1.13 TranscriptSource design: where conversation content comes from

> Goal: let the phone see **turn-by-turn conversation content** for the agent in each pane, instead of a scrolling terminal screen. This resolves the open question from PRD v1.12 §4.
>
> Bottom line up front: **the primary path reads each harness's own structured session log** (pi / Claude Code preferred), and `capture-pane` is only the fallback for harnesses not covered. The pane → log-file association is not guessed — it relies on two facts: the "intercom session ID in the filename" and the "cwd directory slug".

---

## 0. Checked against local facts (tested 2026-08-10)

Before implementing, we mapped out each harness's log format so the design is not based on guesses:

| harness | Log location | Key facts |
|---|---|---|
| pi | `~/.pi/agent/sessions/<slug>/<ts>_<uuid>.jsonl` | **The `uuid` in the filename is the intercom session ID** (tested: `019fec77…` matches the session id from `intercom list`). Pane association is therefore an exact match, not a heuristic |
| Claude Code | `~/.claude/projects/<slug>/<uuid>.jsonl` | Every `user`/`assistant` line carries a `cwd` field, which can be used to verify the slug resolution |
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<uuid>.jsonl` | `session_meta` has `payload.cwd`; the desktop app (ChatGPT.app) stores elsewhere in `~/Library/Application Support/Claude/local-agent-mode-sessions/` |

### Sample lines per format (tested)

pi (`type: message`, content in `message.content[]`, filter by `type: text`):

```jsonc
{"type": "message", "id": "51dc3b00", "parentId": "…", "timestamp": "2026-08-09T07:40:09.946Z",
 "message": {"role": "user", "content": [{"type": "text", "text": "the current vision client…"}], "timestamp": 1786261209943}}
```

Claude Code (`user` lines: user input and `tool_result` both live in `content[]`; `assistant` lines mix `thinking` / `text` / `tool_use`):

```jsonc
{"type": "user", "cwd": "/Users/tsiji/Documents/fflink/front", "timestamp": "2026-07-27T13:00:36.416Z",
 "message": {"role": "user", "content": [{"type": "text", "text": "I'm currently working on a wing…"}]}}
{"type": "assistant", "timestamp": "2026-07-27T13:00:48.569Z",
 "message": {"role": "assistant", "content": [{"type": "text", "text": "Let me first look at the docs in the project…"}]}}
```

**Key point:** both formats carry clean `content[].text` for turns, with no escape sequences; tool calls and thinking are separate `type`s (`tool_use` / `thinking` / `tool_result`) that can be filtered as needed. That's why this beats `capture-pane` and `pipe-pane` for "conversation".

---

## 1. pane → log-file association

`Conversation` already carries `cwd` and `intercom_session_id` (landed in phase 1). The association has three per-harness paths, all fact-based:

### 1.1 pi: exact intercom session ID match (preferred)

Under `~/.pi/agent/sessions/<slug>/` the filenames look like `2026-08-10T16-18-14-827Z_019fec77-b0ab-….jsonl`, where the `uuid` segment starts with the intercom session ID (tested, matches).

- Path: `conv.intercom_session_id` → scan the slug directory for that cwd, find the file whose uuid prefix matches. **One-to-one, can't misidentify.**
- Fallback (no intercom): the jsonl with the latest mtime in the same directory.

slug rule (tested): `"-" + cwd.replace("/", "-") + "-"`. Example: `/Users/tsiji/Documents/TmuxDeck` → `--Users-tsiji-Documents-TmuxDeck--`.

### 1.2 Claude Code: cwd slug + content verification

- Directory: `~/.claude/projects/<slug>/`, slug rule `"-" + cwd.replace("/", "-")` (**no trailing dash, differs from the pi rule** — tested).
- **Verification:** the directory name alone isn't reliable (special characters in paths cause ambiguity), so scan the newest jsonl in the candidate directory, read the first few lines, and confirm using the `cwd` field in the log against `conv.cwd`. The directory count is limited (<100 locally); cache the mapping.
- File selection: the jsonl with the latest mtime in the same directory = the in-progress session.
- Multiple agents in one cwd (common: 3 pi sessions under `vision`): this approach cannot distinguish them on the Claude side (the Claude jsonl uuid has no pane correspondence). Logged as a known boundary (see §5).

### 1.3 Codex: `session_meta.payload.cwd` match (v1.13 associates only, no extraction)

`~/.codex/sessions/YYYY/MM/DD/` is organized by day, with rolling files `rollout-<ts>-<uuid>.jsonl`. Read `payload.cwd` from the `session_meta` line, compare against `conv.cwd`, take the newest. **v1.13 only implements association probing; turn extraction is not implemented** (Codex's `developer` role contains lots of system prompt; the extraction rules need separate design), so it first goes through the capture-pane fallback. Desktop Codex logs live in the app sandbox directory and are likewise out of scope.

### 1.4 Uncovered harnesses (Aider / Gemini CLI / plain shell)

No structured logs → go straight to the `CapturePaneSource` fallback (already in `bridge.rs`).

---

## 2. Turn extraction rules

| Log line | Role | Extraction |
|---|---|---|
| pi `message.role == user`, `content[].type == text` | `Human` | `text` |
| pi `message.role == assistant`, `content[].type == text` | `Agent` | `text` |
| Claude `type == user`, `content[].type == text` (exclude `isMeta`/`isCompactSummary`) | `Human` | `text` |
| Claude `type == assistant`, `content[].type == text` | `Agent` | `text` |
| `thinking` / `tool_use` / `tool_result` / `attachment` | — | **drop** (not shown in v1.13, to avoid flooding the phone with file content; a later version can add a toggle) |
| `custom_message` / `system` / `compaction` | `System` | only `compaction` summary text is optionally usable |

Timestamps: both use RFC3339 (`2026-08-09T07:40:09.946Z`); `Turn.timestamp` normalizes to milliseconds. No chrono dependency — write a minimal parser that only accepts `YYYY-MM-DDTHH:MM:SS[.mmm]Z` (all local files are produced by `new Date().toISOString()`, fixed format).

---

## 3. Incremental fetch: byte cursor

`TranscriptSource::poll(conv, since)` is implemented with "file append log" semantics:

- Each associated file keeps a `(path, byte_offset)` cursor (a `HashMap`).
- Each poll resumes from the cursor, **never rescans the whole file**; parses new lines, filters `timestamp > since`, produces `Turn`s, advances the cursor.
- When the file is rotated/compacted (Claude compaction, pi archival) so that `file_len < cursor`, the cursor resets to read from the top, deduplicating via `since`.
- When the file disappears (agent restarted, new file): re-run the §1 association, reset the cursor to zero, but `since` guarantees old turns are not re-pushed.

---

## 4. Priority chain (Composite)

```
CompositeTranscriptSource::poll(conv, since)
 ├─ kind=Pi          → PiTranscriptSource (exact intercom session ID match)
 ├─ kind=ClaudeCode  → ClaudeTranscriptSource (cwd scan + verification)
 └─ others (Codex/Shell/Unknown/match failed) → CapturePaneSource (full-screen fallback)
```

Layered with logging: `transcript[%3] pi → file.jsonl (cursor 1234)`, `transcript[%3] no structured record, fallback capture-pane`. The phone neither knows nor cares which path was used.

---

## 5. Boundaries and known pitfalls

1. **Claude desktop (Claude.app):** logs live in `~/Library/Application Support/Claude/local-agent-mode-sessions/`, and the uuid has no pane correspondence. v1.13 only covers CLI's `~/.claude/projects/`; desktop goes best-effort on "latest mtime + cwd", falling back when uncertain.
2. **Multiple agents in one cwd:** pi distinguishes exactly via the intercom ID; Claude/Codex cannot, only the newest file is taken. If multiple Claude sessions run in one cwd, content will mix — call this limitation out at acceptance.
3. **pi `compaction`:** compresses history into one record; after a cursor reset, `since` dedupes so it is not re-pushed.
4. **Same timestamps:** Claude sometimes emits several lines in the same millisecond; the cursor advances line by line so nothing is missed, and ordering follows file order, no re-sorting.
5. **File permissions:** `~/.claude/projects/**/*.jsonl` is `0600` (tested), but TmuxDeck runs as the same user as Claude, so no issue.

---

## 6. Acceptance checklist

Unit tests (synthetic jsonl fixtures, written to a temp dir) — all passing (`cargo test`, 27 items):

- [x] pi: filename uuid prefix matches intercom session ID → unique file
- [x] pi: `user`/`assistant` text extracted as Human/Agent, `tool_call` ignored
- [x] Claude: slug directory located + `cwd` field verified; `thinking`/`tool_use`/`tool_result` ignored
- [x] Incremental: second poll returns only new lines (cursor advances correctly)
- [x] File rotation: cursor reset + `since` dedup, no re-push
- [x] RFC3339 → millisecond parser (with and without milliseconds)
- [x] Multiple lines in the same millisecond: none missed or duplicated (last_seen dedup)
- [x] Composite: pi branch doesn't panic without a log; falls back
- [x] On-device: the current pi session resolves to the right jsonl and extracts this conversation's turns (`cargo test -- --ignored`)

On-device verification (needs a machine running pi + Claude Code):

- [ ] Poll the current pi session for this conversation's turns (matches the session from `intercom list`) ✅ verified
- [ ] Poll a pane running Claude Code for that project's conversation (no CLI-mode Claude currently running locally)
- [ ] When the agent is speaking (thinking), the latest text is available; tool execution produces no noisy turns
