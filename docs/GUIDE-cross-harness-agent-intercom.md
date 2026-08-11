# 跨 Harness Agent Intercom 使用指南

> 适用范围：同一台机器、同一 OS 用户下的 Pi、OpenCode、Codex 与 Claude Code。
>
> 四个适配器共享 Agent Intercom protocol v3、本地 broker 和运行时目录，可以跨 Harness 执行定向 `list` / `send` / `ask` / `reply`。它不是公网通信服务，也不是广播聊天室。

## 1. 核心模型

```text
Pi ───────────┐
OpenCode ─────┼── ~/.pi/agent/intercom/broker.sock ── 本地 broker
Codex ────────┤
Claude Code ──┘
```

- 第一个连接的适配器会自动启动 broker，因此 Pi 不一定要最先启动。
- 最后一个客户端断开约 5 秒后，broker 自动退出。
- macOS / Linux 使用 Unix socket；Windows 默认使用命名管道。
- 只有已安装、已加载并成功注册的会话才会出现在列表中。
- 会话名用于可读寻址，但允许重名；真正可信的寻址键是**稳定 session ID**。

默认共享目录：

```text
~/.pi/agent/intercom/
```

其中包含 `broker.sock`、`broker.pid`、`broker.owner`、`broker-asks.json`、`inbox/`、`outbox/` 和 `config.json`。

## 2. 版本与安装原则

四端应使用同一代的 `@dataforxyz/agent-intercom-*` 适配器，不要混用旧的 pi-only `nicobailon/pi-intercom`。新旧适配器或协议版本混用，可能形成互不可见的 broker “岛”。

安装或升级任一适配器后，应让**所有仍打开的会话**完成一次 reload/restart。

### 2.1 Pi

安装：

```bash
pi install npm:@dataforxyz/agent-intercom-pi
```

更新：

```bash
pi update --extension npm:@dataforxyz/agent-intercom-pi
```

安装或更新后，在每个已打开的 Pi 会话中运行：

```text
/reload
```

也可以直接退出并重新启动 Pi。

### 2.2 OpenCode

安装 server plugin：

```bash
mkdir -p ~/.config/opencode
cd ~/.config/opencode
npm install @dataforxyz/agent-intercom-opencode
```

在 `~/.config/opencode/opencode.json` 中注册 server plugin。JSON 中不要使用 `~`，必须写绝对路径：

```json
{
  "$schema": "https://opencode.ai/config.json",
  "plugin": [
    "/Users/you/.config/opencode/node_modules/@dataforxyz/agent-intercom-opencode/dist/plugin.mjs"
  ]
}
```

在 `~/.config/opencode/tui.json` 中注册 TUI plugin，用于提供 `/intercom`、`/intercom-name`、`/intercom-id` 以及 Alt+M / Alt+I 快捷键：

```json
{
  "$schema": "https://opencode.ai/tui.json",
  "plugin": [
    "/Users/you/.config/opencode/node_modules/@dataforxyz/agent-intercom-opencode/dist/tui.mjs"
  ]
}
```

需要注意：

- `dist/plugin.mjs` 只放在 `opencode.json`。
- `dist/tui.mjs` 只放在 `tui.json`。
- 配置或包更新后要完整退出并重启 OpenCode；TUI plugin 不能用 Pi 的 `/reload` 方式热重载。
- 普通 worker 不需要 wrapper，直接运行 `opencode` 即可。
- 本地补丁后的 `tui.mjs` 支持在 OpenCode 刚启动、尚无 active session（即 home 页）时直接使用 `/intercom`、`/intercom-name` 或 `/intercom-id`：插件会自动创建一个空 session、进入该 session，再继续原操作。已有 active session 时始终复用当前 session，不会额外创建。

更新：

```bash
cd ~/.config/opencode
npm update @dataforxyz/agent-intercom-opencode
```

### 2.3 Codex

安装全局适配器：

```bash
npm install -g @dataforxyz/agent-intercom-codex
```

为普通 Codex 会话注册 MCP server：

```bash
codex mcp add codex-intercom -- codex-intercom-mcp
```

验证：

```bash
codex mcp list
```

安装包同时提供：

- `codex-intercom-mcp`：普通 Codex 会话中的工具。
- `coi`：可被消息唤醒、带 Alt+M / Alt+I 的 Codex wrapper。
- `codex-intercom-bridge`：发布多个后台 Codex worker 的高级用法。

更新后重启普通 Codex 会话，并重启所有 `coi` worker：

```bash
npm update -g @dataforxyz/agent-intercom-codex
```

### 2.4 Claude Code

安装全局适配器：

```bash
npm install -g @dataforxyz/agent-intercom-claude
```

为普通 Claude Code 会话注册全局可用的 MCP server（Claude 默认 scope 是 `local`，因此这里显式使用 user scope）：

```bash
claude mcp add -s user claude-intercom -- claude-intercom-mcp
```

验证：

```bash
claude mcp list
```

安装包同时提供：

- `claude-intercom-mcp`：普通 Claude Code 会话中的工具。
- `cci`：普通 wakeable Claude worker。
- `ccim`：最小化 wakeable worker，等价于 `cci --minimal`。
- `claude-intercom-worker`：单进程发布多个后台 worker 的高级用法。

更新后重启普通 Claude Code 会话，并重启所有 `cci` / `ccim` worker：

```bash
npm update -g @dataforxyz/agent-intercom-claude
```

## 3. 启动、命名与稳定身份

### 3.1 Pi

启动时命名：

```bash
pi --name <名称>
```

会话内改名：

```text
/name <新名称>
```

Pi 适配器直接使用 Pi 自身的 session ID 作为 intercom session ID：

- `/name` 只改变可读名称，不改变稳定 ID。
- 恢复同一 Pi session 会保留 intercom ID。
- 新建 Pi session 即使沿用同名，也会得到新的 ID。
- 需要显式复用时，可通过 `pi --session <path-or-id>` 恢复已有 session；高级场景也可使用 `pi --session-id <uuid>` 创建或打开指定 ID。

### 3.2 OpenCode

普通启动：

```bash
OPENCODE_INTERCOM_NAME=<名称> \
OPENCODE_INTERCOM_SESSION_ID=<稳定ID> \
opencode /path/to/project
```

恢复 OpenCode 自身对话：

```bash
OPENCODE_INTERCOM_NAME=<名称> \
OPENCODE_INTERCOM_SESSION_ID=<稳定ID> \
opencode /path/to/project --session <opencode-session-id>
```

`OPENCODE_INTERCOM_SESSION_ID` 是 Intercom 身份；`opencode --session` 指的是 OpenCode 对话。两者不是同一个概念。

若不设置稳定 ID，适配器会生成包含 PID 的临时 ID，进程重启后会变化。

### 3.3 Codex

需要持续接收任务的 worker 推荐用 `coi` 启动：

```bash
coi \
  --name <名称> \
  --id <稳定ID> \
  --cwd /path/to/project
```

- `--name` 是可读名称。
- `--id` 是稳定 intercom session ID。
- `coi` 默认将状态保存到共享 intercom 目录；使用相同 `--id` 重启时可以继续其 app-server thread。
- 只有通过 `coi` 启动的交互终端才有 Alt+M / Alt+I；普通 Codex + MCP 有工具，但没有这些快捷键。

普通 MCP 会话也可以在注册时固定身份：

```bash
codex mcp add <mcp-name> \
  --env CODEX_INTERCOM_NAME=<名称> \
  --env CODEX_INTERCOM_SESSION_ID=<稳定ID> \
  --env CODEX_INTERCOM_MODEL=codex \
  -- codex-intercom-mcp
```

不要让两个并发 Codex 进程使用同一个固定 ID；多 worker 场景下每个 worker 使用独立的 `coi --id`。

### 3.4 Claude Code

wakeable worker 推荐用 `cci` 或 `ccim` 启动：

```bash
cci \
  --name <名称> \
  --id <稳定ID> \
  --cwd /path/to/project
```

最小 worker：

```bash
ccim \
  --name <名称> \
  --id <稳定ID> \
  --cwd /path/to/project
```

- 复用相同 `--id` 会复用该 worker 的持久状态和 Claude conversation。
- Claude conversation ID 可用 `claude --resume <session-id>` 单独查看；它与 intercom `--id` 不同。
- `ccim` 的 woken turn 使用 safe mode，仍可接收工作并自动回复，但 turn 内不能主动调用 MCP intercom 工具联系其他 peer。
- 需要真正的交互 Claude TUI 被原地唤醒时，使用 `cci --tui --name ... --id ...`。

普通 MCP 会话可固定身份：

```bash
claude mcp add -s user <mcp-name> \
  --env CLAUDE_INTERCOM_NAME=<名称> \
  --env CLAUDE_INTERCOM_SESSION_ID=<稳定ID> \
  --env CLAUDE_INTERCOM_MODEL=opus \
  -- claude-intercom-mcp
```

并发 Claude 进程不要共用同一个固定 ID（详见[第 9 节](#9-稳定-session-id-注意事项)）。

## 4. 统一改名能力

运行时改名只更新其他 peer 可见的 `name`，**不会改变稳定 intercom session ID**。改名前后仍是同一个联系目标，已有 pending ask 和按 ID 寻址不受影响。

| Harness | 当前会话的改名入口 | 重启后保留方式 |
|---|---|---|
| Pi | 原生 `/name <新名称>`；适配器自动把 Pi 会话名同步到 Intercom | 恢复同一 Pi session |
| OpenCode | `/intercom-name` 打开改名输入框，或调用 `intercom_set_name({ name: "<新名称>" })` | 继续设置 `OPENCODE_INTERCOM_NAME` |
| Codex 普通 MCP 会话 | `intercom_set_name({ name: "<新名称>" })` | 继续设置 `CODEX_INTERCOM_NAME`；`coi` worker 启动时使用 `--name` |
| Claude Code 普通 MCP 会话 | `intercom_set_name({ name: "<新名称>" })` | 继续设置 `CLAUDE_INTERCOM_NAME`；`cci` / `ccim` worker 启动时使用 `--name` |

OpenCode、Codex 和 Claude Code 的运行时改名只对当前进程生效；重启后会重新读取环境变量或 wrapper 参数。后台 worker 的命名应在启动时完成：

```bash
coi --name <名称> --id <稳定ID> --cwd /path/to/project
cci --name <名称> --id <稳定ID> --cwd /path/to/project
ccim --name <名称> --id <稳定ID> --cwd /path/to/project
```

headless `cci` / `ccim` 没有交互控制台，无法输入 `/name` 之类的 slash command，只能通过启动参数 `--name` 命名；普通 Claude MCP 会话则用 `intercom_set_name` 改名。

> 本节的统一改名入口来自本地 `0.10.0` 安装包补丁，尚未等同于 npm registry 中所有同版本安装。执行 `npm update` 或重新安装可能覆盖补丁；在上游正式发布前，升级后应重新核对 slash command 与 `intercom_set_name` 工具是否存在。

## 5. 快捷键与命令入口

| 操作 | Pi | OpenCode | Codex | Claude Code |
|---|---|---|---|---|
| 运行时改名 | `/name <新名称>` | `/intercom-name` 或 `intercom_set_name` | 普通 MCP：`intercom_set_name`；`coi` 推荐启动时 `--name` | 普通 MCP：`intercom_set_name`；`cci` / `ccim` 推荐启动时 `--name` |
| 选择 peer 并发送 | `/intercom` 或 Alt+M | `/intercom` 或 Alt+M | `coi` 中 Alt+M | plugin 提供 `/claude-intercom:intercom`；`cci` / `ccim` 中 Alt+M |
| 复制当前精确联系目标 | `/intercom-id` 或 Alt+I | `/intercom-id` 或 Alt+I | `coi` 中 Alt+I | plugin 提供 `/claude-intercom:intercom-id`；`cci` / `ccim` 中 Alt+I |
| 列表导航 | ↑ / ↓ | ↑ / ↓ | wrapper 提示流 | wrapper 提示流 |
| 发送 | Enter | Enter | 按提示确认 | 按提示确认 |
| 多行换行 | Shift+Enter | Shift+Enter | 由 Codex composer 处理 | 由 Claude composer/worker 处理 |
| 取消 | Escape | Escape | Escape | Escape |

如果 Alt+M / Alt+I 没反应，先检查终端是否把 Option/Alt 当作 Meta 键传给应用，再确认使用的是带快捷键的入口：Codex 必须是 `coi`，Claude 必须是 `cci` / `ccim`，OpenCode 必须加载 `tui.mjs`。

Claude 的 `/claude-intercom:intercom` 与 `/claude-intercom:intercom-id` 需要安装或按 session 加载 Claude plugin；只执行 `claude mcp add` 的普通会话仍有 Intercom 工具，但没有这两个 plugin slash command。

OpenCode 在已有 session 时，`/intercom`、`/intercom-name`、`/intercom-id` 作用于当前 session；在 home/刚启动、没有 active session 时，本地补丁会先自动创建并进入一个空 session，再继续操作。若仍看到 `Open a session before using Intercom.`，说明新版 `tui.mjs` 尚未加载或本地补丁已被覆盖，应完整重启 OpenCode 并检查 `tui.json` 路径。

`/intercom-id` 或 Alt+I 复制的内容是跨 Harness 的：名称唯一时使用名称，名称重复时自动退回稳定 ID。

## 6. Agent 工具：set name / list / send / ask / reply

`list`、`send`、`ask` 和 `reply` 在四个适配器中含义一致。运行时改名工具目前由本地补丁后的 OpenCode、Codex 普通 MCP 和 Claude Code 普通 MCP 会话提供；Pi 使用原生 `/name`。

### 6.1 设置当前可读名称

OpenCode、Codex 普通 MCP 或 Claude Code 普通 MCP 会话：

```typescript
intercom_set_name({
  name: "<新名称>"
})
```

它只更新可读名称，不改变 `intercom_status({})` 返回的稳定 session ID。持久化规则见[统一改名能力](#4-统一改名能力)。

Pi 使用：

```text
/name <新名称>
```

### 6.2 查看连接状态

```typescript
intercom_status({})
```

用于确认当前 session ID、broker 连接和待处理消息。

### 6.3 列出所有 peer

```typescript
intercom_list({})
```

返回当前会话及所有已连接的 Pi、OpenCode、Codex、Claude Code 会话，包括短 ID、cwd、model 和实时状态。

如果当前 worker 由 orchestrator 管理，应优先查看同一 manager 的团队：

```typescript
intercom_team({})
```

### 6.4 非阻塞通知：send

```typescript
intercom_send({
  to: "<对端名称或ID>",
  message: "请检查 src/api/client.ts 的重试逻辑，完成后回报。"
})
```

`send` 只等待 broker 接收和对端持久入队确认，不等待对方完成工作或回复。适合任务下发、进度和完成通知。

### 6.5 需要答案：ask

```typescript
intercom_ask({
  to: "<对端名称或ID>",
  message: "这个变更需要兼容旧错误格式吗？"
})
```

`ask` 只进行有限时长的前台等待。超时不代表取消：请求会转为异步，迟到回复仍会作为新消息到达。长任务不要一直阻塞等待，改用 `send`，之后再检查状态。

### 6.6 回复收到的 ask：reply

在收到 ask 触发的当前 turn 中：

```typescript
intercom_reply({
  message: "需要兼容旧格式，只新增字段。"
})
```

若之后再回复，且有多个发送者正在等待：

```typescript
intercom_pending({})

intercom_reply({
  to: "<发送者名称或ID>",
  message: "需要兼容旧格式，只新增字段。"
})
```

`to` 是发送者名称或稳定 ID，不是 message/thread ID。不要手工构造 `replyTo`。

## 7. `PI_CODING_AGENT_DIR`

四个适配器都会读取 `PI_CODING_AGENT_DIR`，它会整体替换默认的 `~/.pi/agent` 基目录：

```bash
export PI_CODING_AGENT_DIR="$HOME/.pi/agent"
```

实际 intercom 目录变为：

```text
$PI_CODING_AGENT_DIR/intercom/
```

规则：

1. 希望互相发现的所有 Harness 必须使用**同一个绝对路径**。
2. 不同值会形成彼此不可见的独立 broker 岛；这是有意隔离时才使用的能力。
3. 不要只给 Pi 设置而漏掉 OpenCode、`coi` 或 `cci`。
4. 修改此变量后，需要 reload/restart 全部现有会话。
5. shell alias、tmux/Ghostty 启动命令、LaunchAgent 和 IDE 启动环境都要保持一致。

临时隔离示例：

```bash
PI_CODING_AGENT_DIR="$HOME/.pi/agent-lab" pi --name agent-a
PI_CODING_AGENT_DIR="$HOME/.pi/agent-lab" \
  OPENCODE_INTERCOM_NAME=agent-b \
  OPENCODE_INTERCOM_SESSION_ID=<稳定ID> \
  opencode /path/to/project
```

这两个会话能互见，但看不到默认 `~/.pi/agent/intercom` 下的会话。

## 8. Reload、Restart 与升级

| 场景 | 操作 |
|---|---|
| Pi 安装/更新 extension | 每个已打开 Pi 会话运行 `/reload`，或重启 Pi |
| OpenCode plugin/config 更新 | 完整退出并重启 OpenCode |
| Codex MCP/package 更新 | 重启普通 Codex 会话；重启 `coi` worker，复用原 `--id` |
| Claude MCP/package 更新 | 重启普通 Claude 会话；重启 `cci` / `ccim`，复用原 `--id` |
| `PI_CODING_AGENT_DIR` 修改 | 四端全部 reload/restart |
| broker 自动重启 | 客户端会自动重连，一般不需要人工处理 |

升级跨协议版本时应一次性完成四端更新。不要删除仍被活跃会话使用的 `broker.sock`、`broker.owner`、inbox/outbox 或 ask 状态文件。

推荐排障顺序：

1. 当前端运行 `intercom_status({})`。
2. 确认所有端使用相同 `PI_CODING_AGENT_DIR`。
3. 确认适配器已加载：Pi extension、OpenCode 两个 plugin、Codex/Claude MCP 或 wrapper。
4. OpenCode 在 home 运行 `/intercom`、`/intercom-name` 或 `/intercom-id` 时应自动创建并进入空 session；若仍提示先打开 session，完整退出并重启 OpenCode，确认 `tui.json` 指向补丁后的 `dist/tui.mjs`。
5. Codex wrapper 先运行 `coi --version`；它应直接输出 Codex 版本并退出。若无输出或意外进入 worker，检查 npm 的 `coi` 入口是否正确执行 `node .../dist/coi.mjs "$@"`，再重装或修正 wrapper。
6. 对 Pi 执行 `/reload`，其他 Harness 完整重启。
7. 再运行 `intercom_list({})`。
8. 仅当确认所有客户端已退出后，才考虑处理遗留 runtime 文件；不要在活跃会话期间删除 socket。

## 9. 稳定 session ID 注意事项

1. **名称不是身份。** 同名会话允许存在；按重名发送会失败，应改用稳定 ID。
2. **不要并发复用 ID。** 同一个稳定 ID 同时只应由一个进程注册。
3. **恢复同一身份。** Pi 恢复原 session；OpenCode 重用 `OPENCODE_INTERCOM_SESSION_ID`；Codex/Claude wrapper 重用 `--id`。
4. **Harness conversation ID 与 intercom ID 不同。** OpenCode `--session`、Codex thread ID、Claude `--resume` 都是各自对话标识。
5. **pending ask 依赖身份。** 重启后若换了 intercom ID，旧 ask 的回复授权不会自动转移到新身份。
6. **复制精确目标。** 优先使用 Alt+I 或 `/intercom-id` 获取可粘贴的跨 Harness contact。
7. **安全/高价值流程用 ID。** 名称适合日常协作；发布、破坏性操作审批和跨项目协调应直接用稳定 ID。

## 10. 本地环境核验

编写本文时，本地已安装同版本的四端适配器：

```text
@dataforxyz/agent-intercom-pi       0.10.0
@dataforxyz/agent-intercom-opencode 0.10.0
@dataforxyz/agent-intercom-codex    0.10.0
@dataforxyz/agent-intercom-claude   0.10.0
```

已确认的基础配置：

- Pi settings 已加载 `npm:@dataforxyz/agent-intercom-pi`。
- OpenCode `opencode.json` 已加载 `dist/plugin.mjs`。
- OpenCode `tui.json` 已加载 `dist/tui.mjs`。
- Codex MCP 中 `codex-intercom` 已启用。
- Claude MCP 中 `claude-intercom` 已连接。

版本升级后以实际 `package.json`、`codex mcp list`、`claude mcp list` 和 `intercom_status({})` 为准，不要长期依赖本文的版本号。

OpenCode 的 home 自动建空 session 能力目前同样属于本地 `0.10.0` 补丁。`npm update @dataforxyz/agent-intercom-opencode` 或重新安装可能覆盖它；在上游正式发布前，升级后需重新核对并完整重启 OpenCode，使正确的 `tui.mjs` 生效。

## 11. 推荐工作流

```text
1. 启动并设置唯一的可读名称
2. intercom_list 或 intercom_team 确认目标
3. 用 send 下发任务
4. 只有在下一步依赖答案时才用 ask
5. 收到 ask 的会话用 reply
6. 高风险操作使用稳定 ID，不使用模糊名称
7. 更新适配器后四端一起 reload/restart
```

相关文档：

- [Intercom 线协议参考](./REFERENCE-intercom-protocol.md)
- [v1.12 对话桥 PRD](./PRD-v1.12-conversation-bridge.md)
- [v1.12 决策记录](./DECISIONS-v1.12.md)
