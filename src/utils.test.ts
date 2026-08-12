import assert from "node:assert";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import QRCode from "qrcode";
import { dictionaries, t, tPlural, translateError } from "./i18n.ts";
import {
  applyPaneContents,
  dominantAgentId,
  preservePaneContent,
  matchAgentIdForCommand,
  reorderIds,
  resizePaneAgents,
  resolvePaneAgentId,
  sanitizeNameFrontend,
  summarizePaneAgents,
} from "./utils.ts";
import type { CreateOpts, ToolInfo, TmuxSession, TmuxPane } from "./types.ts";

test("sanitizeNameFrontend - valid alphanumeric names", () => {
  assert.strictEqual(sanitizeNameFrontend("my-project"), "my-project");
  assert.strictEqual(sanitizeNameFrontend("project_123"), "project_123");
});

test("sanitizeNameFrontend - trims whitespace and special characters", () => {
  assert.strictEqual(sanitizeNameFrontend("  hello world!  "), "hello-world");
  assert.strictEqual(sanitizeNameFrontend("foo@bar#baz!"), "foo-bar-baz");
});

test("sanitizeNameFrontend - collapse multiple dashes and strips leading/trailing dashes", () => {
  assert.strictEqual(sanitizeNameFrontend("---foo---bar---"), "foo-bar");
  assert.strictEqual(sanitizeNameFrontend("!!!"), "");
});

test("card reorder remains stable for native session ids", () => {
  const order = ["native:alpha", "$2", "native:beta"];
  assert.deepStrictEqual(
    reorderIds(order, "native:beta", "native:alpha"),
    ["native:beta", "native:alpha", "$2"]
  );
  assert.deepStrictEqual(order, ["native:alpha", "$2", "native:beta"]);
});

test("destroy confirmation distinguishes pane termination from terminal detach", () => {
  const message = tPlural("confirm.destroy", 4, { name: "workspace" });
  assert.match(message, /workspace/);
  assert.match(message, /4 tmux panes/);
  assert.match(message, /Cmd\+W/);
  assert.match(message, /detach/);
});

test("native split models and translations", () => {
  assert.strictEqual(t("card.runningDetached"), "Running · Detached");
  assert.match(t("card.nativeRenameUnsupported"), /cannot be renamed/);
  assert.match(
    t("ERR_NATIVE_WORKSPACE_RENAME_UNSUPPORTED"),
    /cannot be renamed/
  );
  assert.match(
    tPlural("confirm.destroyNative", 1, { name: "native-workspace" }),
    /final Agent/
  );
  const samplePane: TmuxPane = {
    id: "%1",
    command: "zsh",
    active: true,
    session_target: "workspace:1",
    slot: "1",
    attached: false,
  };
  const sampleSession: TmuxSession = {
    id: "s1",
    name: "native-workspace",
    windows_count: 1,
    panes_count: 2,
    attached: false,
    created_at: "123456",
    last_active_ts: 0,
    native_split: true,
    terminal_id: "ghostty",
    panes: [samplePane],
  };
  assert.strictEqual(sampleSession.native_split, true);
  assert.strictEqual(sampleSession.terminal_id, "ghostty");
  assert.strictEqual(samplePane.session_target, "workspace:1");
  assert.strictEqual(samplePane.slot, "1");
  assert.strictEqual(samplePane.attached, false);
});

test("native slot startup exit includes status or signal detail", () => {
  assert.match(
    translateError("ERR_NATIVE_SLOT_AGENT_EXITED|workspace__td_slot_01|status|127"),
    /workspace__td_slot_01 \(exit status 127\)/
  );
  assert.match(
    translateError("ERR_NATIVE_SLOT_AGENT_EXITED|workspace__td_slot_01|signal|9"),
    /workspace__td_slot_01 \(signal 9\)/
  );
});

const makeSession = (
  id: string,
  name: string,
  panes: TmuxPane[]
): TmuxSession => ({
  id,
  name,
  windows_count: 1,
  panes_count: panes.length,
  attached: false,
  created_at: "123456",
  last_active_ts: 0,
  panes,
});

const makePane = (id: string, content?: string): TmuxPane => ({
  id,
  command: "zsh",
  active: false,
  ...(content === undefined ? {} : { content }),
});

test("preservePaneContent keeps captured output across a session poll", () => {
  const previous = [
    makeSession("s1", "alpha", [makePane("%1", "hello"), makePane("%2")]),
  ];
  const incoming = [
    makeSession("s1", "alpha", [makePane("%1"), makePane("%2"), makePane("%3")]),
    makeSession("s2", "beta", [makePane("%9")]),
  ];

  const merged = preservePaneContent(previous, incoming);
  assert.strictEqual(merged[0].panes[0].content, "hello");
  assert.strictEqual(merged[0].panes[1].content, undefined);
  // A pane that appeared this poll has nothing to carry over.
  assert.strictEqual(merged[0].panes[2].content, undefined);
  // A brand new session passes through untouched.
  assert.strictEqual(merged[1], incoming[1]);
  assert.strictEqual(previous[0].panes[0].content, "hello", "no mutation");
});

test("preservePaneContent re-matches a session that was renamed or re-ided", () => {
  const previous = [makeSession("s1", "alpha", [makePane("%1", "kept")])];
  // tmux reports a new id for the same session name.
  const incoming = [makeSession("s7", "alpha", [makePane("%1")])];
  assert.strictEqual(preservePaneContent(previous, incoming)[0].panes[0].content, "kept");
});

test("applyPaneContents commits a whole capture round in one pass", () => {
  const sessions = [
    makeSession("s1", "alpha", [makePane("%1"), makePane("%2")]),
    makeSession("s2", "beta", [makePane("%3", "old")]),
  ];

  const next = applyPaneContents(
    sessions,
    new Map([
      ["%1", "one"],
      ["%3", "new"],
    ])
  );
  assert.strictEqual(next[0].panes[0].content, "one");
  assert.strictEqual(next[1].panes[0].content, "new");
  // Panes with no capture in this round keep their previous identity.
  assert.strictEqual(next[0].panes[1], sessions[0].panes[1]);
  assert.strictEqual(sessions[0].panes[0].content, undefined, "no mutation");
});

test("applyPaneContents returns the same array when nothing changed", () => {
  const sessions = [makeSession("s1", "alpha", [makePane("%1", "same")])];
  // An empty round and an identical round must both skip the re-render.
  assert.strictEqual(applyPaneContents(sessions, new Map()), sessions);
  assert.strictEqual(
    applyPaneContents(sessions, new Map([["%1", "same"]])),
    sessions
  );
  // An unknown pane id must not fabricate state either.
  assert.strictEqual(
    applyPaneContents(sessions, new Map([["%404", "gone"]])),
    sessions
  );
  // A real change still produces a new array.
  assert.notStrictEqual(
    applyPaneContents(sessions, new Map([["%1", "changed"]])),
    sessions
  );
});

test("resizePaneAgents keeps the per-pane list matched to the pane count", () => {
  // Growing a uniform list keeps that agent; a mixed list takes the default.
  assert.deepStrictEqual(resizePaneAgents(["pi", "pi"], 4, "claude"), [
    "pi",
    "pi",
    "pi",
    "pi",
  ]);
  assert.deepStrictEqual(resizePaneAgents(["pi", "codex"], 3, "claude"), [
    "pi",
    "codex",
    "claude",
  ]);
  // Shrinking truncates, and an empty list is filled from the default.
  assert.deepStrictEqual(resizePaneAgents(["pi", "codex", "claude"], 2, "pi"), [
    "pi",
    "codex",
  ]);
  assert.deepStrictEqual(resizePaneAgents([], 2, "shell"), ["shell", "shell"]);
  assert.deepStrictEqual(resizePaneAgents(["pi"], 0, "pi"), []);

  const source = ["pi", "codex"];
  resizePaneAgents(source, 4, "claude");
  assert.deepStrictEqual(source, ["pi", "codex"], "input must not be mutated");
});

test("summarizePaneAgents reports uniform vs mixed pane assignments", () => {
  const uniform = summarizePaneAgents(["pi", "pi", "pi"]);
  assert.strictEqual(uniform.uniform, true);
  assert.strictEqual(uniform.agentId, "pi");
  assert.deepStrictEqual(uniform.groups, [{ agentId: "pi", count: 3 }]);

  const mixed = summarizePaneAgents(["pi", "claude", "pi", "shell"]);
  assert.strictEqual(mixed.uniform, false);
  assert.strictEqual(mixed.agentId, null);
  assert.deepStrictEqual(mixed.groups, [
    { agentId: "pi", count: 2 },
    { agentId: "claude", count: 1 },
    { agentId: "shell", count: 1 },
  ]);

  const empty = summarizePaneAgents([]);
  assert.strictEqual(empty.uniform, true);
  assert.strictEqual(empty.agentId, null);
});

test("mixed workspace summary renders every Agent with its pane count", () => {
  const mix = summarizePaneAgents(["pi", "claude", "pi"])
    .groups.map((group) =>
      t("modal.agentMixItem", { agent: group.agentId, n: group.count })
    )
    .join(t("modal.agentMixSeparator"));
  assert.strictEqual(mix, "pi ×2 · claude ×1");
  assert.strictEqual(
    t("modal.summaryMixed", {
      panesText: tPlural("modal.panesCount", 3),
      mix,
      terminal: "Ghostty",
    }),
    "Will create 3 panes (pi ×2 · claude ×1), and open with Ghostty."
  );
});

test("dominantAgentId picks the most common agent, ties by first appearance", () => {
  assert.strictEqual(dominantAgentId(["pi", "claude", "pi"]), "pi");
  assert.strictEqual(dominantAgentId(["claude", "pi"]), "claude");
  assert.strictEqual(dominantAgentId([]), null);
});

test("matchAgentIdForCommand identifies agent panes and ignores plain shells", () => {
  const agents: ToolInfo[] = [
    { id: "pi", name: "Pi", path: "/usr/local/bin/pi" },
    { id: "claude", name: "Claude Code", path: "/opt/homebrew/bin/claude" },
    { id: "shell", name: "agent.shell", path: "/bin/zsh" },
  ];
  assert.strictEqual(matchAgentIdForCommand("claude", agents), "claude");
  assert.strictEqual(
    matchAgentIdForCommand("/opt/homebrew/bin/claude --model opus", agents),
    "claude"
  );
  assert.strictEqual(matchAgentIdForCommand("zsh", agents), null);
  assert.strictEqual(matchAgentIdForCommand("", agents), null);
  assert.strictEqual(matchAgentIdForCommand(undefined, agents), null);
});

test("resolvePaneAgentId trusts the recorded agent_id over the live command", () => {
  const agents: ToolInfo[] = [
    { id: "pi", name: "Pi", path: "/usr/local/bin/pi" },
    { id: "claude", name: "Claude Code", path: "/opt/homebrew/bin/claude" },
    { id: "shell", name: "agent.shell", path: "/bin/zsh" },
  ];

  // cci reports its version as current_command, so command matching cannot work.
  assert.strictEqual(
    resolvePaneAgentId({ agent_id: "cci", command: "0.10.0" }, agents),
    "cci"
  );
  // Recorded id wins even when the command matches a different agent.
  assert.strictEqual(
    resolvePaneAgentId({ agent_id: "claude", command: "pi" }, agents),
    "claude"
  );
  // No recorded id: fall back to matching the command.
  assert.strictEqual(
    resolvePaneAgentId({ command: "claude --model opus" }, agents),
    "claude"
  );
  // An explicit shell pane is not an Agent pane.
  assert.strictEqual(
    resolvePaneAgentId({ agent_id: "shell", command: "zsh" }, agents),
    null
  );
  assert.strictEqual(
    resolvePaneAgentId({ agent_id: "  ", command: "zsh" }, agents),
    null
  );
});

test("add-pane recommendation follows recorded pane agents", () => {
  const agents: ToolInfo[] = [
    { id: "cci", name: "CCI", path: "/usr/local/bin/cci" },
    { id: "shell", name: "agent.shell", path: "/bin/zsh" },
  ];
  const panes: Pick<TmuxPane, "agent_id" | "command">[] = [
    { agent_id: "cci", command: "0.10.0" },
    { agent_id: "cci", command: "0.10.0" },
    { agent_id: "shell", command: "zsh" },
  ];
  const resolved = panes
    .map((pane) => resolvePaneAgentId(pane, agents))
    .filter((agentId): agentId is string => Boolean(agentId));
  assert.deepStrictEqual(resolved, ["cci", "cci"]);
  assert.strictEqual(dominantAgentId(resolved), "cci");

  // A shell-only workspace recommends nothing, so no menu item is highlighted.
  const shellOnly = [{ agent_id: "shell", command: "zsh" }]
    .map((pane) => resolvePaneAgentId(pane, agents))
    .filter((agentId): agentId is string => Boolean(agentId));
  assert.strictEqual(dominantAgentId(shellOnly), null);
});

test("CreateOpts carries per-pane agent ids alongside the default agent", () => {
  const opts: CreateOpts = {
    name: "workspace",
    dir: null,
    agent_id: "pi",
    pane_agent_ids: resizePaneAgents(["claude"], 2, "pi"),
    panes: 2,
    terminal_id: "ghostty",
  };
  assert.deepStrictEqual(opts.pane_agent_ids, ["claude", "claude"]);
  assert.strictEqual(opts.pane_agent_ids.length, opts.panes);
});

test("missing agent error names the agent that could not be resolved", () => {
  // Rust emits ERR_AGENT_NOT_FOUND|<agent_id> from resolve_agent_command.
  assert.strictEqual(
    translateError("ERR_AGENT_NOT_FOUND|cci"),
    "Agent not found (it may have been uninstalled or removed from configuration): cci"
  );
  assert.doesNotMatch(
    translateError("ERR_AGENT_NOT_FOUND|cci"),
    /ERR_AGENT_NOT_FOUND/,
    "error code must not leak into the UI"
  );
});

test("pane agent count mismatch error reports expected and actual counts", () => {
  assert.strictEqual(
    translateError("ERR_PANE_AGENT_COUNT|4|3"),
    "Per-pane Agent count does not match the pane count: expected 4, got 3"
  );
  assert.strictEqual(
    translateError("ERR_PANE_AGENT_COUNT"),
    "Per-pane Agent count does not match the pane count"
  );
});

test("every Tauri command error code has an en and zh translation", () => {
  const { en, zh } = dictionaries;
  const commandsDir = path.resolve(process.cwd(), "src-tauri/src/commands");
  const codes = new Set<string>();
  for (const file of fs.readdirSync(commandsDir)) {
    if (!file.endsWith(".rs")) continue;
    const source = fs.readFileSync(path.join(commandsDir, file), "utf-8");
    for (const match of source.matchAll(/"(ERR_[A-Z0-9_]+)/g)) {
      codes.add(match[1]);
    }
  }
  assert.ok(codes.size > 0, "expected to find error codes in the command layer");

  const untranslated = [...codes].filter((code) => !en[code] || !zh[code]);
  assert.deepStrictEqual(
    untranslated,
    [],
    `command errors reach the UI via translateError; add translations for: ${untranslated.join(", ")}`
  );
});

test("English and Chinese dictionaries expose the same keys", () => {
  const { en, zh } = dictionaries;
  assert.deepStrictEqual(Object.keys(en).sort(), Object.keys(zh).sort());
  for (const key of [
    "modal.perPaneAgentLabel",
    "modal.paneIndexLabel",
    "modal.applyToAll",
    "modal.applyToAllTitle",
    "modal.summaryMixed",
    "modal.agentMixItem",
    "modal.agentMixSeparator",
    "card.addPaneWith",
    "card.addPaneChoose",
    "card.addPaneRecommended",
    "ERR_AGENT_NOT_FOUND",
    "ERR_PANE_AGENT_COUNT",
    "val.paneAgentCountDetail",
  ]) {
    assert.ok(en[key], `missing en translation for ${key}`);
    assert.ok(zh[key], `missing zh translation for ${key}`);
  }
  // Placeholders must survive translation in both locales.
  assert.match(zh["modal.summaryMixed"], /\{mix\}/);
  assert.match(zh["modal.applyToAllTitle"], /\{agent\}/);
  assert.match(zh["modal.paneIndexLabel"], /\{n\}/);
  assert.match(zh["val.paneAgentCountDetail"], /\{expected\}[\s\S]*\{actual\}/);
});

test("Standard qrcode library generates valid SVG containing QR elements", async () => {
  const targetUrl = "http://192.168.1.100:3030/v1/?token=0123456789abcdef";
  const svg = await QRCode.toString(targetUrl, { type: "svg", margin: 1 });
  assert.ok(svg.includes("<svg"));
  assert.ok(svg.includes("</svg>"));
  assert.ok(svg.includes("path") || svg.includes("rect"));

  const qrData = QRCode.create(targetUrl, { errorCorrectionLevel: "M" });
  assert.ok(qrData.modules.size > 0);
  assert.strictEqual(qrData.modules.size % 4, 1);
});

test("Static mobile HTML contains bugfixes for sendSay, WS resubscribe, ALLOWED_KEYS and subprotocol", () => {
  const htmlPath = path.resolve(process.cwd(), "src-tauri/mobile/index.html");
  assert.ok(fs.existsSync(htmlPath), "src-tauri/mobile/index.html must exist");

  const html = fs.readFileSync(htmlPath, "utf-8");

  // 1. sendSay accepts parameter override
  assert.match(html, /sendSay\s*\(\s*quickText\s*\)/);

  // 2. WS onopen resubscribes if activeConvId exists
  assert.match(html, /if\s*\(\s*this\.activeConvId\s*\)\s*\{\s*this\.sendCmd\(\s*\{\s*type:\s*'subscribe'/);

  // 3. ALLOWED_KEYS matches Rust whitelist
  assert.match(html, /'BSpace'/);
  assert.match(html, /'Up'/);
  assert.match(html, /'Down'/);
  assert.match(html, /'C-c'/);
  assert.match(html, /'Escape'/);

  // 4. Subprotocol tmuxdeck.v1 is specified
  assert.match(html, /'tmuxdeck\.v1'/);

  // 5. sendSay does NOT perform optimistic append to avoid duplicate turns & disconnected false success
  assert.strictEqual(
    /sendSay[\s\S]*?this\.turns\[this\.activeConvId\]\.push/.test(html),
    false,
    "sendSay must not optimistically push turns to local state"
  );

  // 6. Mobile browser compatibility: dynamic viewport, safe areas, keyboard-safe inputs and touch targets
  assert.match(html, /viewport-fit=cover/);
  assert.doesNotMatch(html, /user-scalable=no|maximum-scale=1\.0/);
  assert.match(html, /visualViewport/);
  assert.match(html, /--app-height/);
  assert.match(html, /safe-area-inset-bottom/);
  assert.match(html, /-webkit-overflow-scrolling:\s*touch/);
  assert.match(html, /\.input-field[\s\S]*?font-size:\s*16px/);
  assert.match(html, /\.key-btn[\s\S]*?min-height:\s*44px/);
  assert.doesNotMatch(html, /(^|[^-])shrink:\s*0/m);
  assert.match(html, /enterkeyhint="send"/);
});
