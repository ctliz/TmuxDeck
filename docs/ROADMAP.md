# TmuxDeck 路线图（Roadmap）

> 维护人：产品（tmux-producter）。每完成一项勾选并记录版本；排期变更须在此更新。
> 版本约定：功能代号沿用 PRD 编号（v1.x）；发布版本号从 v1.7.0 起按实际发布递增。

## 当前状态

- **最新发布**：v1.7.0（2026-08-10）— 合并 v1.7 托盘 / v1.8 终端图标 / v1.9 卡片头部 / v1.10 pane 管理 / v1.11 防重复开窗
- **主线**：main 分支直推，CI（macOS + Windows 构建 + 前后端测试）全绿
- **测试体系**：后端 5 单测 + 前端 3 单测 + CI 测试步骤（已落地）
- **质量基线**：tmux 无 server 报错已治理（ERR_TMUX_NO_SERVER 双语友好提示）

## 规划队列

### P0 · v1.12 对话桥（进行中）

手机端接入。定位：TmuxDeck 成为 **pi-intercom 的「人类适配器」**——
该家族已有 Pi / Codex / Claude Code / OpenCode 适配器，唯独没有「人」。

- 需求与验收：`docs/PRD-v1.12-conversation-bridge.md`
- 架构：`docs/ARCHITECTURE.md` · 协议：`docs/REFERENCE-intercom-protocol.md`
- 决策留痕：`docs/DECISIONS-v1.12.md`（五个被否决的方案）· `docs/PRIOR-ART-agent-bus.md`

进度：

- [x] `tmux.rs`：`send_keys` / `send_key_name`（白名单）/ `list_all_panes`
- [x] `intercom.rs`：broker 客户端（UDS + 4 字节大端分帧 + 手工帧分派），无新增依赖
- [x] `bridge.rs`：对话模型、pane↔会话父链关联、投递路由、`Transport` 抽象
- [x] 文档落地（架构 / 协议 / 决策 / 脚本说明 / CONTRIBUTING 同步）
- [ ] **真机验证**：`node scripts/intercom-probe.mjs`（6 条清单见 `scripts/README.md`）
- [ ] `cargo test` 通过（本批代码尚未编译验证）
- [ ] `TranscriptSource` 具体实现 —— **唯一未定项**，见下

**未解决**：对话内容（agent 说了什么）的来源。`capture-pane` 只能兜底；
推荐读 agent 自己的结构化会话记录。trait 已就位，实现待立项。

**外部依赖**：本机装的是 `nicobailon/pi-intercom` 原版（pi-only），
Claude Code / Codex 仍是孤岛。打通需整体迁移到 `dataforxyz` 跨 harness 家族，
且**必须全量迁移**（新旧混用会分裂 broker）。属用户决策项。

### P1 · Windows 实机验证（暂缓，排期待定）

- 验收清单：`docs/WINDOWS-VERIFICATION-v1.7.0.md`（A 环境预检 / B 安装 / C 桥接 / D GUI）
- 进度：A1–A3 已 PASS（tmux 3.4、codex/opencode 可探测、wt/cmd/powershell 齐全）；B/C/D 待排期
- 主机：`tsiji@192.168.1.17`（访问方式见 server-deploy skill「Windows host access」）
- 触发条件：用户排期确认后执行；A/B/C 走 SSH，D 需 Windows 机器 GUI 配合
- 完成标准：全部 PASS 或仅 D8 跳过 → Windows 从「编译级」升级为「实机可用」

### P2 · 里程碑候选（待用户决策立项）

| 候选 | 价值 | 成本预估 | 备注 |
|---|---|---|---|
| per-pane agent 混搭 | 单个工作区跑不同 agent 编排 | 中 | v1.1 PRD 曾明确不做，需求待验证 |
| 工作区模板/布局预设 | 常用布局一键复用 | 低-中 | 同上 |
| macOS 签名 + 自动更新 | 消除 Gatekeeper 警告、用户自动升级 | 中-高 | 涉及 Apple 开发者账号 + tauri-updater |
| 拆分 App.tsx | 控技术债，功能增长前重构 | 中 | 单文件 987 行；`lib.rs` 已于 2026-08-10 拆分完成 |

### P3 · 技术债与持续改进

- [x] 拆分 `lib.rs` 为模块（tmux / registry / config / models / tray / commands，2026-08-10）
- [x] 引入自动化测试（v1.7.0，2026-08-10）
- [x] tmux 无 server 报错治理（v1.7.0，2026-08-10）
- [x] 发布流程命令行化（gh CLI 接入，draft → 正式发布）
- [x] create_session 字段命名修复（v1.7.1，2026-08-10）
- [ ] v1.7.2 候选：Ghostty 打开会话多实例 bug（open -na 强制新实例，AppleScript new window 方案已验证，开发中）
- [ ] 终端启动方式评估：wezterm / kitty / alacritty 的 open -na 潜在同类多实例问题（Ghostty 修复后评估，不主动扩大范围）
- [ ] README 措辞：Windows 验证通过后，从「macOS 为 battle-tested」更新为双平台声明

## 原则（沿用 PRD 惯例）

- 极简优先：未验证的需求不进队列；每期明确「不做」清单
- 小步快跑：单期工作量 ≤ 2 人日，发布即打 tag
- 文档纪律：功能立项先写 PRD，发布必写 RELEASE-NOTES
