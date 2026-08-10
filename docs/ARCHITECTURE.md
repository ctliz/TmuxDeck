# TmuxDeck 架构说明

> 面向开发者与 agent。用户请看 [README](../README.md)。
> 本文描述 v1.12 拆分后的模块结构与数据流。

---

## 模块地图

```
src-tauri/src/
├── main.rs           入口，仅调用 lib::run()
├── lib.rs            Tauri Builder、托盘装配、command 注册
│
├── tmux.rs           ← 核心层：所有 tmux CLI 调用的唯一出口
├── registry.rs       终端 / agent 的探测与图标解析
├── config.rs         ~/.config/tmuxdeck/config.json 读写
├── models.rs         跨模块共享的数据结构
├── tray.rs           菜单栏菜单构建
│
├── intercom.rs       ← pi-intercom broker 客户端（agent 总线接入）
├── bridge.rs         ← 对话桥：pane ⊕ intercom 会话 → 统一对话模型
│
└── commands/         Tauri command 薄封装，不含业务逻辑
    ├── session.rs    会话级：创建 / 打开 / 列举 / 删除 / 改名
    ├── pane.rs       pane 级：新增 / 删除 / 抓取 / 发送输入
    └── utils.rs      图标、WSL 路径转换
```

**分层约束**：`commands/` 只做参数解析与错误转译，业务逻辑放在 `tmux.rs` / `bridge.rs`。
`intercom.rs` 与 `bridge.rs` **不依赖 tauri crate**——这样它们可以被单测直接覆盖，
将来也能抽成独立守护进程而无需改动。

前端目前仍是 `src/App.tsx` 单文件。

---

## 数据流

### 桌面看板（既有）

```
App.tsx ──invoke("get_tmux_sessions")──▶ commands/session.rs ──▶ tmux.rs ──▶ tmux CLI
```

4 秒轮询。这条路径 v1.12 未改动。

### 对话桥（v1.12 新增）

```
                     ┌─────────────────┐
tmux list-panes -a ──▶                 │
                     │  bridge.rs      │──▶ Conversation[]（按「等人的」置顶）
broker 会话注册表 ────▶  Registry       │
                     └─────────────────┘
```

三条通路各自的来源与状态：

| 用途 | 来源 | 状态 |
|---|---|---|
| 有哪些对话、各自什么状态 | broker 注册表 + `tmux list-panes -a` | 已实现 |
| 人 → agent | `intercom send`（优先）/ `send-keys`（兜底） | 已实现 |
| agent → 人 | `TranscriptSource` | **未定，见下** |

---

## 两处不显眼但关键的实现

### 1. pane ↔ intercom 会话的关联

intercom 上报的 `pid` 是 **agent 进程本身**，而 tmux 的 `pane_pid` 通常是 pane 里
那个 **shell**——agent 一般是 shell 的子进程，有时还隔着包装脚本（`cci` / `coi`）。
两者不相等，无法直接匹配。

`bridge.rs::find_owning_pane` 因此沿父进程链上溯（`ps -o ppid=`），
最多 12 层，遇到环或 pid ≤ 1 即停。

> 曾考虑用 cwd 匹配，但同一目录下开多个 pane 是常态，会产生歧义。父链是确定的。

### 2. AgentKind 的防抖

`pane_current_command` 拿到的是**前台进程名**。agent 执行 bash 工具时，
这个值会临时变成 `bash`——若据此更新，agent 会被误判成普通 shell。

`ConversationRegistry::refresh_panes` 因此只在识别出具体 agent 时更新 `kind`，
识别为 `Shell` / `Unknown` 时保留原值。

---

## 输入通道：两条，不混用

| 通道 | 用途 | 实现 |
|---|---|---|
| 字面文本 | 用户消息 | `send_keys()` → `tmux send-keys -l` |
| 控制键 | Escape / C-c / 方向键 | `send_key_name()` → 白名单校验后 `tmux send-keys <key>` |

**必须分开。** `tmux send-keys` 不加 `-l` 时会把 `C-c`、`Escape` 这类字符串
当键名解析——用户消息里出现这些词就会被当控制键执行。
控制键走独立白名单，也避免 send-keys 变成通用键盘注入口。

多行文本逐行发送（行内容 + 显式 `Enter`），因为部分 TUI 对裸 `\n` 的处理不一致。

---

## 未解决：对话内容从哪来

「有哪些对话」「什么状态」「怎么说话」都已打通，缺的是 **agent 说了什么**。

| 方案 | 状态 | 问题 |
|---|---|---|
| `capture-pane` | 已实现为兜底（`CapturePaneSource`） | 只有当前屏幕、无历史、TUI 重绘导致抖动、无轮次边界 |
| `pipe-pane` 原始流 | 未实现 | 混着大量光标移动与重绘转义序列，还原轮次极难 |
| **读 agent 的结构化会话记录** | **推荐主路径** | 本身即干净的分轮次数据；每个 agent 需一个读取器 |

`TranscriptSource` trait 已就位。关联问题已解决一半——父链上溯能拿到 agent 的
pid 与 cwd，据此定位其会话记录文件是可做的。

---

## 依赖姿态

v1.12 **未引入任何新的 Rust 依赖**：intercom 客户端只用 `std` + 已有的
`serde` / `serde_json` / `dirs`。Unix domain socket 走 `std::os::unix::net`，
分帧手写。

Windows 下 broker 使用命名管道，`intercom.rs` 目前 `#[cfg(unix)]` 门控，
Windows 返回 `ERR_INTERCOM_UNSUPPORTED_PLATFORM` 并自动降级到 send-keys 通道。

---

## 相关文档

- [PRD-v1.12 对话桥](./PRD-v1.12-conversation-bridge.md) — 需求与验收
- [intercom 协议参考](./REFERENCE-intercom-protocol.md) — 线协议细节
- [v1.12 决策记录](./DECISIONS-v1.12.md) — 被否决的方案与原因
- [现成方案调研](./PRIOR-ART-agent-bus.md) — 为什么不自造总线
