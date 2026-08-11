# docs 索引

## 从哪开始

| 你是 | 先看 |
|---|---|
| 用户 | [README](../README.md) · [中文](../README.zh-CN.md) |
| 想改代码 | [CONTRIBUTING](../CONTRIBUTING.md) → [ARCHITECTURE](./ARCHITECTURE.md) |
| 想知道现在在做什么 | [ROADMAP](./ROADMAP.md) |
| 接手 v1.12 对话桥 | [PRD-v1.12](./PRD-v1.12-conversation-bridge.md) → [DECISIONS-v1.12](./DECISIONS-v1.12.md) → [DESIGN-v1.13](./DESIGN-v1.13-transcript-source.md) → [DESIGN-v1.14](./DESIGN-v1.14-transport-security.md)

## 工程文档

| 文档 | 内容 |
|---|---|
| [ARCHITECTURE.md](./ARCHITECTURE.md) | 模块地图、数据流、两处不显眼的关键实现 |
| [REFERENCE-intercom-protocol.md](./REFERENCE-intercom-protocol.md) | pi-intercom 线协议（从上游源码反推整理，避免重复推导） |
| [DESIGN-v1.13-transcript-source.md](./DESIGN-v1.13-transcript-source.md) | 对话内容源设计（Claude Code JSONL 优先 + 兑底） |
| [DESIGN-v1.14-transport-security.md](./DESIGN-v1.14-transport-security.md) | 手机端传输与安全方案 |
| [ROADMAP.md](./ROADMAP.md) | 排期与进度，产品维护 |
| [SIGNING-DECISION.md](./SIGNING-DECISION.md) | 为何暂不签名 |
| [WINDOWS-VERIFICATION-v1.7.0.md](./WINDOWS-VERIFICATION-v1.7.0.md) | Windows 实机验收清单 |

## 决策留痕

| 文档 | 内容 |
|---|---|
| [DECISIONS-v1.12.md](./DECISIONS-v1.12.md) | 手机端接入被否决的五个方案及原因——**提新方案前先读** |
| [PRIOR-ART-agent-bus.md](./PRIOR-ART-agent-bus.md) | 现成方案调研：为什么不自造 agent 总线 |

## 功能 PRD

按版本号排列。功能立项先写 PRD 是本项目的文档纪律。

| PRD | 主题 |
|---|---|
| [v1.1](./PRD-v1.1.md) | 初始产品定义 |
| [v1.2](./PRD-v1.2-i18n.md) | 国际化（en / zh-CN） |
| [v1.3](./PRD-v1.3-windows.md) | Windows 支持（WSL） |
| [v1.4](./PRD-v1.4-activity.md) | 活跃时间显示 |
| [v1.5](./PRD-v1.5-preview.md) | 窗格内容预览 |
| [v1.6](./PRD-v1.6-liquid-glass.md) | 液态玻璃视觉 |
| [v1.7](./PRD-v1.7-tray.md) | 菜单栏常驻 |
| [v1.8](./PRD-v1.8-terminal-icons.md) | 终端图标 |
| [v1.9](./PRD-v1.9-card-header.md) | 卡片头部精简 |
| [v1.10](./PRD-v1.10-pane-mgmt.md) | pane 级管理 |
| [v1.11](./PRD-v1.11-focus-existing.md) | 防重复开窗 |
| [v1.12](./PRD-v1.12-conversation-bridge.md) | **对话桥：intercom 接入 + 手机端多路对话** |

## 发布说明

[v1.5.0](./RELEASE-NOTES-v1.5.0.md) · [v1.6.0](./RELEASE-NOTES-v1.6.0.md) ·
[v1.7.0](./RELEASE-NOTES-v1.7.0.md) · [v1.7.1](./RELEASE-NOTES-v1.7.1.md) ·
[v1.7.2](./RELEASE-NOTES-v1.7.2.md) · [v1.8.0](./RELEASE-NOTES-v1.8.0.md)
