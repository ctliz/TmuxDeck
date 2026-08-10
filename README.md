# 🖥️ TmuxDeck

> **Ghostty & Tmux 4-Pi Agent 工作区控制台**  
> 一款专为多项目并行、多 Agent 协同设计的 Tauri 桌面 GUI 客户端。

---

## ✨ 核心特性

- 🎛️ **项目卡片可视化**：一目了然查看所有运行中的 Tmux 会话（关联窗口数、分屏数、创建时间与关联 Pane 命令）。
- ⚡ **一键唤起 Ghostty**：点击卡片或快捷键恢复会话，自动调用 `open -na Ghostty --args --command="tmux attach-session -t <project>"`。
- 🤖 **一键 4-Pi 项目阵列**：点击【新建 4-Pi 工作区】，自动创建平铺 (tiled) 4 分屏并独立启动 `pi` 智能体。
- 🎨 **暗黑科技风 UI**：使用 Tauri 2.0 + React + Tailwind CSS 打造高颜值的现代卡片仪表盘。
- 🔄 **实时无缝监控**：自动识别环境依赖（Tmux / Ghostty / Pi Agent），4 秒自动轮询无感刷新。

---

## 🛠️ 构建与开发

### 环境要求
- macOS (arm64 / x86_64)
- [Rust](https://www.rust-lang.org/) (`cargo`)
- [Node.js](https://nodejs.org/) & `npm`
- [Tmux](https://github.com/tmux/tmux)
- [Ghostty](https://ghostty.org/)

### 本地开发 (Dev)
```bash
npm install
npm run tauri dev
```

### 编译打包 (.app & .dmg)
```bash
npm run tauri build
```
打包输出路径位于 `src-tauri/target/release/bundle/macos/TmuxDeck.app`。

---

## 📄 License
MIT License
EOF
