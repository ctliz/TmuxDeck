import assert from "node:assert";
import { test } from "node:test";
import { sanitizeNameFrontend } from "./utils.ts";

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
