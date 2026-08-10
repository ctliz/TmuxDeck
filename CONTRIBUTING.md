# 参与 TmuxDeck 开发

感谢你愿意贡献！本文档面向开发者，用户文档请看 [README](README.md)。

## 本地开发

**环境要求**：Node.js · [Rust](https://www.rust-lang.org/tools/install) · tmux · macOS

```bash
git clone git@github.com:ctliz/TmuxDeck.git
cd TmuxDeck
npm install
npm run tauri dev      # 启动开发模式（热重载）
npm run tauri build    # 打包 .app / .dmg
```

产物位于 `src-tauri/target/release/bundle/`。

> 如果 build 报 `cargo: command not found`,先执行 `source "$HOME/.cargo/env"`。

**技术栈**：Tauri 2.0 · React · TypeScript · Tailwind CSS · Rust

## 项目结构

```
src/App.tsx           前端全部 UI（单文件）
src-tauri/src/lib.rs  后端全部逻辑（命令 + 注册表）
docs/PRD-v1.1.md      产品需求文档，改功能前建议先读
```

---

## 加一个新终端

两处改动，都在 `src-tauri/src/lib.rs`:

**1. 注册表** — `detect_environment()` 里加一行 `(id, 显示名, 探测路径)`:

```rust
("wezterm", "WezTerm", vec!["/Applications/WezTerm.app"]),
```

**2. 启动分支** — `open_session()` 的 `match` 里加一条:

```rust
"wezterm" => Command::new("/usr/bin/open")
    .args(["-na", "WezTerm", "--args", "start", "--", &script_path])
    .status(),
```

### 为什么不用处理引号转义

所有终端启动的都是同一个中间脚本 `/tmp/tmuxdeck-<session>.sh`,里面写好了 `exec tmux attach-session -t '<name>'`。

各家终端传命令的语法千奇百怪（`open --args` vs `osascript do script`）,直接拼接 attach 命令必然踩转义的坑。**统一执行一个脚本路径**就绕开了全部问题 —— 新增终端时请沿用这个模式，不要直接拼命令字符串。

## 加一个新 Agent

只需一行，在 `detect_environment()` 的 agent 注册表里:

```rust
("aider", "Aider", "aider"),   // (id, 显示名, 可执行文件名)
```

探测逻辑会自动走 `which` 以及 `~/.nvm/versions/node/*/bin/` 多版本目录。

---

## 代码约定

**会话名必须过滤。** 任何接收 session name 的 Tauri command,第一行都要调 `sanitize_session_name()`。该名字会被拼进 shell 命令和文件路径，不过滤就是命令注入漏洞。

**保持极简。** 参见 [PRD 第 7 节](docs/PRD-v1.1.md) 的「明确不做」清单。已经明确排除的方向（per-pane 混搭 Agent、工作区模板、多条自定义 Agent 管理、远程 SSH）请先开 issue 讨论，不要直接提 PR。

**只问必要的问题。** 这是本项目的核心设计原则：某个选项只有一个候选时，整行隐藏；没装的工具不出现在界面上。新增 UI 时请遵循。

## 提交 PR 前

- [ ] `npm run tauri build` 编译通过
- [ ] 改了分屏逻辑？跑一下 `tmux list-panes -s -t <name> | wc -l` 确认数量精确
- [ ] 改了会话名相关逻辑？用 `a'; rm -rf ~; '` 和 `../../etc/passwd` 测一下
- [ ] UI 文案不要出现内部实现名词

## 报告问题

提 [issue](https://github.com/ctliz/TmuxDeck/issues) 时请附上：macOS 版本、`tmux -V`、装了哪些终端 / Agent、以及复现步骤。
