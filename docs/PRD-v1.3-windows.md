# TmuxDeck v1.3 Windows 支持 PRD

> 目标：让 Windows 开发者用上 TmuxDeck，**复用现有架构与注册表结构**。
> 核心决策：**tmux 跑在 WSL 内**，Windows 终端外壳（cmd / PowerShell / Windows Terminal）作为入口。
> 前提：v1.2 的 i18n 已合入，所有新增文案直接走语言包。

---

## 1. 背景与事实约束

**Windows 没有原生 tmux。** 这是无法绕过的硬事实，决定了整体架构：

- ❌ 不可行：在 Windows 侧直接 `Command::new("tmux")` —— 系统里没有这个东西
- ✅ 可行：WSL（Windows Subsystem for Linux）里装 tmux，Windows 侧通过 `wsl.exe` 桥接

**关键洞察：`wsl.exe` 本身就是完美的桥。**
```
wsl.exe -- tmux list-sessions -F '#{session_name}'
wsl.exe -- tmux attach-session -t myproject
```
wsl.exe 直接透传 argv，**Windows 侧 attach 不需要脚本文件**（对比 macOS 需要 `.sh` 是因为 `open -na` 的引号地狱）。

---

## 2. 架构

```
┌─ Windows 侧 ──────────────────────────────┐
│  TmuxDeck (Tauri)                         │
│    ├─ cmd.exe / powershell.exe / wt.exe   │  ← 终端外壳（启动入口）
│    └─ wsl.exe ──┐                         │
└─────────────────┼─────────────────────────┘
                  ▼
┌─ WSL 内 ──────────────────────────────────┐
│  tmux server / 各 session / Agents        │  ← 真实运行时
└───────────────────────────────────────────┘
```

**分层职责**：
- WSL = 运行时（tmux + Agent 都活在 WSL 里）
- Windows = 控制台（TmuxDeck 只是管理界面）

**Agent 约束**：Agent CLI（claude / codex / opencode / pi 等）必须在 **WSL 内**安装。`new-session` 创建的 pane 里跑的是 WSL 内的 agent。

---

## 3. 后端抽象点（全部集中在 lib.rs）

### 3.1 `run_tmux(args) -> Output` —— 唯一的桥接函数

```rust
// 所有 tmux 命令调用统一走这个函数，内部按平台分流：
#[cfg(target_os = "windows")]
fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("wsl.exe").arg("--").arg("tmux").args(args).output()
}
#[cfg(target_os = "macos")]
fn run_tmux(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new(get_tmux_bin()).args(args).output()
}
```

**改动面**：`get_tmux_sessions` / `create_session` / `kill_session` / `rename_session` / `get_session_panes`
全部从 `Command::new(tmux)` 改为 `run_tmux(...)`。调用点不变，只改函数内部。

**`create_session` 的特殊性**：它内部拼 bash 脚本执行多条命令。Windows 上必须改造：

```rust
// 改造后：逐条调用 run_tmux()，不再拼 /bin/bash -c 字符串
run_tmux(&["new-session", "-d", "-s", name, "-c", dir, agent_cmd])?;
for _ in 1..panes {
    run_tmux(&["split-window", "-t", name, "-c", dir, agent_cmd])?;
}
run_tmux(&["select-layout", "-t", name, "tiled"])?;
```

> **好消息**：v1.1 时 developer 已经把分屏改成逐条 split 的循环写法（当时为修 P1），
> 现在正好天然适配 Windows。**禁止**退回 bash 拼接。

### 3.2 终端启动（open_session）

| id | 名称 | 启动方式 |
|---|---|---|
| `wt` | Windows Terminal | `wt.exe new-tab -- wsl.exe -- tmux attach -t <name>`（`Command::new("wt.exe")` 直接 argv） |
| `cmd` | Command Prompt | `cmd.exe /c start cmd /k wsl.exe -- tmux attach -t <name>` |
| `powershell` | PowerShell | `powershell.exe -NoExit -Command "wsl.exe -- tmux attach -t <name>"` |

- cmd / powershell 是 Windows 必有 → **Windows 双兜底**，永不卡死
- **不需要脚本文件**（wsl.exe 透传 argv，无引号问题）

### 3.3 配置路径

```rust
fn get_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    { dirs::config_dir().unwrap_or_default().join("tmuxdeck").join("config.json") } // %APPDATA%\tmuxdeck
    #[cfg(target_os = "macos")]
    { ...现有逻辑... }
}
```
新增 `dirs = "1"` 依赖（Rust 生态标准做法，跨平台统一）。

### 3.4 Agent / 工具探测

| 项 | macOS | Windows |
|---|---|---|
| 二进制探测 | `which <bin>` | `wsl.exe -- which <bin>`（在 WSL 内探测！） |
| nvm 多版本 | `~/.nvm/versions/node/*/bin/` | `wsl.exe -- bash -c 'ls ~/.nvm/versions/node/*/bin/<bin>'` |

**关键**：Agent 活在 WSL 里，所以**探测也必须发生在 WSL 内**，不能用 Windows 的 `where.exe` 探测一个 WSL 内才有的东西。

### 3.5 工作目录（Windows 特有痛点）

`tauri-plugin-dialog` 返回 **Windows 路径**（`C:\Users\foo`），但 tmux/agent 要的是 **WSL 路径**（`/mnt/c/Users/foo`）。

```rust
// 新增：Windows 侧把 Windows 路径转 WSL 路径
#[cfg(target_os = "windows")]
fn to_wsl_path(win_path: &str) -> String {
    // 调 wsl.exe wslpath -u '<win_path>'
    Command::new("wsl.exe").arg("wslpath").arg("-u").arg(win_path)
        .output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| win_path.to_string())
}
```

**前端工作目录输入框**在 Windows 上：
- 文件夹选择器选完 → 自动转换显示为 WSL 路径
- 用户手输时**提示**用 `/mnt/...` 格式

---

## 4. 前端改动

### 4.1 环境缺失引导（文案分平台）

macOS 缺 tmux → 引导 `brew install tmux`
Windows 缺 tmux → 引导两步：`wsl --install` → `sudo apt install tmux`

新增 i18n key：`tmux.missing.win`（en/zh 各一）

### 4.2 终端下拉

- Windows 上只显示 wt / cmd / powershell（已安装的）
- macOS 逻辑不变
- **探测结果本身由后端返回**，前端无需感知平台差异 —— 架构上已经隔离

### 4.3 无其他前端改动

卡片、统计、错误码翻译全复用。i18n 已有 key 尽量复用，新增 key 全部双语。

---

## 5. 风险与对策

| 风险 | 对策 |
|---|---|
| WSL 未安装 | 环境引导页分平台文案 + 一键复制 `wsl --install` |
| WSL 内无 tmux | 引导 `sudo apt install tmux` |
| wsl.exe 在 cmd 里交互模式兼容性 | Windows 11 ConPTY 已解决；Win10 22H2+ 建议升级 |
| wslpath 转换失败 | 兜底返回原路径 + 前端提示手输 WSL 路径 |
| WSL 发行版多个 | v1.3 只支持默认发行版（`wsl.exe --` 不带 `-d`），不做发行版选择 UI |

---

## 6. 验收标准

1. **在 Windows 机器实机**：装了 WSL + tmux 后，`detect_environment` 能列出 wt/cmd/powershell 至少一个
2. 用 cmd 创建 4 分屏工作区，`wsl.exe -- tmux list-panes -s -t <name> | wc -l` = 4
3. 用 Windows Terminal 打开既有会话，attach 成功
4. 文件夹选择器选 `C:\Users\x\proj` → 实际创建在 `/mnt/c/Users/x/proj`
5. 配置写入 `%APPDATA%\tmuxdeck\config.json`，重启后默认值带出
6. **macOS 上全部回归通过**（现有 7 条 v1.1 验收 + i18n 验收）
7. WSL 缺失时，引导页出现 `wsl --install` 提示且可复制

> ⚠️ 我在 macOS 上无法实机验证 Windows 分支。**交叉编译**：
> `cargo build --target x86_64-pc-windows-msvc`（需 Rust target 组件 + 链接器）。
> 若无法完整交叉编译，**至少保证 `#[cfg(target_os = "windows")]` 分支语法级正确**，
> 逻辑由 peer review 人工过一遍。验收 1-5 需要你在 Windows 机器上跑。

---

## 7. 明确不做（防止过度设计）

- ❌ WSL 发行版选择 UI（多发行版用户等 v1.4）
- ❌ Git-Bash / MSYS2 / Cygwin 支持（tmux 在这些环境可用但体验差，暂不做）
- ❌ Windows 原生终端（Windows Terminal 之外的第三方终端如 ConEmu / Cmder，等社区 PR）
- ❌ 跨平台 session 共享 / 同步（WSL 与 macOS 是两个独立世界）
- ❌ Linux 桌面原生支持（gnome-terminal / konsole 等，注册表结构已可平移，等 v1.4）
- ❌ Windows 上探测 Agent 的 Windows 原生版（claude.exe 等）—— 统一用 WSL 内版本

---

## 8. 工作量预估

| 项 | 估算 |
|---|---|
| `run_tmux` 抽象 + 改造 5 处调用 | 0.5 天 |
| `create_session` 去 bash 拼接 | 0.5 天 |
| 终端注册表 Windows 分支 + open_session | 0.5 天 |
| 配置路径 dirs + wslpath 转换 | 0.5 天 |
| WSL 内探测 + 引导文案 | 0.5 天 |
| 前端微调 + i18n 新增 key | 0.5 天 |
| **合计** | **约 3 人日** |
