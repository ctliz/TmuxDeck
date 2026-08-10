# v1.12 决策记录：被否决的方案

> 手机端接入这件事，方案在成型前推翻过五次。
> 记下来是为了**不要再提一遍**——每条都写清当时为什么否，以及什么条件下值得重新考虑。

---

## 1. 内嵌 HTTP server + 移动网页 PWA

**否决。**

最初的设计：Tauri 进程内起 axum，绑 `0.0.0.0:7420`，托管一个响应式网页，
手机扫二维码配对。附带配对码、设备 token、吊销列表、CORS 与 DNS rebinding 防护。

否的原因：

- 手机端真正需要的是**分诊**（十个 agent 里哪个在等我），不是又一个网页
- 局域网内明文 HTTP，token 会被同网段嗅探；上 TLS 则 iOS 自签证书体验极差
- iOS 的 Web Push 要求 PWA 被加到主屏才生效，这个前置条件很难让人照做
- 整套配对/鉴权/吊销是**自己发明的安全机制**，而这是暴露 shell 执行入口的场景

**重新考虑的条件**：需要桌面级信息密度的多路对话界面，且已有 HTTPS（Tailscale 或 Tunnel）。

---

## 2. 完整终端模拟（xterm.js + WebSocket）

**否决。**

否的原因：

- SSH 客户端（Blink / Termius）加 Tailscale 今天就能做到，且体验更好
- tmux 中同一 session 的所有 client **共享窗口尺寸**——手机一 attach，
  桌面终端会被压成手机宽度。绕开需要 grouped session（`tmux new-session -t <原session>`），
  是一整块额外复杂度
- 它解决的是「我想看输出」，而瓶颈是「哪个在等我」

**重新考虑的条件**：需要在手机上操作 TUI 本身（而非与 agent 对话）。

---

## 3. 仅做 IM Bot 通知（飞书 / Telegram）

**部分保留，但不作为主形态。**

Bot 作为客户端有真实优势：只有出站连接、无需监听端口、推送白送、出门即可用。
但它是**单向通知 + 线性对话**，给不了「同时进行多路对话、并在其间转发」的形态，
而这正是用户要的。

现状：`Transport` trait 已抽象，Bot 可以作为其中一个实现接入。

---

## 4. 自建跨工具 agent 消息总线

**撤销。**

一度打算自己做 `tmuxdeck send @backend "..."`，用 shell 作为所有 agent 的最大公约数。
思路本身是对的——但 **[Agent Intercom](https://github.com/dataforxyz/agent-intercom-pi)
已经做完了**，而且覆盖 Pi / Codex / Claude Code / OpenCode 四种 harness，
还解决了我们尚未动手的部分：持久化投递、忙时排队、送达回执、消息风暴限流。

详见 [PRIOR-ART-agent-bus.md](./PRIOR-ART-agent-bus.md)。

**结论**：TmuxDeck 不做总线，做那个家族里唯一缺失的**人类/手机适配器**。

---

## 5. 靠 capture-pane 轮询做四态判定

**否决。整套删除。**

曾设计过一套启发式：2 秒轮询 `capture-pane`、对内容做 hash、
以「静默时长」推断 agent 是在干活、等别的 agent、还是等人。
其间还发现全局静默会漏报（pi 集群活跃时会掩盖 Claude Code 卡住），
于是又加了「通信组」概念来收窄作用域。

**全部作废**——broker 直接给出 `idle` / `thinking` / `tool:<name>`，
是各会话自动上报的事实。启发式再精巧也不该用来猜一个已经能直接读到的值。

未接入 intercom 的 agent（Aider、Gemini CLI、纯 shell）状态为 `unknown`。
**不猜**——宁可显示未知，也不要用抖动的启发式制造假信号。

---

## 6. 让每个 agent 装 hook 主动上报

**未采纳，但不算否决。**

思路：给 Claude Code 装 `Notification` / `Stop` hook，事件发生时 POST 到本地端点。
判定会从启发式变成事实，延迟从数十秒降到亚秒。

不采纳的原因：intercom 已经提供了同等信号，且**不需要改用户的 agent 配置**。
装 hook 是侵入性的，还得每个 agent 写一份。

**重新考虑的条件**：某个常用 agent 始终没有 intercom 适配器，
但有好用的 hook 机制。

---

## 保留的原则

- **宁可显示未知，不要用启发式制造假信号。** 通知的价值建立在可信上，
  一次假警报的代价远大于一次漏报的便利。
- **能读到事实就不要猜。** 这是第 5 条被推翻的根本原因。
- **不自己发明安全机制。** 第 1 条那套配对/token/吊销就是典型反例；
  Bot 与 intercom 都靠既有的信任边界（IM 账号、同 OS 用户）。
