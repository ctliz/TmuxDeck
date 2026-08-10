# TmuxDeck v1.2 国际化 PRD

> 目标：面向全球开发者开源，界面语言跟随系统，**默认英文优先**。
> 原则：极简。不引入 i18n 框架，不做翻译管理后台，只解决"看得懂"。

---

## 1. 背景

项目即将开源。当前 UI 与错误信息**全部为中文**，非中文用户无法使用。

实测统计：
- 前端 `src/App.tsx`：约 **45 处** 中文 UI 文案
- 后端 `src-tauri/src/lib.rs`：**17 处** 中文字符串（错误信息 + 3 个显示名）

---

## 2. 范围

**v1.2 只做两种语言：`en` / `zh-CN`。**

- `en` 为**默认与兜底**（开源项目首要受众）
- 其他语言等社区 PR，本期不做

---

## 3. 技术方案（不引框架）

### 3.1 前端

新建 `src/i18n.ts`，一个文件搞定：

```ts
const en = { "app.subtitle": "Multi-agent workspace console for tmux", ... };
const zh = { "app.subtitle": "tmux 多 Agent 工作区控制台", ... };

const lang = navigator.language.startsWith("zh") ? zh : en;

export function t(key: string, vars?: Record<string, string | number>): string {
  let s = (lang as any)[key] ?? (en as any)[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}
```

- 语言检测：`navigator.language`，只判断是否以 `zh` 开头
- 缺 key 时回落 `en`，`en` 也缺则原样返回 key（开发期能立刻看出漏翻）
- **不要**引入 i18next / react-intl —— 45 条文案用不上框架

### 3.2 后端（Rust）

**原则：Rust 不做翻译，改为返回稳定的错误码，由前端翻译。**

理由：错误信息最终都显示在前端，让两侧各维护一套语言包必然不同步。

```rust
// 改造前
return Err("项目名称不能为空".to_string());
// 改造后
return Err("ERR_NAME_EMPTY".to_string());
```

错误码清单（全部大写下划线）：

| 错误码 | 原中文 |
|---|---|
| `ERR_NAME_EMPTY` | 项目名称不能为空 |
| `ERR_NAME_INVALID` | 非法的项目名称 |
| `ERR_TMUX_NOT_FOUND` | 未找到 tmux 安装 |
| `ERR_TMUX_LIST_FAILED` | 无法运行 tmux list-sessions |
| `ERR_TMUX_GENERIC` | tmux 错误 |
| `ERR_CREATE_FAILED` | 创建 tmux session 失败 |
| `ERR_KILL_FAILED` | 销毁 session 失败 |
| `ERR_RENAME_FAILED` | 重命名 session 失败 |
| `ERR_SCRIPT_WRITE_FAILED` | 写入脚本失败 |
| `ERR_TERMINAL_LAUNCH_FAILED` | 终端打开失败 |

**带系统详情的错误**（原来拼了 `{}` 的）统一格式 `CODE|详情`，前端按 `|` 切分：
翻译码 + 原样附加详情（系统报错不翻译，那是给开发者看的）。

**3 个非错误的显示名**特殊处理：
- `"Terminal (系统)"` → 注册表里改为 `"terminal.system"` 这类 key，前端 `t()` 翻译
- `"纯 Shell"` → key `agent.shell`
- `"自定义 Agent"` → key `agent.custom`
> 注意：这三个是 `ToolInfo.name`，前端渲染 chip 时判断 —— 若 `name` 以 `terminal.` / `agent.` 开头则走 `t()`，否则原样显示（第三方工具名如 "Ghostty" 不翻译）。

---

## 4. 两个必须处理的坑

### 4.1 禁止字符串拼接

当前代码大量存在 `共 {n} 个项目工作区` 这类由多段拼成的写法。英文语序不同，拼接必然出错。

**要求：一条完整句子必须是一个 key，变量用 `{n}` 占位。**

```
❌ <span>共 <b>{n}</b> 个项目工作区</span>
✅ t("stats.total", { n })   →  en: "{n} workspaces"   zh: "共 {n} 个工作区"
```

若必须在句中嵌入高亮样式，就把数字单独渲染、句子拆成 `stats.total.prefix` / `suffix` 两个 key，
**但优先选择整句不带样式**（更简单，视觉损失可接受）。

### 4.2 英文复数

英文有单复数，中文没有。

**做法：需要复数的 key 提供两条 `_one` / `_other`，由 `t()` 之外的简单判断选择。**

```ts
// 只有确实需要的地方这么做，不要给所有 key 都加
const key = n === 1 ? "stats.total_one" : "stats.total_other";
```

涉及位置：工作区数量、窗口数、分屏数、可用终端/Agent 数。
zh 两条填相同文案即可。

---

## 5. 文案翻译要求

**英文是给全球开发者看的第一印象，不要机翻腔。**

参考对照（README 已定调，保持一致）：

| key | en | zh-CN |
|---|---|---|
| `app.subtitle` | Multi-agent workspace console for tmux | tmux 多 Agent 工作区控制台 |
| `btn.newWorkspace` | New Workspace | 新建工作区 |
| `btn.open` | Open | 打开 |
| `card.destroy` | Destroy session | 销毁工作区 |
| `empty.title` | No workspaces yet | 暂无工作区 |
| `tmux.missing.title` | tmux is required | 未检测到 tmux |
| `tmux.missing.hint` | TmuxDeck uses tmux to manage agent sessions. Install it first: | TmuxDeck 依赖 tmux 管理多 Agent 会话，请先安装： |
| `agent.shell` | Plain Shell | 纯 Shell |
| `agent.custom` | Custom Agent | 自定义 Agent |
| `terminal.system` | Terminal (System) | 终端 (系统) |
| `confirm.destroy` | Destroy workspace "{name}"? | 确定销毁工作区「{name}」？ |

其余按此风格补齐。**英文用 sentence case**（只首字母大写），不要每个词都大写。

---

## 6. 顺带清理

代码里的**中文注释**（如 `{/* 卡片头部 */}`、`// 硬阻断：...`）请一并改为英文。
开源后这些注释是给全球贡献者读的。

---

## 7. 验收标准

1. 系统语言为英文时，UI **无任何中文**（含错误弹窗、confirm 对话框、placeholder、tooltip）
2. 系统语言为中文时，显示效果与 v1.1 一致，无遗漏、无 key 泄漏（不出现 `app.subtitle` 这种原文）
3. 触发一次真实错误（如新建时输入 `!!!`），英文环境下显示英文提示，中文环境显示中文
4. 数字为 1 时英文单复数正确（`1 workspace` 而非 `1 workspaces`）
5. 第三方工具名不被翻译（Ghostty / iTerm2 / Claude Code 等保持原样）
6. `npm run tauri build` 通过
7. 代码中无中文注释残留

---

## 8. 明确不做

- ❌ 应用内语言切换下拉框（跟随系统即可，v1.2 不做设置项）
- ❌ 英文/中文以外的语言
- ❌ 引入任何 i18n 框架或翻译平台
- ❌ 语言偏好持久化到 config.json
- ❌ README / CONTRIBUTING 的英文版（文档国际化单独排期）
