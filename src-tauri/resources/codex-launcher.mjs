#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const manifestPath = process.env.AGENT_INTERCOM_TEAM_MANIFEST;
if (!manifestPath) {
  process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_TEAM_MANIFEST environment variable not set\n");
  process.exit(1);
}

let manifest;
try {
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (e) {
  process.stderr.write("LAUNCHER_ERROR: Failed to read team manifest: " + e.message + "\n");
  process.exit(1);
}

const mySessionId = process.env.AGENT_INTERCOM_SESSION_ID;
if (!mySessionId) {
  process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_SESSION_ID not set\n");
  process.exit(1);
}

const member = (manifest.members || []).find(m => m.sessionId === mySessionId);
if (!member) {
  process.stderr.write("LAUNCHER_ERROR: Session ID not found in manifest members\n");
  process.exit(1);
}

const envRole = process.env.AGENT_INTERCOM_ROLE;
if (envRole && envRole !== member.role) {
  process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_ROLE mismatch\n");
  process.exit(1);
}

if (member.role === "worker") {
  const managerTarget = process.env.AGENT_INTERCOM_MANAGER_TARGET;
  if (!managerTarget || managerTarget !== manifest.leadId) {
    process.stderr.write("LAUNCHER_ERROR: AGENT_INTERCOM_MANAGER_TARGET mismatch for worker\n");
    process.exit(1);
  }
}

const argv = process.argv.slice(2);
const codexBin = process.env.CODEX_REAL_BIN || "/usr/local/bin/codex";

try {
  execFileSync(codexBin, argv, { stdio: "inherit", env: process.env });
} catch (e) {
  if (e.status !== undefined) {
    process.exit(e.status);
  }
  process.exit(1);
}
