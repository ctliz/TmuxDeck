# 🖥️ TmuxDeck

> **tmux 多 Agent 工作区控制台**
> 一款为多项目并行、多 Agent 协同设计的 Tauri 桌面 GUI。
> 用你自己的终端，跑你自己的 Agent。

---

## ✨ 核心特性

- 🎛️ **工作区一览** — 所有 tmux 会话以卡片呈现：窗口数、分屏数、创建时间、每个 pane 正在跑什么。
- 🖥️ **用你装的终端** — 自动探测 Ghostty / iTerm2 / WezTerm / kitty / Alacritty / 系统终端，**只列出已安装的**，不给你无效选项。
- 🤖 **跑你用的 Agent** — 自动探测 Pi / Claude Code / Codex / OpenCode / Gemini CLI / Aider，外加纯 Shell 兜底；还能自定义一条命令（如 `claude --model opus`）。
- 🔢 **分屏随你** — 1 / 2 / 4 / 6 分屏 tiled 平铺，每个 pane 自动启动所选 Agent。
- 🧠 **零配置可用** — 记住上次的终端 / Agent / 分屏数与常用目录，下次直接带出；目录走系统文件夹选择器，**全流程不用手打路径**。
- 🛡️ **永不卡死** — 系统终端 + 纯 Shell 双重兜底，即使一个第三方工具都没装也能正常使用。

---

## 🚀 快速开始

1. 打开 TmuxDeck
2. 点击 **新建工作区**
3. 填项目名 → 选目录（📁 按钮）→ 选 Agent / 分屏 / 终端
4. 点击创建，终端自动弹出，Agent 已在各分屏就位

> 关掉终端窗口不会销毁工作区。回到 TmuxDeck 点卡片上的 **打开** 即可随时接回。

---

## 🎯 设计原则

**只问必要的问题。** 某个维度只探测到一个候选时（比如你只装了一个终端），该选项行会**整行隐藏**——没有选择就不该打扰用户。

**默认值来自你的习惯。** 每次创建成功后记住你的选择，下次自动带出。

**不制造无效选项。** 没装的终端和 Agent 根本不会出现在界面上。

---

## 🛠️ 环境要求

| 依赖 | 必需性 |
|---|---|
| macOS (arm64 / x86_64) | 必需 |
| **tmux** | **必需**（唯一硬依赖，缺失时应用会引导安装） |
| 终端模拟器 | 非必需，系统自带 Terminal.app 即可兜底 |
| AI Agent CLI | 非必需，纯 Shell 即可兜底 |

```bash
brew install tmux    # 唯一必装项
```

---

## 🔧 开发与构建

```bash
npm install
npm run tauri dev      # 本地开发
npm run tauri build    # 打包 .app / .dmg
```

产物路径：`src-tauri/target/release/bundle/macos/TmuxDeck.app`

**技术栈**：Tauri 2.0 + React + TypeScript + Tailwind CSS + Rust

---

## ⚙️ 配置文件

位置：`~/.config/tmuxdeck/config.json`（由应用自动读写，一般无需手改）

```json
{
  "default_terminal": "ghostty",
  "default_agent": "pi",
  "default_panes": 4,
  "custom_agent": { "name": "Claude Opus", "command": "claude --model opus" },
  "recent_dirs": ["/Users/you/projects/foo"]
}
```

---

## 📐 扩展终端 / Agent

两张注册表都在 `src-tauri/src/lib.rs` 的 `detect_environment()` 里，加一行即可：

```rust
// 终端：(id, 显示名, 探测路径)
("wezterm", "WezTerm", vec!["/Applications/WezTerm.app"]),

// Agent：(id, 显示名, 可执行文件名)
("aider", "Aider", "aider"),
```

终端还需在 `open_session()` 的 `match` 中补一条启动分支。所有终端统一执行
`/tmp/tmuxdeck-<session>.sh` 脚本，因此无需处理各终端的引号转义差异。

---

## 📄 License

MIT License
