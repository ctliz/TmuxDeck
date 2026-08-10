# TmuxDeck v1.6 液态玻璃 UI 重构 PRD

> 目标：从「厚重深色科技风」改为「macOS Liquid Glass 极简风」：
> 去掉顶部全部区域，搜索框半透明居中，新建入口变为卡片列表第一张虚线卡片。
> 纯前端重构，后端零改动。

---

## 1. 背景与问题

当前 UI 是「厚重深色科技风」：
- 顶部有一个大 header（logo + 标题 + 环境指示器 + 刷新 + 新建按钮）
- 其下还有一条搜索/统计横条
- 两层横条占掉大量垂直空间，视觉很重

用户反馈：不够简洁、看着太重。

---

## 2. 目标设计（Liquid Glass）

整体基调：**毛玻璃（backdrop-blur）+ 半透明 + 淡入淡出 + 平滑过渡**，
参考 macOS Tahoe (26) 的 Liquid Glass 设计语言。

### 2.1 顶部区域：全部移除

删除：
- header（logo / 标题 / 版本号 / 副标题）
- 环境状态指示器（tmux/terminals/agents 计数）
- 刷新按钮
- 新建按钮（移动到卡片列表，见 2.3）
- 搜索/统计横条

**保留功能，不保留形态**：
- 刷新 → 仍然 4s 轮询 + 手动下拉刷新？**v1.6 手动刷新去掉**（自动轮询已够，极简），
  或放一个极小的刷新按钮到搜索框右侧（可选，见 2.2）
- 环境指示 → 仅在 tmux 缺失的硬阻断引导页出现（已存在），正常状态不显示
- 统计 → 保留在搜索框 placeholder 或 tooltip 里，不占常驻空间

### 2.2 搜索框：半透明，居中

```
┌──────────────────────────────────────────┐
│                    🔍                    │   ← 居中，窄，半透明
└──────────────────────────────────────────┘
```

- 固定宽度约 240-320px，垂直居中于内容区顶部
- 样式：`backdrop-blur-xl bg-white/10 border border-white/15 rounded-full`
- 无标签、无外部框，就是一个漂浮的 pill 输入框
- 聚焦时轻微放大 + 高亮（transition）
- placeholder：`t("search.placeholder")`

### 2.3 新建入口：卡片列表第一张

卡片网格的第一张卡片是「新建工作区」入口：

```
┌─ ─ ─ ─ ─ ─ ─ ┐   ┌──────────────┐   ┌──────────────┐
│      +        │   │  project-a   │   │  project-b   │
│  新建工作区    │   │  ...         │   │  ...         │
└─ ─ ─ ─ ─ ─ ─ ┘   └──────────────┘   └──────────────┘
   虚线边框 + 加号     实卡片             实卡片
```

- 样式：`border-2 border-dashed border-white/20` + 半透明背景 + 大加号图标
- hover：边框变亮 + 淡入淡出箭头/文字提示
- 点击 → 打开现有「新建工作区」Modal（不新增表单）
- **永远在第一位**（即使有卡片），搜索时不参与过滤
- **空状态处理**：当 `filteredSessions.length === 0` 时，**不再显示整屏空状态页**，而是只显示
  「新建卡片」+ 一行小字提示（`empty.hint`）。原「立即新建」按钮逻辑并入新建卡片。

### 2.4 卡片视觉升级（液态玻璃）

现有卡片从 `bg-slate-900/80 border-slate-800` 改为：

```css
bg-white/10 backdrop-blur-xl border border-white/15 rounded-2xl
shadow-lg shadow-black/5 hover:shadow-xl hover:bg-white/15
transition-all duration-300
```

- 三态点、实时预览、agent 名高亮等**逻辑全部保留**，只换皮肤
- 背景：全局从 `bg-slate-950` 改为**渐变 + 暗色毛玻璃底**：
  `bg-gradient-to-br from-slate-900 via-slate-950 to-indigo-950/60`
  （保持深色底才能让白色半透明毛玻璃有对比度）

### 2.5 过渡与动画

- 卡片进入：`animate-fade-in-up`（淡入 + 上移 8px，300ms ease-out）
- 卡片删除/刷新：过渡动画（`transition-opacity`）
- 搜索过滤：卡片淡出淡入而非瞬间消失
- Modal：现有 `backdrop-blur-sm` 升级为 Liquid Glass 样式 + 淡入缩放
- 所有 hover/active 状态：`transition-all duration-200`

新增 CSS（index.css）：

```css
@keyframes fade-in-up {
  from { opacity: 0; transform: translateY(8px); }
  to   { opacity: 1; transform: translateY(0); }
}
```

---

## 3. 组件结构变化

| 现状 | 重构后 |
|---|---|
| `<header>`（logo/标题/环境/按钮） | **删除** |
| 搜索+统计横条 `<div>` | 搜索框独立居中 + 统计进 placeholder/tooltip |
| 卡片网格 | 第一张插「新建」虚线卡片 |
| 卡片样式 | 液态玻璃 |

**i18n**：新增 key：
- `search.hint`（可选，tooltip 显示统计，如 en: "3 workspaces · 1 running"）

---

## 4. 验收标准

1. 顶部**无任何**常驻横条（header/搜索条），内容区从视口顶部开始
2. 搜索框半透明圆角居中，聚焦有过渡动画
3. 卡片网格第一张是虚线「新建工作区」卡片，点击打开现有 Modal
4. 卡片、Modal 均为液态玻璃样式（半透明 + 毛玻璃 + 细边框）
5. 卡片有淡入动画，搜索过滤有过渡（非瞬间消失）
6. 功能零回归：创建/打开/删除/重命名/实时预览/三态全部可用
7. macOS build + CI 双平台通过
8. i18n 三方对齐脚本通过（新增的 key 双语齐全）

---

## 5. 明确不做

- ❌ 改变任何后端逻辑 / 注册表 / 命令
- ❌ 深色/浅色主题切换（v1.6 只做深色液态玻璃）
- ❌ 自定义主题色 / 背景图
- ❌ 窗口透明穿透（Tauri 透明窗口是另一个工程，需改 tauri.conf.json + 平台配置，另立项）
- ❌ 侧边栏 / 导航重构
- ❌ 移动响应式布局调整（桌面应用，保持桌面优先）
- ❌ 手动刷新按钮（自动 4s 轮询已够）——若验收时发现确实需要，加到搜索框旁，极小尺寸

---

## 6. 工作量预估

| 项 | 估算 |
|---|---|
| 删除 header/搜索条 + 重构布局 | 0.5 天 |
| 液态玻璃卡片样式 + 新建卡片 | 0.5 天 |
| 动画/过渡 + Modal 皮肤 | 0.5 天 |
| **合计** | **约 1.5 人日** |
