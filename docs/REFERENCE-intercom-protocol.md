# pi-intercom 线协议参考

> 这份文档最初从 `nicobailon/pi-intercom` 反推整理，现已按本机
> `@dataforxyz/agent-intercom-*` protocol v3 的 `types.ts`、`broker.ts` 与
> `broker/framing.ts` 更新。**写下来是为了不必再推一遍。**
>
> 实现见 `src-tauri/src/intercom.rs`，验证脚本见 `scripts/intercom-probe.mjs`。

---

## 传输

| 平台 | 传输 |
|---|---|
| macOS / Linux | Unix domain socket |
| Windows | 命名管道（TmuxDeck 尚未实现） |

socket 路径：

```
$PI_CODING_AGENT_DIR/intercom/broker.sock   （若该环境变量已设置）
~/.pi/agent/intercom/broker.sock            （默认）
```

**broker 的生命周期不归我们管**：它由第一个 intercom 会话自动拉起，
最后一个会话断开 5 秒后自行退出。因此「socket 不存在」是常态而非错误，
调用方应降级到 `send-keys` 通道，而不是尝试拉起 broker。

---

## 分帧

```
┌────────────────┬─────────────────────────┐
│ 4 字节大端长度  │  UTF-8 JSON（长度即此段） │
└────────────────┴─────────────────────────┘
```

单帧上限 **1 MiB**，超限方应报错并断开。注意 TCP/UDS 会拆包粘包，
读取端必须做重组——`intercom-probe.mjs` 和 `intercom.rs` 都实现了。

---

## 客户端 → broker

| `type` | 关键字段 | 说明 |
|---|---|---|
| `register` | `protocol: "pi-intercom"`、`version: 3`、`session`（见下）、`sessionId?`、`stateId?` | 连上后第一件事；版本不匹配会断开 |
| `unregister` | `preserveAsks?` | 优雅退出 |
| `list` | `requestId` | 请求会话列表，异步经 `sessions` 返回 |
| `send` | `to`、`message` | `to` 可以是会话名或会话 ID |
| `message_received` | `deliveryId` | 收方持久入队后确认本次 delivery |
| `message_rejected` | `deliveryId`、`code`、`reason` | 收方拒绝冲突 delivery |
| `presence` | `status?`、`name?`、`model?` … | 更新自身状态 |
| `cancel_message` / `cancel_ask` | `messageId` | 撤回 |
| `extension_publish` / `extension_state_commit` | `namespace` … | 扩展总线，TmuxDeck 未用 |

### register 的 session 字段

```jsonc
{
  "name": "me",          // 其他会话据此寻址
  "cwd": "/path",        // 展示元数据
  "model": "human",      // 填 human 让别人一眼看出这是人不是 agent
  "pid": 12345,          // 关联 pane 的关键：需沿父链上溯匹配 pane_pid
  "startedAt": 1754870400000,
  "lastActivity": 1754870400000,
  "status": "idle"
}
```

> `cwd` / `model` / `pid` / `status` 都是**展示元数据，不构成身份认证**。
> broker 的信任边界是「同一 OS 用户」，不是密码学主体。

---

## broker → 客户端

| `type` | 关键字段 | 说明 |
|---|---|---|
| `registered` | `sessionId`、`protocol`、`version` | 注册成功，拿到自己的会话 ID |
| `sessions` | `requestId`、`sessions[]` | `list` 的应答 |
| `message` | `deliveryId`、`from`、`message` | 收到一条消息；处理后必须按 `deliveryId` ACK |
| `presence_update` | `session` | 某会话状态变了 |
| `session_joined` / `session_left` | `session` / `sessionId` | 上下线 |
| `delivery_accepted` | `messageId`、`deliveryId` | broker 已接纳并等待收方确认 |
| `delivered` | `messageId`、`deliveryId` | 收方已确认持久接收 |
| `delivery_failed` | `messageId`、`code`、`reason`、`retryable` | 投递失败 |
| `error` | `error` | broker 报错 |
| `message_control` / `extension_*` | — | TmuxDeck 未消费 |

**入站解析必须容忍未知 `type`**：上游协议在演进（跨 harness 分支已到 v3，
新增了若干帧类型）。`intercom.rs` 因此手工分派而非使用 serde 内部标记枚举——
遇到未知类型忽略即可，不会导致整条连接反序列化失败。

---

## SessionInfo

```jsonc
{
  "id": "20d43841…",     // 稳定会话 ID，寻址的可信键
  "name": "planner",     // 可重名；重名时发送会失败，应改用 id
  "cwd": "/projects/api",
  "model": "claude-sonnet-4",
  "pid": 12345,
  "startedAt": 1754870400000,
  "lastActivity": 1754870400000,
  "status": "thinking",  // 见下
  "contextPct": 43       // 上下文占用百分比，可能缺失
}
```

### status —— 四态判定的事实来源

| 值 | 含义 |
|---|---|
| `idle` | 空闲，可接收输入 |
| `thinking` | 模型正在生成 |
| `tool:<name>` | 正在执行某个工具 |
| 缺失 / 其他 | 未知（不要猜） |

由各会话在 pi 生命周期事件中**自动上报**。

> 这一条就取消了「轮询 capture-pane + 内容 hash 比对 + 静默启发式」的全部必要性。
> 不要再实现那套东西。

---

## Message

```jsonc
{
  "id": "m-1",
  "timestamp": 1754870400000,
  "replyTo": "m-0",       // 回复某条消息；收方据此匹配到对应的 ask
  "expectsReply": true,   // 对方在 ask，正阻塞等待 ← 最高优先级信号
  "content": {
    "text": "需要你确认",
    "attachments": [
      { "type": "snippet", "name": "auth.ts", "language": "typescript", "content": "…" }
    ]
  }
}
```

`attachments.type` 取值：`file` / `snippet` / `context`。

**`expectsReply: true` 是手机端唯一应当触发推送的信号**——它意味着有 agent
正阻塞等你回话，而不只是发了条通知。

---

## 投递语义

broker 负责寻址、ask 边和 delivery 状态；各 Harness 适配器负责持久入队，并在安全时机注入目标会话。所以**不要**为了怕打断 agent 而在 TmuxDeck 再实现一套投递时机判断——直接 `send` 即可。这正是 intercom 优于 `send-keys` 直塞字符的核心原因（后者会被正在思考的 TUI 吞掉或打断）。

v3 投递分两步：`delivery_accepted` 只表示 broker 已接纳；收方必须在处理 `message` 后发送 `message_received { deliveryId }`，发送方才会收到 `delivered`。不能用业务 `message.id` 代替 `deliveryId`。

### 回复必须来自收到 ask 的那个 session

broker 对 `replyTo` 的校验是 **sessionId 级身份**（`broker.ts`：
`replyEdge.to !== currentId` 即返回 `delivery_failed`）：

- 新开一条连接重新注册（即使同名）会拿到新的 sessionId，**回不了同一个 ask**；
- 回复只能在**收到 ask 的那条连接**上发出（`intercom.rs` 的 `reply()` 正是如此——
  同一持久连接、同一 sessionId）；
- 实测：独立进程带 `replyTo` 发送被拒，报 `Reply target does not match the pending ask`。

对手机端的影响：TmuxDeck 必须保持单一持久连接，回复永远走它，
不能为每条回复临时建连接。

---

## 两个分支的差异

本机当前已统一安装 **`@dataforxyz/agent-intercom-*` 0.10.0 跨 Harness 适配器**，覆盖 Pi / Codex / Claude Code / OpenCode。下表保留与 `nicobailon/pi-intercom` 原版（pi-only）的历史差异，便于排查旧环境：

| | 原版 | 跨 harness 版 |
|---|---|---|
| 支持的 agent | 仅 pi | Pi、Codex、Claude Code、OpenCode |
| 运行时文件 | `broker.sock` `broker.pid` `config.json` | 另有 `broker.owner`、`broker-asks.json`、`inbox/`、`outbox/` |
| 投递持久化 | 无（仅存 pi 会话历史） | 持久化 inbox/outbox + ACK + 断线重放 |
| `ask` 语义 | 客户端硬阻塞 10 分钟 | 软等 30 秒后转异步，10 分钟内可迟回 |
| 工具形态 | 单个 `intercom({action})` | 拆分为 `intercom_send` / `_ask` / `_reply` / … |
| 许可证 | MIT | AGPL-3.0-or-later |

**迁移是全有或全无**：上游明确警告新旧适配器混用会分裂成互不可见的
broker「岛」，必须全部升级并在每个会话 `/reload`。

> 对手机场景而言跨 harness 版明显更合适：人不在电脑前时，
> `ask` 硬阻塞 10 分钟是很差的语义。

### 许可证注意

按线协议自行实现客户端**不构成衍生作品**；`intercom.rs` 即为独立实现，
未复制上游任何源码。若将来需要修改上游适配器本身，则受 AGPL 约束。

---

## 上游

- [nicobailon/pi-intercom](https://github.com/nicobailon/pi-intercom)（MIT，pi-only）
- [dataforxyz/agent-intercom-pi](https://github.com/dataforxyz/agent-intercom-pi)（AGPL，跨 harness）
