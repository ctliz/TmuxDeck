import assert from "node:assert";
import { test } from "node:test";
import { t, tPlural, translateError } from "./i18n.ts";
import { reorderIds, sanitizeNameFrontend } from "./utils.ts";
import type { TmuxSession, TmuxPane } from "./types.ts";

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
