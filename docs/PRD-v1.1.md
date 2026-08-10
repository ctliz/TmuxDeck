# TmuxDeck v1.1 产品需求文档

> 目标：从「Ghostty + 4×Pi 的硬编码工具」进化为「**任意终端 × 任意 Agent** 的 tmux 工作区控制台」。
> 原则：**极简优先**。默认零配置可用，高级项折叠隐藏；不做用不到的抽象。

---

## 1. 现状问题（v1.0）

| 问题 | 表现 |
|---|---|
| 终端写死 | 只能 Ghostty，未安装的用户直接不可用 |
| Agent 写死 | 只能 `pi`，用 Claude Code / Codex / OpenCode 的用户被排除 |
| 分屏写死 | 必须 4 分屏，单人小项目开 4 个 agent 是浪费 |
| 概念泄漏 | UI 满屏 "4-Pi"、"Pi Ready"、"Pi Agent"，产品名被实现细节绑架 |
| 路径靠手打 | 工作目录要用户手输绝对路径，极易出错 |
| 无记忆 | 每次新建都要重填一遍，无默认值持久化 |

---

## 2. 核心设计：注册表 + 自动探测 + 记住上次

### 2.1 三个可选维度

新建工作区时用户只面对三个选择，且**全部有智能默认值**：

```
[项目名]  [目录 📁]        ← 必填 / 可选
─────────────────────
Agent:  ( pi ) claude  codex  opencode  + 自定义
分屏:   1  2  ( 4 )  6
终端:   ( Ghostty )  iTerm2  Terminal      ← 只列出已安装的
```

- 括号 = 默认选中项（来自上次使用记录，首次为探测到的第一个）
- **只显示已安装的**：没装 Kitty 就不出现 Kitty，不给用户制造无效选项
- 若某维度只探测到 1 个候选 → 该行整行隐藏（极简：没有选择就不要问）

### 2.2 终端注册表（Rust 侧静态表）

统一执行模型：**先把 attach 命令写进临时脚本，再让终端执行脚本**。
这样彻底规避各终端千奇百怪的引号转义问题。

```
/tmp/tmuxdeck-<session>.sh   内容: #!/bin/bash\nexec <tmux> attach-session -t '<name>'
```

| id | 显示名 | 探测路径 | 启动方式（`$S` = 脚本路径） |
|---|---|---|---|
| `ghostty` | Ghostty | `/Applications/Ghostty.app` | `open -na Ghostty --args --command=$S` |
| `iterm2` | iTerm2 | `/Applications/iTerm.app` | `osascript -e 'tell app "iTerm" to create window with default profile command "$S"'` |
| `terminal` | 终端 (系统) | `/System/Applications/Utilities/Terminal.app` | `osascript -e 'tell app "Terminal" to do script "$S"'` + activate |
| `wezterm` | WezTerm | `/Applications/WezTerm.app` | `open -na WezTerm --args start -- $S` |
| `kitty` | kitty | `/Applications/kitty.app` | `open -na kitty --args $S` |
| `alacritty` | Alacritty | `/Applications/Alacritty.app` | `open -na Alacritty --args -e $S` |

> Terminal.app 永远存在于 macOS → **保证 TmuxDeck 至少有一个可用终端**，不再出现"环境不满足"的死路。

### 2.3 Agent 注册表

探测方式：`which <bin>` + 常见路径（含 nvm 多版本目录，用 glob 扫 `~/.nvm/versions/node/*/bin/<bin>`）。

| id | 显示名 | 探测 bin | 启动命令 |
|---|---|---|---|
| `pi` | Pi | `pi` | `pi` |
| `claude` | Claude Code | `claude` | `claude` |
| `codex` | Codex | `codex` | `codex` |
| `opencode` | OpenCode | `opencode` | `opencode` |
| `gemini` | Gemini CLI | `gemini` | `gemini` |
| `aider` | Aider | `aider` | `aider` |
| `shell` | 纯 Shell | — | `$SHELL`（**永远可用的兜底项**） |

**自定义**：用户可在设置里填一条自由命令（如 `claude --model opus`），保存为一个 agent 项。
v1.1 只支持 **1 条自定义**，不做管理列表（够用即可）。

### 2.4 分屏

- 可选 `1 / 2 / 4 / 6`，统一 `select-layout tiled`
- 每个 pane 都启动同一个 agent（v1.1 不做 per-pane 混搭，需求未验证）
- `1` 分屏 = 单 agent 工作区，这是很多人的真实用法

---

## 3. 配置持久化

文件：`~/.config/tmuxdeck/config.json`（Tauri 直接读写，不引入额外依赖）

```json
{
  "default_terminal": "ghostty",
  "default_agent": "pi",
  "default_panes": 4,
  "custom_agent": { "name": "Claude Opus", "command": "claude --model opus" },
  "recent_dirs": ["/Users/x/Desktop/TmuxDeck"]
}
```

- 每次成功创建后写回本次选择 → 下次自动带出
- `recent_dirs` 最多 5 条，在目录输入框下方做快捷 chip

---

## 4. UI 调整清单

### 4.1 文案去 Pi 化
| 旧 | 新 |
|---|---|
| `新建 4-Pi 工作区` | `新建工作区` |
| `4-Pi 屏阵列布局` | `分屏` |
| `Pi Ready` | 显示真实 agent 名，如 `claude ×4` |
| `恢复会话 (Ghostty)` | `打开` （终端图标 tooltip 说明用哪个终端） |
| 副标题 `Ghostty & Tmux 4-Pi Agent 工作区控制台` | `tmux 多 Agent 工作区控制台` |

### 4.2 顶部环境指示器
- 从「Tmux / Ghostty / Pi 三个写死项」→「**Tmux ✓** + 已探测到 N 个终端 / M 个 Agent」
- 点击展开小面板列出具体清单，平时不占地方
- tmux 未安装是唯一的**硬阻断**：全屏引导 `brew install tmux`，一键复制命令

### 4.3 新建弹窗（极简）
- 目录：**加系统文件夹选择器按钮**（`tauri-plugin-dialog`），手输保留
- 三个选项行用 **segmented chips**，不用下拉框（一眼看全、一次点击）
- 只有 1 个候选的行自动隐藏
- 底部保留"自动配置"说明，文案动态化：`将创建 4 个分屏，每个运行 claude，并用 Ghostty 打开`

### 4.4 卡片
- pane 预览格子数量跟随真实 `panes_count`（不再固定画 4 格）
- 格子高亮判定：`pane.command` 命中任一已知 agent bin → 高亮 + 显示 agent 名
- 保留：重命名、销毁、打开

---

## 5. 后端接口变更（Tauri commands）

```rust
// 新增
detect_environment() -> Environment {
  tmux: Option<String>,
  terminals: Vec<ToolInfo>,   // 只含已安装
  agents:    Vec<ToolInfo>,   // 只含已安装 + shell 兜底
}
struct ToolInfo { id: String, name: String, path: String }

load_config()  -> Config
save_config(config: Config) -> ()
pick_directory() -> Option<String>          // 走 dialog 插件

// 改造（原 create_4pi_session / attach_session）
create_session(CreateOpts {
  name: String, dir: Option<String>,
  agent_id: String, panes: u8, terminal_id: String,
}) -> ()

open_session(name: String, terminal_id: String) -> ()

// 保留不动
get_tmux_sessions() / kill_session() / rename_session()
```

**兼容性**：v1.0 的 `check_env` / `create_4pi_session` / `attach_session` 直接删除，无存量用户负担。

---

## 6. 验收标准

1. 机器上**没装 Ghostty 也没装 pi** 时，应用仍能正常创建并打开工作区（Terminal.app + shell 兜底）
2. 装了 3 个终端 → 新建弹窗终端行出现 3 个 chip；只装 1 个 → 该行不显示
3. 选 `claude` + `2` 分屏 → tmux 里确实是 2 个 pane 各跑一个 claude
4. 创建一次后关掉应用重开 → 新建弹窗默认值 = 上次的选择
5. 卡片上能看出这个 session 跑的是哪个 agent、几个 pane
6. 全流程**不需要手打任何路径**
7. UI 中不再出现 "4-Pi" 字样

---

## 7. 明确不做（防止过度设计）

- ❌ per-pane 不同 agent 的混搭编排
- ❌ 多条自定义 agent 的增删改管理界面
- ❌ 工作区模板 / 布局预设保存
- ❌ 远程 SSH tmux
- ❌ Linux / Windows 终端注册表（本期只做 macOS，但注册表结构要能平移）
- ❌ agent 版本号探测、自动安装
