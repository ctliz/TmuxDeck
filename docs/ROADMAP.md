# TmuxDeck 路线图（Roadmap）

> 维护人：产品（tmux-producter）。每完成一项勾选并记录版本；排期变更须在此更新。
> 版本约定：功能代号沿用 PRD 编号（v1.x）；发布版本号从 v1.7.0 起按实际发布递增。

## 当前状态

- **最新发布**：v1.7.0（2026-08-10）— 合并 v1.7 托盘 / v1.8 终端图标 / v1.9 卡片头部 / v1.10 pane 管理 / v1.11 防重复开窗
- **主线**：main 分支直推，CI（macOS + Windows 构建 + 前后端测试）全绿
- **测试体系**：后端 5 单测 + 前端 3 单测 + CI 测试步骤（已落地）
- **质量基线**：tmux 无 server 报错已治理（ERR_TMUX_NO_SERVER 双语友好提示）

## 规划队列

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
| 拆分 App.tsx / lib.rs | 控技术债，功能增长前重构 | 中 | 单文件 987 行 / 1157 行 |

### P3 · 技术债与持续改进

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
