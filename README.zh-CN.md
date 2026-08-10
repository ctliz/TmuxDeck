# TmuxDeck

*[English](README.md) · [简体中文](README.zh-CN.md)*

管理运行 AI 编码 Agent 的 tmux 会话的桌面仪表盘。

TmuxDeck 把 tmux 会话变成可视化仪表盘。每个会话是一张卡片，显示每个分屏中正在运行的程序、最后活跃时间，以及会话是否仍在运行。点击一下，即可在你选择的终端中重新连接（attach）到该会话。

基于 [Tauri](https://tauri.app/) 构建，支持 macOS。Windows 支持（通过 WSL）正在开发中。

## 为什么做 TmuxDeck

tmux 以简洁著称，但简洁的代价是：一切都存在你的脑子里——会话名、分屏布局、哪个 agent 在哪。只管理几个工作区时这没什么；当每个项目都衍生出多个 agent 对话、同时有几十个在跑时，找到对的那个就成了每天最大的开销。

TmuxDeck 把这些用可视化的方式还给你：一键创建、一眼看全、一点找回。它也降低了门槛——没学过 tmux 命令的人也能直接用，仪表盘就是界面，tmux 藏在后台。

## 功能

- **会话总览。** 每个 tmux 会话以卡片呈现，包含窗口数、分屏数、每个分屏的运行命令和最后活跃时间。
- **一键创建工作区。** 输入会话名、选择目录、选择 Agent、分屏数和终端。分屏自动创建，终端自动打开。
- **使用你已经安装的工具。** 运行时会检测已安装的终端和 Agent，未安装的不会显示。支持的终端：Ghostty、iTerm2、WezTerm、kitty、Alacritty 和系统终端。支持的 Agent：Claude Code、Codex、OpenCode、Gemini CLI、Aider、Pi，或纯 Shell。
- **记住你的选择。** 上次使用的终端、Agent 和分屏数会保存到 `~/.config/tmuxdeck/config.json`，下次启动时自动恢复。
- **无需任何配置。** 即使没有安装任何第三方终端或 Agent，TmuxDeck 也会回退到系统终端和默认 Shell。

## 环境要求

- macOS（Apple Silicon 或 Intel）
- [tmux](https://github.com/tmux/tmux) —— 安装方式：

  ```sh
  brew install tmux
  ```

终端和 Agent 都是可选的。应用会检测你安装了哪些工具，只提供已安装的选项。

## 安装

从 [Releases 页面](https://github.com/ctliz/TmuxDeck/releases) 下载最新版本，将 `.dmg` 拖入「应用程序」文件夹。

如果 macOS 提示无法验证开发者，请右键点击应用图标，选择「打开」，然后确认。这是未签名构建的预期行为。

## 使用

1. 打开 TmuxDeck。
2. 点击 **新建工作区**。
3. 输入名称，选择目录，然后选择 Agent、分屏数和终端。
4. 点击 **创建**。

终端会打开并连接到新会话。关闭终端窗口不会销毁工作区——会话会继续运行，随时可以从仪表盘重新打开。只有点击卡片上的删除按钮才会销毁会话。

## 配置

设置保存在 `~/.config/tmuxdeck/config.json` 中，由应用自动写入。通常不需要手动编辑此文件。

```json
{
  "default_terminal": "ghostty",
  "default_agent": "pi",
  "default_panes": 4,
  "custom_agent": { "name": "Claude Opus", "command": "claude --model opus" },
  "recent_dirs": ["/Users/you/projects/foo"]
}
```

`custom_agent` 条目用于向新建工作区对话框添加自定义 Agent 命令。

## 常见问题

**使用 TmuxDeck 必须安装 Ghostty 或 Claude Code 吗？**

不需要。应用会检测已安装的工具，隐藏未安装的。如果什么都没安装，会使用系统终端和你的 Shell。

**关闭 TmuxDeck 会杀掉我的 Agent 吗？**

不会。工作区运行在 tmux 中，而不是应用内。关闭应用或终端窗口都不会影响会话。只有点击删除按钮才会销毁会话。

**为什么某个终端没有出现在选项中？**

只显示已安装的终端。如果某一类只有一个候选，整行会被隐藏，而不是显示为一个固定选项。如果你在非标准位置安装了终端且未被识别，请提交 issue。

**TmuxDeck 支持 Linux 或 Windows 吗？**

目前仅支持 macOS。Windows 支持（通过 WSL）正在开发中。

## 开发

开发环境搭建、终端/Agent 注册表及代码规范，请参阅 [CONTRIBUTING.md](CONTRIBUTING.md)。

## License

[MIT](LICENSE)
