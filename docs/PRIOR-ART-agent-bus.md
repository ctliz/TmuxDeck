# 调研：agent 总线 / 手机端接入的现成方案

> 结论先行：**我们想造的东西已经存在，而且比设计稿完整。**
> `Agent Intercom` 覆盖了 pi + Codex + Claude Code + OpenCode 四种 harness，
> 共用一个本地 broker 和一套协议。TmuxDeck 不应该造总线，
> 应该成为这个家族里唯一缺失的那块拼图——**人类/手机适配器**。

调研日期：2026-08-10

---

## 1. Agent Intercom（决定性发现）

跨 harness、同机的 agent 消息系统。起源于 `nicobailon/pi-intercom`，
`dataforxyz` 把它扩成了跨工具家族：

| Harness | 仓库 |
|---|---|
| Pi | `dataforxyz/agent-intercom-pi` |
| Codex | `dataforxyz/agent-intercom-codex` |
| Claude Code | `dataforxyz/agent-intercom-claude` |
| OpenCode | `dataforxyz/agent-intercom-opencode` |
| 生命周期管理 | `dataforxyz/agent-intercom-orchestrator` |

**四个适配器共用同一个 broker 和同一套协议，跨 host 边界互发消息。**
这正是我们说的"跨家族通信"，而且它已经上线了。

### 它已经解决的，恰好是我们 PRD 里最难的部分

| PRD 里的难题 | Agent Intercom 的现成答案 |
|---|---|
| 四态判定（干活/等 agent/等人/退出） | broker 自动发布会话状态：`idle` / `thinking` / `tool:<name>` |
| 通信组、全局静默启发式 | 不存在了——一个 broker 一份全局注册表 |
| 「谁在等谁」 | `broker-asks.json` 存 ask/reply 边；`intercom_pending` 直接列出未决询问、发起者、已等时长 |
| 投递时机（往正在思考的 pane 塞字符会被吞） | 持久化 inbox + **idle-gated 投递**：忙时排队，空闲才注入；300ms 合批 |
| 投递可靠性 | 收方原子写入 inbox 后才 ACK，至少一次语义，断线重放 |
| 消息风暴 | 每会话最多 256 条未完成出站；按字节的连接级限流 |
| 寻址 | 会话名 + 稳定 session ID，重名时拒绝模糊投递 |

`intercom_list` 返回：会话名、短 ID、工作目录、模型、**实时状态**。
这一条就把 PRD 第 2 节（capture-pane 轮询 + hash 比对 + 静默启发式）整节作废。

### 技术细节（写适配器要用）

- 传输：macOS/Linux 用 Unix domain socket，Windows 用命名管道
- 协议：`pi-intercom` v3，**4 字节长度前缀 + JSON**
- 运行时目录：`~/.pi/agent/intercom/`（或 `$PI_CODING_AGENT_DIR/intercom/`）
  - `broker.sock` / `broker.pid` / `broker.owner` / `config.json`
  - `inbox/<hash>.json`、`outbox/<hash>.json`、`broker-asks.json`
- broker 首次连接时自动拉起，最后一个会话断开后 5 秒退出。无守护进程要管
- 工具面：`intercom_send`（发完就走）、`intercom_ask`（阻塞等 30s，超时转异步）、
  `intercom_reply`、`intercom_list`、`intercom_pending`、`intercom_status`、`intercom_team`

> README 里有一句很关键：broker 的 runtime instance ID 机制是为了
> "防止桌面 Pi 与**移动 RPC host** 同时打开同一 transcript 时的重连冲突"。
> **他们已经预期会有移动端 host 接入，但家族里还没有这个适配器。**

### 许可证注意

`agent-intercom-pi` 是 **AGPL-3.0-or-later**（早期 MIT 版本仍可按原条款使用）。
自己按线协议实现一个客户端不构成衍生作品，但**不要直接复制其源码**。
写 Rust 适配器对着协议实现即可。

---

## 2. AWS Labs CAO（形态不同，可借鉴）

`awslabs/cli-agent-orchestrator`，Apache-2.0。支持 Claude Code、Codex、Gemini、
Kiro、Kimi、Copilot、OpenCode、Q CLI——**唯独没有 pi**。

- 每个 agent 一个独立 tmux session，通过 MCP 暴露 `handoff` / `assign` / `send_message`
- 服务端按 `CAO_TERMINAL_ID` 路由，跟踪 `IDLE / PROCESSING / COMPLETED / ERROR`
- `cao session send <name> "msg"` 是纯 shell 命令 —— 印证了「shell 是所有 agent 的最大公约数」
- 自带 Web UI（`localhost:9889`）
- **插件可把 agent 间消息转发到 Discord / Slack / Telegram** —— 我们设想的通知链路
- 安全姿态与我们的结论一致：仅 localhost + Host 头校验防 DNS rebinding

**与我们工作方式的差异**：CAO 是 supervisor 主动 spawn worker 的层级模型；
我们是「手动开一堆 agent，然后互相说话」的对等模型。**Agent Intercom 才是对的形状。**
但 CAO 的 IM 转发插件、状态机命名值得抄。

---

## 3. 手机端已有方案

| 项目 | 覆盖 | 说明 |
|---|---|---|
| **Happy**（`slopus/happy`） | Claude Code、Codex | **开源**，移动 + Web 客户端，端到端加密，**权限请求与任务完成的推送通知**，终端离线时仍可看历史。最值得参考的一个 |
| Omnara | Claude Code、Codex | 闭源商用，App Store / Play 均有。已知短板：agent 需要输入时**不发系统通知**，要自己开 App 看 |
| VibeTunnel | 通用终端 | 浏览器访问 Mac 终端。复刻终端体验，但**无推送通知**，也没有回答问题/看 diff 的界面 |

三者都不支持 pi，也都不提供跨 agent 家族的总线视图。

---

## 4. 结论与建议

### 不要自造的

- ❌ **跨工具消息总线** —— Agent Intercom 已完成，且四个适配器正好覆盖我们全部工具
- ❌ **四态判定 / 静默启发式 / 通信组** —— broker 的会话状态与 ask 边是事实，不需要猜
- ❌ **send-keys 投递队列** —— intercom 的 durable inbox + idle-gated 投递严格更优
- ❌ **完整手机终端** —— Happy / VibeTunnel 已有

### 值得做的（家族里的真空）

> **TmuxDeck = Agent Intercom 的人类/手机适配器。**

具体形态：TmuxDeck 作为第五个适配器连上 `broker.sock`，注册成一个名为 `me` 的会话。

- agent 需要人时：`intercom_ask({ to: "me", message: "..." })` → TmuxDeck 收到 → 推送到手机
- 手机回复 → TmuxDeck 走 `intercom_reply` → broker 负责 idle-gated 投递与 ACK
- 桌面看板：读 `intercom_list`，直接拿到真实状态，不再轮询 capture-pane
- `intercom_pending` 天然就是「等你回话的收件箱」——**PRD 里设想的收件箱 UI，数据源现成**

**通知不再是检测出来的，是寻址到 `me` 的一条消息。** 与之前的判断一致，
只是总线不用我们造。

### 保留的兜底

没装 intercom 适配器的 agent（Aider、Gemini CLI、纯 shell）仍需 `send_keys` +
静默启发式覆盖。但它从主线降级为长尾兜底，可以做得很糙。

### 动手前必须验证的三件事

1. `~/.pi/agent/intercom/broker.sock` 在你机器上是否存在、协议版本是否为 v3
2. 非 pi 的适配器（Claude Code 的 `cci` / Codex 的 `coi` 包装器）是否需要改变现有启动方式
3. 用一个最小 Rust/Node 客户端连上 broker，跑通 `list` 与收一条 `send`——
   **跑通了再动 TmuxDeck 一行代码**

---

## 5. 对既有 PRD 的处置

| 文档 | 处置 |
|---|---|
| `PRD-v1.12-mobile-server.md` | 大幅作废。第 2 节（状态判定）整节删除；第 3 节保留消息与回复的 UI 设计；传输层改为 intercom broker |
| 「自造总线」构想 | 撤销，改为实现 intercom 适配器 |

---

## 来源

- [dataforxyz/agent-intercom-pi](https://github.com/dataforxyz/agent-intercom-pi)
- [nicobailon/pi-intercom](https://github.com/nicobailon/pi-intercom)
- [nicobailon/pi-messenger](https://github.com/nicobailon/pi-messenger)
- [earendil-works/pi](https://github.com/earendil-works/pi)
- [awslabs/cli-agent-orchestrator](https://github.com/awslabs/cli-agent-orchestrator)
- [slopus/happy](https://github.com/slopus/happy) · [happy.engineering](https://happy.engineering/)
- [Omnara (App Store)](https://apps.apple.com/us/app/omnara-claude-codex-mobile/id6748426727)
- [absmartly/Tmux-Orchestrator](https://github.com/absmartly/Tmux-Orchestrator)
