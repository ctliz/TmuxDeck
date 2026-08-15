import assert from "node:assert";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import QRCode from "qrcode";
import { agentDisplayName, dictionaries, t, tPlural, translateError } from "./i18n.ts";
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
import { claudeHint, claudeSwitchTarget, reorderPaneAgentsForLead } from "./types.ts";
import type {
  CreateOpts,
  ManagedClaudeStatus,
  ToolInfo,
  TmuxSession,
  TmuxPane,
  WorkspaceInstallPlan,
} from "./types.ts";

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

const claudeStatus = (
  state: ManagedClaudeStatus["state"],
  overrides: Partial<ManagedClaudeStatus> = {}
): ManagedClaudeStatus => ({
  state,
  version: "0.10.1-tmuxdeck.1",
  standardClaudeAvailable: true,
  usingStandard: false,
  ...overrides,
});

test("Claude only interrupts the create flow when it needs a decision", () => {
  assert.strictEqual(claudeHint(claudeStatus("not-installed")), "install");
  assert.strictEqual(claudeHint(claudeStatus("needs-repair")), "repair");
  // A working setup and an unsupported platform both stay completely silent.
  assert.strictEqual(claudeHint(claudeStatus("healthy")), null);
  assert.strictEqual(claudeHint(claudeStatus("unavailable")), null);
  assert.strictEqual(claudeHint(null), null);
});

test("choosing standard Claude stops the nudge from coming back", () => {
  for (const state of ["not-installed", "needs-repair", "healthy"] as const) {
    assert.strictEqual(
      claudeHint(claudeStatus(state, { usingStandard: true })),
      null,
      `${state} must not nag after the user opted into standard Claude`
    );
  }
});

test("the chip menu always keeps a way back to enhanced messaging", () => {
  // Opting out is reversible from every state the adapter can be in.
  for (const state of ["not-installed", "needs-repair", "healthy"] as const) {
    assert.strictEqual(
      claudeSwitchTarget(claudeStatus(state, { usingStandard: true })),
      "managed",
      `${state} must still offer a route back to enhanced messaging`
    );
  }
  assert.strictEqual(claudeSwitchTarget(claudeStatus("healthy")), "standard");
});

test("the chip menu stays hidden when there is nothing to switch to", () => {
  // Nothing to fall back on, and no managed story at all off macOS.
  assert.strictEqual(
    claudeSwitchTarget(claudeStatus("healthy", { standardClaudeAvailable: false })),
    null
  );
  assert.strictEqual(claudeSwitchTarget(claudeStatus("unavailable")), null);
  assert.strictEqual(
    claudeSwitchTarget(claudeStatus("unavailable", { usingStandard: true })),
    null
  );
  assert.strictEqual(claudeSwitchTarget(null), null);
  // An unhealthy adapter is handled by the hint line, not by the quiet menu.
  assert.strictEqual(claudeSwitchTarget(claudeStatus("not-installed")), null);
  assert.strictEqual(claudeSwitchTarget(claudeStatus("needs-repair")), null);
});

test("the hint line and the chip menu never appear at the same time", () => {
  for (const state of [
    "not-installed",
    "needs-repair",
    "healthy",
    "unavailable",
  ] as const) {
    for (const usingStandard of [false, true]) {
      for (const standardClaudeAvailable of [false, true]) {
        const status = claudeStatus(state, { usingStandard, standardClaudeAvailable });
        assert.ok(
          claudeHint(status) === null || claudeSwitchTarget(status) === null,
          `${state} (usingStandard=${usingStandard}, standard=${standardClaudeAvailable}) shows two Claude affordances at once`
        );
      }
    }
  }
});

test("batch add-pane errors show only their localized sentence", () => {
  // The rollback payload nests another error code and the count payload carries a
  // value the UI cannot produce. Both are diagnostics, so neither reaches the user.
  const rollback = translateError(
    "ERR_ADD_PANES_ROLLBACK|ERR_ADD_PANE_FAILED|kill-session refused"
  );
  assert.strictEqual(rollback, t("ERR_ADD_PANES_ROLLBACK"));
  assert.ok(!rollback.includes("ERR_"), `raw code leaked: ${rollback}`);
  assert.ok(!rollback.includes("kill-session"), `diagnostics leaked: ${rollback}`);

  const count = translateError("ERR_ADD_PANES_COUNT|7");
  assert.strictEqual(count, t("ERR_ADD_PANES_COUNT"));
  assert.ok(!count.includes("7"), `internal count leaked: ${count}`);
});

test("add-pane label switches between one and several panes", () => {
  // The single-pane wording stays free of a redundant "1".
  const one = tPlural("card.addPaneWith", 1, { agent: "Pi" });
  assert.ok(!one.includes("1"), `single-pane label should not count: ${one}`);
  assert.ok(one.includes("Pi"));
  for (const n of [2, 4]) {
    const many = tPlural("card.addPaneWith", n, { agent: "Pi" });
    assert.ok(many.includes(String(n)), `expected ${n} in: ${many}`);
    assert.ok(many.includes("Pi"));
  }
});

test("add-pane plural copy keeps both placeholders in each locale", () => {
  const { en, zh } = dictionaries;
  for (const dict of [en, zh]) {
    assert.match(dict["card.addPaneWith_one"], /\{agent\}/);
    assert.match(dict["card.addPaneWith_other"], /\{agent\}/);
    assert.match(dict["card.addPaneWith_other"], /\{n\}/);
  }
});

test("either Claude backend renders as one plain agent label", () => {
  // Keyed off the agent id, so a backend rename cannot resurface the mode.
  assert.strictEqual(
    agentDisplayName({ id: "claude", name: "Claude Code · Intercom (Managed)" }),
    "Claude"
  );
  assert.strictEqual(
    agentDisplayName({ id: "claude", name: "Claude Code · Standard" }),
    "Claude"
  );
  assert.strictEqual(
    agentDisplayName({ id: "claude", name: "whatever the backend calls it next" }),
    "Claude"
  );
});

test("other agents keep their own names and translations", () => {
  assert.strictEqual(agentDisplayName({ id: "codex", name: "Codex" }), "Codex");
  // A user's custom agent name must win over the generic "agent.custom" label.
  assert.strictEqual(
    agentDisplayName({ id: "custom", name: "My Runner" }),
    "My Runner"
  );
  // Dictionary-keyed names still resolve through translateName.
  assert.strictEqual(
    agentDisplayName({ id: "shell", name: "agent.shell" }),
    t("agent.shell")
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
    "card.addPaneWith_one",
    "card.addPaneWith_other",
    "card.addPaneChoose",
    "card.addPaneRecommended",
    "card.addPaneCount",
    "card.addPaneBusy",
    "claude.hintInstall",
    "claude.hintRepair",
    "claude.enable",
    "claude.repair",
    "claude.useManaged",
    "claude.useStandard",
    "claude.modeManaged",
    "claude.modeStandard",
    "claude.modeCurrent",
    "claude.working",
    "agent.claude",
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
  assert.match(zh["claude.modeCurrent"], /\{mode\}/);
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

test("mobile vendors marked and DOMPurify inline, with provenance and no duplicate copy", () => {
  const mobileDir = path.resolve(process.cwd(), "src-tauri/mobile");
  const html = fs.readFileSync(path.join(mobileDir, "index.html"), "utf-8");

  // Pinned versions are visible in the markers, so an upgrade cannot be silent.
  assert.match(html, /BEGIN VENDOR: marked 18\.0\.9 -- SPDX-License-Identifier: MIT/);
  assert.match(html, /END VENDOR: marked 18\.0\.9/);
  assert.match(
    html,
    /BEGIN VENDOR: DOMPurify 3\.4\.13 -- SPDX-License-Identifier: Apache-2\.0/
  );
  assert.match(html, /END VENDOR: DOMPurify 3\.4\.13/);
  // Both libraries must actually be present, not just announced.
  assert.match(html, /marked v18\.0\.9/);
  assert.match(html, /DOMPurify 3\.4\.13/);

  // No external fetch: the server only answers /v1/.
  assert.doesNotMatch(html, /<script[^>]+src=/i);

  // Provenance lives in vendor/, but the code must exist in exactly one place.
  const vendorDir = path.join(mobileDir, "vendor");
  assert.ok(fs.existsSync(path.join(vendorDir, "README.md")));
  assert.ok(fs.existsSync(path.join(vendorDir, "marked.LICENSE")));
  assert.ok(fs.existsSync(path.join(vendorDir, "dompurify.LICENSE-APACHE")));
  const strayJs = fs.readdirSync(vendorDir).filter((f) => f.endsWith(".js"));
  assert.deepStrictEqual(
    strayJs,
    [],
    `vendor/ must not duplicate the inlined code: ${strayJs.join(", ")}`
  );

  // Full license texts stay out of the shipped HTML.
  assert.doesNotMatch(html, /Apache License\s*\n\s*Version 2\.0/);
});

test("mobile message rendering keeps its sanitizing pipeline", () => {
  const html = fs.readFileSync(
    path.resolve(process.cwd(), "src-tauri/mobile/index.html"),
    "utf-8"
  );

  // Raw HTML is escaped by Marked's renderer before sanitizing, so even an
  // otherwise allowed tag such as <strong> is displayed literally.
  assert.match(html, /markdownRenderer\.html\s*=\s*function/);
  assert.match(html, /return escapeRawHtml\(token && token\.text\)/);
  // Every rendered message then goes through DOMPurify with an explicit
  // allowlist as a second, independent defense.
  assert.match(html, /DOMPurify\.sanitize\(rawHtml, \{[\s\S]*?ALLOWED_TAGS/);
  for (const tag of ["img", "style", "svg", "form", "iframe", "script"]) {
    assert.match(
      html,
      new RegExp(`FORBID_TAGS:[^\\]]*'${tag}'`),
      `${tag} must be forbidden outright`
    );
  }
  // Only safe protocols keep their href, and links never hand over the opener.
  assert.match(html, /\^\(https\?:\|mailto:\)/);
  assert.match(html, /removeAttribute\('href'\)/);
  assert.match(html, /'noopener noreferrer'/);

  // Copy buttons are built as elements; message text is never concatenated in.
  assert.match(html, /createElement\('button'\)/);
  assert.doesNotMatch(html, /innerHTML\s*\+?=\s*[`'"][^`'"]*\$\{\s*(?:turn|text)\b/);
});

test("mobile list groups by backend workspace metadata only", () => {
  const html = fs.readFileSync(
    path.resolve(process.cwd(), "src-tauri/mobile/index.html"),
    "utf-8"
  );

  // Grouping and the aggregate status are real functions, not inline guesswork.
  assert.match(html, /function groupConversations\(/);
  assert.match(html, /const statusRank = /);

  // Only the backend's camelCase metadata may decide a group.
  assert.match(html, /conv\.workspaceId/);
  assert.match(html, /conv\.workspaceName/);
  // Native slot naming must never be parsed client-side.
  assert.doesNotMatch(html, /__td_slot_/);
  // Missing metadata falls into one localized bucket instead of the session name.
  assert.match(html, /__ungrouped__/);
  assert.match(html, /ungrouped:/);

  // thinking and running-tool share the "working" rank.
  assert.match(html, /'thinking':\s*1/);
  assert.match(html, /'running-tool':\s*1/);
  assert.match(html, /'awaiting-human':\s*0/);

  // Only awaiting workspaces are promoted. Other aggregate states must not
  // cause the workspace list to jump around.
  assert.match(html, /Number\(b\.awaitingCount > 0\) - Number\(a\.awaitingCount > 0\)/);
  assert.doesNotMatch(html, /groups\.sort\(function\(a, b\) \{ return a\.rank - b\.rank/);

  // Headers are operable and expose their state to assistive tech.
  assert.match(html, /aria-expanded="\$\{expanded\}"/);
  assert.match(html, /onkeydown="if\(event\.key==='Enter'\|\|event\.key===' '\)/);

  // Expansion state is in-memory only; nothing is persisted.
  assert.match(html, /this\.groupOverrides = new Map\(\)/);
  assert.doesNotMatch(html, /localStorage\.setItem\(\s*['"]tmuxdeck-groups/);
});

test("mobile labels capture output only from the authoritative transcript kind", () => {
  const html = fs.readFileSync(
    path.resolve(process.cwd(), "src-tauri/mobile/index.html"),
    "utf-8"
  );

  // Exact equality means missing and unknown values both return false.
  assert.match(
    html,
    /function isCaptureFallback\(conv\)\s*\{\s*return conv\?\.transcriptKind === 'capture';\s*\}/
  );
  // Transport availability is independent of transcript reliability and must
  // never be used as a capture heuristic again.
  const helper = html.match(
    /function isCaptureFallback\(conv\)\s*\{[\s\S]*?\n\s*\}/
  )?.[0] ?? "";
  assert.ok(helper, "expected isCaptureFallback helper");
  assert.doesNotMatch(helper, /intercomSessionId/);
});

test("mobile conversation view keeps a minimal persistent action set", () => {
  const html = fs.readFileSync(
    path.resolve(process.cwd(), "src-tauri/mobile/index.html"),
    "utf-8"
  );

  // The conversation owns one action bar; the list-level brand/status header is
  // hidden while a conversation is active.
  assert.match(html, /body\.in-conversation > header\s*\{\s*display:\s*none/);
  assert.match(html, /classList\.toggle\('in-conversation'/);
  // Workspace context remains visible and accessible inside that bar.
  assert.match(html, /class="stream-workspace"/);
  assert.match(html, /aria-label="\$\{this\.escapeAttr\(workspaceName \+ ' · ' \+ title\)\}"/);

  // The control keys live in the More sheet only; no always-on key bar.
  assert.doesNotMatch(html, /class="controls-bar"/);
  assert.match(html, /id="more-sheet"/);
  assert.match(html, /id="message-sheet"/);

  // Awaiting is a notice, not another button: Send already replies.
  assert.match(html, /notice-bar awaiting/);
  assert.doesNotMatch(html, />\s*Reply\s*</);

  // Offline exposes exactly one recovery action, and no standing Refresh.
  assert.match(html, /notice-bar offline/);
  assert.doesNotMatch(html, />\s*(Refresh|刷新)\s*</);

  // Pane ids and transport wording stay out of the conversation chrome.
  assert.doesNotMatch(html, /Kind:\s*\$\{/);
  assert.doesNotMatch(html, /<span class="conv-id">/);
});

test("scope error codes are translated in both en and zh dictionaries", () => {
  const scopeErrors = [
    "ERR_SCOPE_UNAVAILABLE",
    "ERR_SCOPE_CONFLICT",
    "ERR_SCOPE_REATTACH",
    "ERR_SCOPE_GEN_FAILED",
  ];
  for (const code of scopeErrors) {
    assert.ok(dictionaries.en[code], `Missing en translation for ${code}`);
    assert.ok(dictionaries.zh[code], `Missing zh translation for ${code}`);
    assert.notStrictEqual(translateError(code), code);
  }
});

test("frontend and mobile surfaces zero scope leakage", () => {
  const mobileHtml = fs.readFileSync(
    path.resolve(process.cwd(), "src-tauri/mobile/index.html"),
    "utf-8"
  );
  assert.doesNotMatch(mobileHtml, /AGENT_INTERCOM_SCOPE_ID/);
  assert.doesNotMatch(mobileHtml, /scopeId/i);
  assert.doesNotMatch(mobileHtml, /workspaceScope/i);

  const typesContent = fs.readFileSync(
    path.resolve(process.cwd(), "src/types.ts"),
    "utf-8"
  );
  assert.doesNotMatch(typesContent, /AGENT_INTERCOM_SCOPE_ID/);
  assert.doesNotMatch(typesContent, /scope_id/i);
  assert.doesNotMatch(typesContent, /scopeId/i);
});

test("reorderPaneAgentsForLead moves the designated lead to index 0 and preserves order of others", () => {
  // Lead is already index 0
  assert.deepStrictEqual(
    reorderPaneAgentsForLead(["pi", "claude", "codex"], 0),
    ["pi", "claude", "codex"]
  );

  // Promoting index 1 (claude) to lead
  assert.deepStrictEqual(
    reorderPaneAgentsForLead(["pi", "claude", "codex"], 1),
    ["claude", "pi", "codex"]
  );

  // Promoting index 2 (codex) to lead in mixed pane configs
  assert.deepStrictEqual(
    reorderPaneAgentsForLead(["pi", "claude", "codex", "shell"], 2),
    ["codex", "pi", "claude", "shell"]
  );

  // Duplicate pane configs before preallocation are equivalent; promoting pane 2 produces a new array where index 0 is still pi
  const dupes = ["pi", "pi", "pi"];
  const reorderedDupes = reorderPaneAgentsForLead(dupes, 2);
  assert.deepStrictEqual(reorderedDupes, ["pi", "pi", "pi"]);
  assert.notStrictEqual(reorderedDupes, dupes, "must return a new array instance");

  // Invalid index handling: negative, out of bounds, fractional (e.g. 1.5), NaN returns new array copy without mutation
  const original = ["pi", "claude", "codex"];
  assert.deepStrictEqual(reorderPaneAgentsForLead(original, -1), ["pi", "claude", "codex"]);
  assert.deepStrictEqual(reorderPaneAgentsForLead(original, 5), ["pi", "claude", "codex"]);
  assert.deepStrictEqual(reorderPaneAgentsForLead(original, 1.5), ["pi", "claude", "codex"]);
  assert.deepStrictEqual(reorderPaneAgentsForLead(original, 0.7), ["pi", "claude", "codex"]);
  assert.deepStrictEqual(reorderPaneAgentsForLead(original, NaN), ["pi", "claude", "codex"]);
  assert.notStrictEqual(reorderPaneAgentsForLead(original, -1), original, "must return a new array instance");
  assert.deepStrictEqual(reorderPaneAgentsForLead([], 0), []);
});

test("adapter consent action copy, source enums and error codes in both locales", () => {
  const samplePlan: WorkspaceInstallPlan = {
    planId: "opaque-plan-1",
    planFingerprint: "fingerprint-abc",
    requiresConsent: true,
    canApply: false,
    canCreateWithoutInstalling: false,
    healthyAgentIds: ["pi"],
    items: [
      {
        agentId: "claude",
        hostDisplayName: "Claude Code",
        adapterKind: "claude-plugin-monitor",
        state: "not-installed",
        targetVersion: "0.13.0-connect.1",
        installedVersion: null,
        sourceKind: "bundled",
        configChangeKind: "app-private-managed",
        networkRequired: false,
        license: "AGPL-3.0-or-later",
        actionReason: "install",
      },
      {
        agentId: "codex",
        hostDisplayName: "OpenAI Codex",
        adapterKind: "codex-mcp",
        state: "incompatible-namespace",
        targetVersion: "0.12.0-connect.1",
        installedVersion: "0.9.0",
        sourceKind: "npm-registry",
        packageName: "@ctliz/agent-intercom-codex",
        configChangeKind: "host-config-registered",
        networkRequired: true,
        license: "AGPL-3.0-or-later",
        actionReason: "manual-migration-required",
      },
    ],
  };
  assert.strictEqual(samplePlan.requiresConsent, true);
  assert.strictEqual(samplePlan.canApply, false);
  assert.strictEqual(samplePlan.canCreateWithoutInstalling, false);
  assert.strictEqual(samplePlan.items[0].adapterKind, "claude-plugin-monitor");
  assert.strictEqual(samplePlan.items[0].sourceKind, "bundled");
  assert.strictEqual(samplePlan.items[1].sourceKind, "npm-registry");
  assert.strictEqual(samplePlan.items[1].actionReason, "manual-migration-required");

  assert.strictEqual(t("consent.actionInstall"), "Install adapter & create");
  assert.strictEqual(t("consent.actionWithout"), "Create without installing");
  assert.strictEqual(t("consent.actionCancel"), "Cancel");
  assert.strictEqual(t("consent.offlineBundle"), "Bundled offline");
  assert.strictEqual(t("consent.source.bundled"), "Bundled offline asset");
  assert.strictEqual(t("consent.source.npmRegistry", { pkg: "@ctliz/agent-intercom-codex" }), "npm: @ctliz/agent-intercom-codex");
  assert.strictEqual(t("consent.actionReason.manualMigration"), "Manual migration required");

  const adapterErrors = [
    "ERR_PLAN_STALE",
    "ERR_ADAPTER_INSTALL_FAILED",
    "ERR_ADAPTER_NOT_FOUND",
    "ERR_ADAPTER_INCOMPATIBLE_NS",
  ];
  for (const code of adapterErrors) {
    assert.ok(dictionaries.en[code], `Missing en translation for ${code}`);
    assert.ok(dictionaries.zh[code], `Missing zh translation for ${code}`);
    assert.notStrictEqual(translateError(code), code);
  }
});

test("AdapterConsentModal and CreateWorkspaceModal surface zero scope leakage", () => {
  const consentModalSrc = fs.readFileSync(
    path.resolve(process.cwd(), "src/components/AdapterConsentModal.tsx"),
    "utf-8"
  );
  assert.doesNotMatch(consentModalSrc, /AGENT_INTERCOM_SCOPE_ID/);
  assert.doesNotMatch(consentModalSrc, /scope_id/i);
  assert.doesNotMatch(consentModalSrc, /scopeId/i);

  const createModalSrc = fs.readFileSync(
    path.resolve(process.cwd(), "src/components/CreateWorkspaceModal.tsx"),
    "utf-8"
  );
  assert.doesNotMatch(createModalSrc, /AGENT_INTERCOM_SCOPE_ID/);
  assert.doesNotMatch(createModalSrc, /scope_id/i);
  assert.doesNotMatch(createModalSrc, /scopeId/i);
});
