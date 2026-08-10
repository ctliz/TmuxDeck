# TmuxDeck v1.12 对话桥：intercom 接入 + 手机端多路对话

> 目标：**在手机上同时跟多个 pane 里的 agent 对话**，并能把 A 的内容转给 B。
> 不是终端模拟，不是通知收件箱，而是一组可以并行进行的对话——每个 pane 是一个对话对象。
> 传输是持久连接（WebSocket 形态），不是「推一条通知」的单向通道。

---

## 0. 决策记录

| 项 | 决定 | 理由 |
|---|---|---|
| 总线 | **复用 pi-intercom broker，不自造** | 已有跨 harness 家族，且已解决状态、排队投递、回执 |
| TmuxDeck 定位 | **intercom 的「人类适配器」** | 家族里 Pi/Codex/Claude/OpenCode 都有适配器，唯独没有「人」 |
| 手机端形态 | 多路对话客户端（持久连接） | 用户诉求：能进每个 pane 聊，也能跨 pane 转发 |
| 推送通道 | **先只做抽象层**（`Transport` trait） | 具体接 ntfy / 飞书 / TG 待定，不提前绑定 |
| ~~四态判定 / 静默启发式~~ | **删除** | broker 直接给 `idle` / `thinking` / `tool:<name>`，是事实不是猜测 |
| ~~自造消息总线~~ | **撤销** | 见 `PRIOR-ART-agent-bus.md` |

调研依据见 [`PRIOR-ART-agent-bus.md`](./PRIOR-ART-agent-bus.md)。

---

## 1. 架构

```
┌──────────────────── TmuxDeck ────────────────────┐
│                                                   │
│  React 桌面 UI ──invoke──┐                        │
│                          ├──▶ tmux.rs ──▶ tmux    │
│  bridge.rs（对话桥）──────┘                        │
│      │                                            │
│      ├── ConversationRegistry                     │
│      │     pane 清单 ⊕ intercom 会话 → 对话表      │
│      │                                            │
│      ├── intercom.rs ──unix socket──▶ broker.sock │
│      │     注册为 "me"，收发定向消息、订阅状态      │
│      │                                            │
│      └── Transport（trait）──▶ 手机端              │
└───────────────────────────────────────────────────┘
```

三条数据通路，各有各的来源：

| 用途 | 来源 | 状态 |
|---|---|---|
| 有哪些对话、各自什么状态 | broker 注册表 + `tmux list-panes -a` | ✅ 已实现 |
| 我 → agent | `intercom send`（优先）/ `send-keys`（兜底） | ✅ 已实现 |
| agent → 我 | `TranscriptSource` | ⚠️ 见第 4 节，唯一未定 |

---

## 2. 已实现（本次）

### `tmux.rs`

- `list_all_panes()` — 一次 `list-panes -a` 拿全部 pane 的 session / 进程 / cwd
- `send_keys(pane, text, submit)` — 自由文本走 `-l` literal 通道，多行逐行发送
- `send_key_name(pane, key)` — 控制键**白名单**通道（`Escape` / `C-c` / 方向键等）

> 两条通道刻意分开：不分开的话，消息里出现 "C-c" 会被 tmux 当控制键执行。

### `intercom.rs`

pi-intercom broker 客户端，对齐上游 `types.ts` 与 `broker/framing.ts`：

- 传输 Unix domain socket，分帧 4 字节大端长度 + JSON，单帧上限 1 MiB
- `connect()` 注册为 `me`（`model: "human"`，其他会话在 list 里一眼看出这是人）
- `request_list` / `send` / `reply` / `acknowledge` / `update_presence`
- 独立读线程 → `mpsc::Receiver<IntercomEvent>`
- **入站帧手工分派**：遇到未知类型忽略而非报错。上游协议在演进
  （dataforxyz 分支已到 v3），容忍未知是必需的
- 无新增依赖（复用已有的 serde / serde_json / dirs）

### `bridge.rs`

- `AgentKind::from_command` — 从 `pane_current_command` 识别 agent 类型；
  **agent 执行工具时进程名会临时变成 `bash`，此时不把 kind 打回 Shell**
- `ConversationRegistry` — pane 表 ⊕ intercom 会话表 → 统一对话表，
  `list()` 按「等人的排最前」排序
- **pane ↔ intercom 会话关联**：intercom 报的是 agent 进程 pid，
  tmux 的 `pane_pid` 通常是那个 shell，所以沿父进程链上溯匹配（最多 12 层，防环）
- `deliver()` — 有 intercom 走 broker（忙时排队、不打断思考中的 agent），否则退回 send-keys
- `forward()` — 跨对话转发，自动加来源标注
- `Transport` / `ClientEvent` / `ClientCommand` — 手机端传输抽象 + `LogTransport`

---

## 3. 手机端协议（已定义，待接传输）

事件（服务端 → 手机）：

```jsonc
{ "type": "conversations",   "items": [ /* Conversation[] */ ] }
{ "type": "status-changed",  "id": "%3", "status": "awaiting-human" }
{ "type": "turn",            "turn": { "conversationId": "%3", "role": "agent", "text": "…" } }
{ "type": "awaiting-human",  "id": "%3", "title": "backend", "preview": "…", "replyTo": "m-1" }
```

指令（手机 → 服务端）：

```jsonc
{ "type": "say",     "id": "%3", "text": "继续" }
{ "type": "key",     "id": "%3", "key": "Escape" }
{ "type": "forward", "from": "%1", "to": "%3", "text": "…" }
{ "type": "refresh" }
```

`Conversation.status` 取值：`idle` / `thinking` / `running-tool` / `awaiting-human` / `unknown`。
其中 `awaiting-human` 来自 intercom 消息的 `expectsReply`——**对方正阻塞等你回话**，
这是手机端唯一应当触发推送的信号。

---

## 3.5 让 agent 知道「可以找人」

技术链路通了不等于会被用到——**agent 得知道 `me` 这个地址存在，以及什么时候该用。**
这一步是纯文档动作，但没有它整个功能不会自己发生。

在各项目的 `AGENTS.md` / `CLAUDE.md` 里加入：

```xml
<intercom-human>
本机有一个名为 `me` 的 intercom 会话，它是人（TmuxDeck 的手机端）。

**什么时候找 me：**
- 被卡住且无法自行决定（需要产品判断、需要授权、方案有分歧）
- 要做不可逆的操作前（删数据、改线上配置、force push）
- 任务完成，且后续方向需要人来定

**什么时候不要找：**
- 能自己查清楚的事
- 例行进度播报
- 另一个 agent 就能回答的问题——先用 intercom 问它

**用哪个：** 需要等答复用 `ask`（人会收到推送）；只是知会用 `send`。
</intercom-human>
```

> `ask` 与 `send` 的区别在手机端是**是否推送**：`ask` 意味着有 agent 正阻塞
> 等你回话，会推送；`send` 只在对话里留一条未读。让 agent 用对，通知才不会变成噪音。

---

## 4. 唯一未定：对话内容从哪来

「有哪些对话、状态如何」和「我怎么说话」都已解决。剩下的是
**agent 说了什么**——这需要拿到分轮次的对话内容，三个候选：

| 方案 | 可行性 | 问题 |
|---|---|---|
| `capture-pane` | 已实现为兜底（`CapturePaneSource`） | 只有当前屏幕，无历史，TUI 重绘导致内容抖动，没有轮次边界 |
| `pipe-pane` 抓原始流 | 拿得到全部字节 | 混着大量光标移动与重绘转义序列，还原「谁说了什么」极难 |
| **读 agent 自己的结构化会话记录** | **推荐** | 本身就是干净的分轮次数据（如 Claude Code 的 `~/.claude/projects/**/*.jsonl`）；代价是每个 agent 一个读取器，且要把 pane 关联到对应记录文件 |

`TranscriptSource` trait 已就位，实现待定。建议按方案 3 做主路径、方案 1 兜底。

> 关联问题其实已经解决了一半：`bridge.rs` 的父进程链上溯能把 pane 关到 agent 进程，
> 拿到 pid 与 cwd 之后，定位该 agent 的会话记录文件是可做的。

---

## 5. 前置：本机 intercom 版本

`ls ~/.pi/agent/intercom/` 的结果显示当前装的是 **`nicobailon/pi-intercom` 原版（pi-only）**，
不是 `dataforxyz` 的跨 harness 分支：

| 文件 | 本机 | 原版 | 跨 harness 版 |
|---|---|---|---|
| `broker.sock` / `broker.pid` / `extension-state` | ✅ | ✅ | ✅ |
| `broker.owner` | ❌ | 无 | 有 |
| `inbox/` `outbox/` `broker-asks.json` | ❌ | 无 | 有 |

**影响**：现在只有 pi 会话能接入总线，Claude Code / Codex 仍是孤岛，
只能走 `send-keys` 兜底。要打通需整体迁移到 dataforxyz 家族——
上游明确警告新旧适配器混用会分裂成互不可见的 broker「岛」，**必须全部升级并 `/reload`**。

两版还有个对手机场景很关键的差异：原版 `ask` 是客户端硬阻塞 10 分钟、
消息只存在 pi 会话历史里；新版有持久化 inbox/outbox、ACK、断线重放，
`ask` 改为软等 30 秒转异步。**人不在电脑前时，后者的语义明显更合适。**

---

## 6. 验收

已可验证（`cargo test`）：

- [ ] `send_keys` 对 `%abc`、空文本、超 8 KiB 分别返回对应错误码
- [ ] `send_key_name` 拒绝白名单外的键
- [ ] intercom 分帧读写往返一致，长度前缀为 4 字节大端
- [ ] 未知类型的 broker 帧被忽略而非导致错误
- [ ] agent 执行工具时 `AgentKind` 不被打回 `Shell`
- [ ] 对话列表把 `awaiting-human` 排在最前
- [ ] pane 消失后对话表与 intercom 映射同步清理

需真机验证（先跑 `scripts/intercom-probe.mjs`）：

- [ ] 探针能连上 broker 并注册成功
- [ ] 其他 pi 会话的 `intercom list` 里能看到我们
- [ ] pi 发给我们的消息能收到，`expectsReply` 能正确识别
- [ ] 我们发给 pi 会话的消息能送达
- [ ] 父进程链上溯能把 intercom 会话正确关到它所在的 pane

---

## 7. 后续

| 版本 | 内容 |
|---|---|
| v1.13 | `TranscriptSource` 具体实现（Claude Code JSONL 优先），对话内容打通 |
| v1.14 | `Transport` 的 WebSocket 实现 + 手机端 UI |
| v1.15 | 推送通道接入（ntfy / 飞书 / TG 三选一） |
| v1.16 | 桌面端也用同一套对话表：卡片按「等你」置顶 |
