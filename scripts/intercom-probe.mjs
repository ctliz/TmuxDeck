#!/usr/bin/env node
/**
 * intercom-probe — 验证 TmuxDeck 能否作为一个"人类适配器"接入 pi-intercom broker。
 *
 * 协议来自 nicobailon/pi-intercom：
 *   传输  Unix domain socket  ~/.pi/agent/intercom/broker.sock
 *   分帧  4 字节大端长度 + UTF-8 JSON
 *
 * 用法：
 *   node intercom-probe.mjs                      # 注册并常驻，列出会话、打印收到的消息
 *   node intercom-probe.mjs send <目标> <消息>    # 注册、发一条、退出
 *
 * 验证目标（跑通即证明适配器路线成立）：
 *   1. 能连上 broker 并注册成功            → 拿到 sessionId
 *   2. list 能拿到真实会话与实时状态        → 替代 capture-pane 轮询与四态判定
 *   3. 其他 pi 会话能看到 "tmuxdeck-probe" → 人可以成为总线上的一个地址
 *   4. 从 pi 发消息给它能收到              → 通知链路成立
 *   5. 它发消息给 pi 会话能送达            → 手机回复链路成立
 */

import net from "node:net";
import os from "node:os";
import path from "node:path";
import fs from "node:fs";
import crypto from "node:crypto";

const SESSION_NAME = "tmuxdeck-probe";

const runtimeDir = process.env.PI_CODING_AGENT_DIR
  ? path.join(process.env.PI_CODING_AGENT_DIR, "intercom")
  : path.join(os.homedir(), ".pi", "agent", "intercom");
const sockPath = path.join(runtimeDir, "broker.sock");

if (!fs.existsSync(sockPath)) {
  console.error(`✗ 找不到 broker socket：${sockPath}`);
  console.error("  说明 broker 没在跑。先开一个装了 pi-intercom 的 pi 会话，再重试。");
  process.exit(1);
}

// ── 分帧 ──────────────────────────────────────────────────────────────
function writeMessage(socket, msg) {
  const json = JSON.stringify(msg);
  const len = Buffer.byteLength(json, "utf-8");
  const frame = Buffer.allocUnsafe(4 + len);
  frame.writeUInt32BE(len, 0);
  frame.write(json, 4, len, "utf-8");
  socket.write(frame);
}

function createReader(onMessage) {
  let buf = Buffer.alloc(0);
  return (chunk) => {
    buf = Buffer.concat([buf, chunk]);
    while (buf.length >= 4) {
      const len = buf.readUInt32BE(0);
      if (buf.length < 4 + len) break;
      const payload = buf.subarray(4, 4 + len);
      buf = buf.subarray(4 + len);
      try {
        onMessage(JSON.parse(payload.toString("utf-8")));
      } catch (e) {
        console.error("✗ 解析帧失败:", e.message);
      }
    }
  };
}

// ── 参数 ──────────────────────────────────────────────────────────────
const [, , cmd, target, ...rest] = process.argv;
const sendMode = cmd === "send";
const sendText = rest.join(" ");

if (sendMode && (!target || !sendText)) {
  console.error("用法: node intercom-probe.mjs send <目标会话名或ID> <消息内容>");
  process.exit(1);
}

// ── 连接 ──────────────────────────────────────────────────────────────
const socket = net.createConnection(sockPath);
const listRequestId = crypto.randomUUID();
let mySessionId = null;

socket.on("connect", () => {
  console.log(`✓ 已连接 ${sockPath}`);
  const now = Date.now();
  writeMessage(socket, {
    type: "register",
    session: {
      name: SESSION_NAME,
      cwd: process.cwd(),
      model: "human",          // 这里就是"人"这个身份
      pid: process.pid,
      startedAt: now,
      lastActivity: now,
      status: "idle",
    },
  });
});

const fmtStatus = (s) => {
  if (!s) return "?";
  if (s === "idle") return "○ idle";
  if (s === "thinking") return "● thinking";
  if (s.startsWith("tool:")) return `◐ ${s}`;
  return s;
};

socket.on(
  "data",
  createReader((msg) => {
    switch (msg.type) {
      case "registered": {
        mySessionId = msg.sessionId;
        console.log(`✓ 注册成功  sessionId=${msg.sessionId}`);
        if (msg.features?.length) console.log(`  broker features: ${msg.features.join(", ")}`);

        if (sendMode) {
          const id = crypto.randomUUID();
          writeMessage(socket, {
            type: "send",
            to: target,
            message: { id, timestamp: Date.now(), content: { text: sendText } },
          });
          console.log(`→ 发往 ${target}: ${sendText}`);
        } else {
          writeMessage(socket, { type: "list", requestId: listRequestId });
        }
        break;
      }

      case "sessions": {
        if (msg.requestId !== listRequestId) return;
        console.log(`\n── 在线会话 (${msg.sessions.length}) ──`);
        for (const s of msg.sessions) {
          const me = s.id === mySessionId ? " ← 我" : "";
          const ctx = s.contextPct != null ? `  ctx ${s.contextPct}%` : "";
          console.log(
            `  ${fmtStatus(s.status).padEnd(16)} ${(s.name ?? "(未命名)").padEnd(22)} ` +
              `${s.id.slice(0, 8)}  ${s.model}${ctx}`,
          );
          console.log(`  ${"".padEnd(16)} ${s.cwd}${me}`);
        }
        console.log(
          `\n常驻中。在任一 pi 会话里执行以下命令，看这里能否收到：\n` +
            `  intercom({ action: "send", to: "${SESSION_NAME}", message: "hello from pi" })\n` +
            `Ctrl-C 退出。\n`,
        );
        break;
      }

      case "message": {
        const from = msg.from;
        console.log(`\n📨 来自 ${from.name ?? from.id.slice(0, 8)} (${from.cwd})`);
        if (msg.message.expectsReply) console.log("   ⚠ 对方在等回复（ask）");
        console.log(`   ${msg.message.content.text}`);
        for (const a of msg.message.content.attachments ?? []) {
          console.log(`   [附件 ${a.type}] ${a.name}`);
        }
        // 回执：告诉发送方我们确实收到了
        writeMessage(socket, {
          type: "message_receipt",
          receipt: { messageId: msg.message.id, status: "receiver_received", timestamp: Date.now() },
        });
        console.log(`   ↩ 回复： node intercom-probe.mjs send ${from.name ?? from.id} "..."`);
        break;
      }

      case "presence_update":
        console.log(`  · ${msg.session.name ?? msg.session.id.slice(0, 8)} → ${fmtStatus(msg.session.status)}`);
        break;

      case "session_joined":
        console.log(`  + 上线 ${msg.session.name ?? msg.session.id.slice(0, 8)}`);
        break;

      case "session_left":
        console.log(`  - 下线 ${msg.sessionId.slice(0, 8)}`);
        break;

      case "delivered":
        console.log(`✓ 已送达 (${msg.messageId.slice(0, 8)})`);
        if (sendMode) shutdown(0);
        break;

      case "delivery_failed":
        console.error(`✗ 投递失败: ${msg.reason}`);
        if (sendMode) shutdown(1);
        break;

      case "message_receipt":
        console.log(`  ✓ 回执 ${msg.receipt.status} (${msg.receipt.messageId.slice(0, 8)})`);
        break;

      case "error":
        console.error(`✗ broker 报错: ${msg.error}`);
        break;

      default:
        console.log(`  (未处理的帧: ${msg.type})`);
    }
  }),
);

function shutdown(code) {
  try {
    writeMessage(socket, { type: "unregister" });
  } catch {}
  socket.end();
  setTimeout(() => process.exit(code), 100);
}

socket.on("error", (e) => {
  console.error(`✗ socket 错误: ${e.message}`);
  process.exit(1);
});
socket.on("close", () => {
  console.log("连接已关闭");
  process.exit(0);
});
process.on("SIGINT", () => {
  console.log("\n注销中…");
  shutdown(0);
});
