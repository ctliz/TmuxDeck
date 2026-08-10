# scripts

开发与验证用的一次性脚本。不参与构建，不打进发布包。

---

## `intercom-probe.mjs`

验证 TmuxDeck 能否作为「人类适配器」接入 pi-intercom broker。
**在写任何依赖 intercom 的功能之前，先跑通这个。**

协议实现对齐上游 `types.ts` 与 `broker/framing.ts`，
细节见 [`docs/REFERENCE-intercom-protocol.md`](../docs/REFERENCE-intercom-protocol.md)。

### 前置

至少有一个装了 `pi-intercom` 的 pi 会话正在运行——broker 由它拉起，
最后一个会话退出 5 秒后 broker 会自行关闭。

```sh
ls ~/.pi/agent/intercom/broker.sock   # 存在即 broker 在跑
```

### 用法

```sh
# 注册并常驻：列出所有在线会话，打印收到的消息
node scripts/intercom-probe.mjs

# 发一条消息后退出
node scripts/intercom-probe.mjs send <目标会话名或ID> "消息内容"
```

### 验证清单

| # | 步骤 | 通过标准 |
|---|---|---|
| 1 | `node scripts/intercom-probe.mjs` | 打印 `✓ 注册成功 sessionId=…` |
| 2 | 观察会话列表 | 每个 pi 会话都带状态（`idle` / `thinking` / `tool:…`） |
| 3 | 在任一 pi 会话执行 `intercom({ action: "list" })` | 列表里能看到 `tmuxdeck-probe` |
| 4 | 在 pi 里 `intercom({ action: "send", to: "tmuxdeck-probe", message: "hi" })` | 探针打印 `📨 来自 …` |
| 5 | 在 pi 里改用 `action: "ask"` | 探针打印 `⚠ 对方在等回复（ask）` |
| 6 | `node scripts/intercom-probe.mjs send <pi会话名> "收到"` | 打印 `✓ 已送达`，且 pi 会话里出现该消息 |

第 2 条通过即证明**不需要**自己实现状态判定；
第 4、5 条通过即证明通知链路成立；第 6 条通过即证明手机回复链路成立。

三条都通，`src-tauri/src/bridge.rs` 的全部假设即成立。

### 常见结果

| 现象 | 原因 |
|---|---|
| `✗ 找不到 broker socket` | 没有 pi 会话在跑，或 broker 已因空闲退出 |
| 会话列表只有自己 | 其他 pi 会话没装 `pi-intercom`，或装后未 `/reload` |
| 看不到 Claude Code / Codex | 本机装的是 pi-only 原版；跨 harness 需迁移到 `dataforxyz` 家族 |
| `✗ 投递失败: …` | 目标名重复或不存在——重名时应改用会话 ID |
