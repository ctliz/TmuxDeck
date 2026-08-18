# TmuxDeck 多 CLI 通信指南

> 适用版本：TmuxDeck v1.14.3；Agent Intercom Protocol v4。
>
> 本文说明 Pi、Claude Code、Codex、OpenCode、Grok、Agy 如何接入本地 Broker，以及各 CLI 的启动方式、身份、环境变量、故障排查和安全边界。

## 1. 通信架构

```text
Pi ───────────┐
Claude Code ──┤
Codex ────────┤
OpenCode ─────┼── 本地 Agent Intercom broker ── Unix socket / named pipe
Grok ─────────┤
Agy ──────────┘
                     ▲
                     │
                 TmuxDeck
```

- TmuxDeck 为每个 pane/slot 分配稳定的 worker session ID，并写入团队 manifest。
- 第一个成功注册的 adapter 会启动本地 broker；其他 CLI 连接同一个 broker。
- v4 broker 按 workspace scope 隔离发现和短 ID 解析。
- 同一 workspace 内可使用名称或短 ID；跨 scope 必须使用完整 session ID。
- 这是同一 OS 用户下的本地通信机制，不是公网消息服务，也不是安全认证边界。
- 只有已安装、已加载且成功注册的 CLI 才会出现在 `intercom_list` 中。

默认运行目录：

```text
~/.pi/agent/intercom/
```

常见文件包括 broker socket、PID/owner、inbox/outbox、请求记录和配置文件。

## 2. TmuxDeck 自动加入团队

在 **Create Workspace** 中选择 Agent 后，TmuxDeck 会为每个 pane/slot 生成：

- `AGENT_INTERCOM_TEAM_MANIFEST`：绝对路径的团队 manifest；
- `AGENT_INTERCOM_SESSION_ID`：worker 稳定身份；
- `AGENT_INTERCOM_SESSION_NAME`：可读名称；
- `AGENT_INTERCOM_ROLE`：lead 使用 `manager`，其他 pane 使用 `worker`；
- `AGENT_INTERCOM_SCOPE_ID`：workspace scope；
- `AGENT_INTERCOM_MANAGER_TARGET` / `AGENT_INTERCOM_MANAGER_SESSION_ID`：worker 的 lead 目标；
- 对应 CLI 的身份环境变量，例如 `CODEX_INTERCOM_*`、`CLAUDE_INTERCOM_*`。

TmuxDeck 不要求用户手动执行 join 命令。启动命令、环境、tmux pane 元数据和 manifest 必须属于同一次创建事务。

### 2.1 终端能力

TmuxDeck pane 使用：

```text
TERM=tmux-256color
COLORTERM=truecolor
focus-events=on
extended-keys=on
terminal-overrides=*:RGB
```

不要将 `TERM` 粗暴改成 `xterm-ghostty`。tmux 会管理 pane 的终端类型；如果 CLI 仍与直接 Ghostty 表现不同，应同时记录 `TERM`、`TERM_PROGRAM`、窗口尺寸、`stty -a` 和 `capture-pane -p -e`。

### 2.2 权限绕过选项

TmuxDeck 面板默认对支持的 CLI 使用 bypass 模式，可在创建面板中关闭：

| CLI | 面板默认选项 | 说明 |
|---|---|---|
| Claude | `--dangerously-skip-permissions` | 跳过权限确认 |
| Codex | `--dangerously-bypass-approvals-and-sandbox` | 同时绕过 approvals 和 sandbox，风险最高 |
| OpenCode | `--auto` | 自动执行模式 |
| Grok | `--permission-mode bypassPermissions` | 跳过权限提示 |
| AGY | `--dangerously-skip-permissions` | 跳过权限确认 |
| Pi / Aider / shell | 无通用 bypass flag | 不注入伪造选项 |

这些选项只作用于 TmuxDeck 面板生成的默认命令，不会修改用户自定义命令，也不会影响直接在 Ghostty 中启动的 CLI。

## 3. 各 CLI 的通信方式

### 3.1 Pi

Pi 通过扩展加载 Agent Intercom。只安装官方 npm 包：

```text
pi install npm:@ctliz/pi-intercom@0.12.1
```

不要同时再装 `git:github.com/ctliz/agent-intercom-pi`。两份插件会注册同一套工具，Pi 启动即退出。

通信特点：

- 扩展在 Pi 进程内运行；
- 使用 Core v0.2.0 的 v4 协议和 team manifest；
- 可使用 `/intercom`、`/name`、`/intercom-join`、`/intercom-status`；
- 更新扩展后，在每个已打开的 Pi 会话执行：

```text
/reload
```

否则旧扩展可能继续连接旧 broker。

### 3.2 Claude Code

TmuxDeck macOS managed Claude 使用 bundled、固定 digest 的 Claude adapter。它包含：

- Claude plugin manifest；
- monitor；
- MCP server；
- `cci` / runtime 文件。

面板中的 managed Claude 典型启动形态：

```text
cci --tui --safe --id <session-id> --name <workspace> · Claude 01
```

通信链路：

```text
Claude Code
  ├─ plugin / MCP server
  ├─ inbox monitor
  └─ local broker
```

安装或修复完成后，TmuxDeck 会校验资源 SHA-256、managed marker、plugin chain、JavaScript runtime 和 monitor smoke test。健康状态为 `Healthy` 后，顶部 Claude chip 不应继续显示安装/修复提示。

标准 Claude 和 managed Claude 是两种独立模式：

- **Use Standard Claude**：使用系统检测到的 `claude`；
- **Use Managed Claude**：使用 TmuxDeck app-private managed root；
- 自定义 Agent 命令不会被 TmuxDeck 改写。

### 3.3 Codex

Codex 的普通通信通过 MCP server：

```text
Codex CLI
  └─ MCP client
      └─ node <managed>/dist/codex-server.mjs
          └─ local broker
```

TmuxDeck managed Codex 的 MCP 配置必须直接指向 bundled server：

```toml
[mcp_servers.codex-intercom]
command = "node"
args = ["<managed-root>/0.12.0-connect.1/dist/codex-server.mjs"]
```

不要将 MCP server 配置成 `codex-launcher.mjs`。launcher 是 CLI 启动包装，不是 MCP server；它默认调用 CLI，可能依赖错误的 `/usr/local/bin/codex` 路径。

Codex 面板启动命令通常为：

```text
codex --dangerously-bypass-approvals-and-sandbox
```

MCP 健康检查应至少验证 JSON-RPC `initialize` 响应。若 Codex TUI 没有输入提示，先区分：

1. MCP handshake 是否失败；
2. command 是否带 interactive mode/bypass flag；
3. prompt 是否已经在 ANSI capture 中绘制；
4. `TERM=tmux-256color` 与直接 Ghostty 的 `TERM=xterm-ghostty` 是否不同。

### 3.4 Grok 与 Agy

Grok 和 Agy 使用经 Claude MCP bridge 连接的外部手动安装 Intercom 插件；TmuxDeck 不会内置、安装或配置它们。安装前先确认 `PATH` 中可执行 `claude-intercom-mcp`：

```bash
command -v claude-intercom-mcp
```

再安装插件提供方给出的插件：

```bash
# Grok
grok plugin install <agent-intercom-grok plugin path> --trust

# Agy
agy plugin install <agent-intercom-agy plugin path>
```

Grok 的 MCP child 不会继承任意 pane 环境变量。多 pane Auto-Team 需要包含具体身份和 scope 的隔离每 pane MCP 配置；否则 Grok 只使用 live-only fallback 身份。AGY 同样要求宿主把每 pane 身份传给 MCP child。TmuxDeck 无法唤醒它们；请主动调用 `intercom_pending`。

### 3.5 OpenCode

OpenCode 使用两个配置面：

```text
opencode.json ── dist/plugin.mjs（服务端通信）
tui.json       ── dist/tui.mjs（TUI 命令、快捷键）
```

完整通信链路：

```text
OpenCode
  ├─ plugin.mjs
  ├─ tui.mjs
  └─ local broker
```

不要把 `tui.mjs` 注册到 `opencode.json`，也不要把 `plugin.mjs` 注册到 `tui.json`。配置或包升级后必须完全退出并重启 OpenCode。

TmuxDeck managed OpenCode 使用 bundled adapter 和 SDK dependency closure，安装阶段需要在 GUI PATH 下找到 Node/npm。当前 adapter 会为 staging 子进程补齐 PATH，并保留调用方显式 PATH。

## 4. 常用通信操作

不同 CLI 的 UI 命令略有差异，但协议概念一致。Grok 和 AGY 使用插件提供的 Intercom 界面且无法被唤醒，因此应主动调用 `intercom_pending`；Grok 要加入 TmuxDeck Auto-Team 还需要 materialize 的每 pane MCP 配置：

| 操作 | Pi | Claude | Codex | OpenCode |
|---|---|---|---|---|
| 查看在线 peer | `intercom_list` / `/intercom` | MCP 工具或 `/claude-intercom:intercom` | MCP 工具 | `/intercom` / MCP 工具 |
| 发送消息 | `intercom_send` | `intercom_send` | `intercom_send` | `intercom_send` |
| 请求回复 | `intercom_ask` | `intercom_ask` | `intercom_ask` | `intercom_ask` |
| 回复请求 | `intercom_reply` | `intercom_reply` | `intercom_reply` | `intercom_reply` |
| 查看自身身份 | `intercom_whoami` | `intercom_whoami` | `intercom_whoami` | `intercom_whoami` |
| 重命名 | Pi `/name` | 启动环境/adapter 名称 | 启动环境/adapter 名称 | `/intercom-name` |

推荐发送流程：

1. 先调用 `intercom_list`；
2. 名称唯一时可使用名称；
3. 名称重复或跨 scope 时使用完整 session ID；
4. 发送后等待 `ack` 或 reply，不要重复发送造成重复任务。

## 5. 故障排查

### 5.1 CLI 在 TmuxDeck 中找不到

记录：

```bash
env | egrep '^(PATH|TERM|COLORTERM|TERM_PROGRAM|LANG)='
which claude
which codex
which opencode
which grok
which agy
which pi
which claude-intercom-mcp
which npm
which node
```

GUI 启动的 Tauri 不一定继承 shell 的 PATH。TmuxDeck adapter 会扫描常见 Homebrew、NVM、Cargo、local 和 OpenCode 路径；自定义安装位置仍需放入 PATH 或使用自定义命令。

### 5.2 Plan 显示 `ERR_PLAN_INVALID`

检查 plan：

- `planId` 是否为 `plan_` 加 32 位小写 hex；
- fingerprint 是否为 64 位小写 hex；
- `canApply` 是否为 false；
- items 是否包含 `unavailable` 或 `migration-required`。

GUI PATH 缺失可能使已经安装的 Codex 被错误标为 unavailable。必须重启最新 Tauri 进程后重新生成 plan，不能复用旧 plan。

### 5.3 已安装但 UI 仍提示安装/修复

检查：

```text
~/Library/Application Support/tmuxdeck/managed/<harness>/<version>/tmuxdeck-managed.json
```

Claude npm 布局的 plugin manifest 位于：

```text
<root>/node_modules/@ctliz/agent-intercom-claude/.claude-plugin/plugin.json
```

不要只检查 `<root>/.claude-plugin`。安装成功后 TmuxDeck 会清理 health/environment cache；若 UI 仍显示旧状态，完全退出并重启 Tauri。

### 5.4 MCP handshake 失败

确认配置直接启动 server：

```text
node <managed>/dist/<harness>-server.mjs
```

不要把 CLI launcher 当 MCP server。对 Codex 发送 JSON-RPC initialize，并查看 stderr、server path、Node 版本和 managed root 完整性。

### 5.5 TUI 输入框看不见

按顺序采集：

```bash
tmux list-panes -a -F 'session=#{session_name} pane=#{pane_id} tty=#{pane_tty} pid=#{pane_pid} #{pane_width}x#{pane_height} cmd=#{pane_current_command}'
ps eww -p <cli-pid>
stty -f <pane-tty> -a
tmux show-options -s -t <session>
tmux capture-pane -p -e -J -t <pane> -S -60
```

`capture-pane -p -e` 中若已经出现 `›`、`>` 或其他 prompt ANSI 序列，说明输入提示已绘制，问题更可能是颜色、终端 capability 或视觉布局，而不是 stdin 被禁用。

## 6. 安全与 provenance

- Codex bypass 会同时绕过 approvals 和 sandbox；只在可信项目目录使用。
- OpenCode `--auto`、Grok bypass、AGY bypass 和 Claude bypass 也会降低人工确认；可在 TmuxDeck 创建面板时关闭 bypass。
- 自定义命令原样保留，TmuxDeck 不为 Pi、Aider 或 shell 猜测危险 flag。
- Grok 和 AGY 插件仅支持外部手动安装。它们要求 `PATH` 中存在 `claude-intercom-mcp`，由 TmuxDeck 注入每 pane 身份，必须轮询 `intercom_pending`，不能依赖唤醒机制。
- managed adapter 使用 bundled artifact、固定版本和 SHA-256；不要用未经授权的 registry 或网络包替换 release resource。
- Core、Pi、Claude、Codex、OpenCode 与 Grok/Agy bridge 插件必须使用相容的 protocol-v4 版本；混用旧 Core 或旧 adapter 可能形成互不可见的 broker island。
- 不要把 session ID、team manifest 或 broker token 当作公开凭据；它们属于本地同用户运行时身份和路由数据。
