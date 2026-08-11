# TmuxDeck v1.2 Internationalization PRD

> Goal: open-source for developers worldwide; UI language follows the system, **English-first by default**.
> Principle: minimal. No i18n framework, no translation-management backend, just "readable".

---

## 1. Background

The project is about to be open-sourced. The current UI and error messages are **entirely in Chinese**, unusable for non-Chinese speakers.

Measured:
- Frontend `src/App.tsx`: about **45** Chinese UI strings
- Backend `src-tauri/src/lib.rs`: **17** Chinese strings (error messages + 3 display names)

---

## 2. Scope

**v1.2 does only two languages: `en` / `zh-CN`.**

- `en` is **default and fallback** (primary audience for an open-source project)
- Other languages wait for community PRs; out of scope this release

---

## 3. Technical approach (no framework)

### 3.1 Frontend

Create `src/i18n.ts`, one file does it all:

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

- Language detection: `navigator.language`, only checks whether it starts with `zh`
- Missing key falls back to `en`; missing in `en` too → return the key as-is (instantly visible during development that a translation is missing)
- **Don't** bring in i18next / react-intl — 45 strings don't justify a framework

### 3.2 Backend (Rust)

**Principle: Rust does not translate. It returns stable error codes, and the frontend translates.**

Rationale: error messages ultimately all render in the frontend; having each side maintain its own language pack guarantees drift.

```rust
// before
return Err("项目名称不能为空".to_string());
// after
return Err("ERR_NAME_EMPTY".to_string());
```

Error-code list (all uppercase underscore):

| Error code | Original Chinese |
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

**Errors carrying system details** (those that used to concatenate `{}`) use the uniform format `CODE|details`, split on `|` in the frontend: translated code + raw details appended (system errors aren't translated — those are for developers).

**3 non-error display names** handled specially:
- `"Terminal (系统)"` → change in the registry to a key like `"terminal.system"`, translated by frontend `t()`
- `"纯 Shell"` → key `agent.shell`
- `"自定义 Agent"` → key `agent.custom`
> Note: these three are `ToolInfo.name`; when the frontend renders a chip it checks — if `name` starts with `terminal.` / `agent.` it goes through `t()`, otherwise shown as-is (third-party tool names like "Ghostty" are not translated).

---

## 4. Two pitfalls that must be handled

### 4.1 No string concatenation

The current code has a lot of `共 {n} 个项目工作区`-style constructs built from multiple segments. English word order differs, so concatenation is guaranteed to break.

**Requirement: one complete sentence is one key; variables use `{n}` placeholders.**

```
❌ <span>共 <b>{n}</b> 个项目工作区</span>
✅ t("stats.total", { n })   →  en: "{n} workspaces"   zh: "共 {n} 个工作区"
```

If highlighting must be embedded mid-sentence, render the number separately and split the sentence into `stats.total.prefix` / `suffix` keys — **but prefer a whole un-styled sentence** (simpler; the visual loss is acceptable).

### 4.2 English plurals

English has singular/plural; Chinese doesn't.

**Approach: keys needing plurals provide two variants `_one` / `_other`, selected by a simple check outside `t()`.**

```ts
// only do this where actually needed; don't add it to every key
const key = n === 1 ? "stats.total_one" : "stats.total_other";
```

Affected locations: workspace count, window count, pane count, available terminal/agent count.
For zh, both variants are filled with the same string.

---

## 5. Copy translation requirements

**English is the first impression for global developers; no machine-translation tone.**

Reference table (README has set the tone; stay consistent):

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

Fill in the rest in this style. **English uses sentence case** (only first letter capitalized), not all-caps words.

---

## 6. Incidental cleanup

**Chinese comments** in code (e.g. `{/* 卡片头部 */}`, `// 硬阻断：...`) should also be changed to English. After open-sourcing, these comments are read by global contributors.

---

## 7. Acceptance criteria

1. With the system language set to English, the UI shows **no Chinese at all** (including error popups, confirm dialogs, placeholders, tooltips)
2. With the system language set to Chinese, the display matches v1.1, no omissions, no key leakage (no raw `app.subtitle`-style strings)
3. Trigger a real error (e.g. enter `!!!` when creating): English environment shows the English message, Chinese environment shows Chinese
4. English singular/plural correct for a count of 1 (`1 workspace`, not `1 workspaces`)
5. Third-party tool names are not translated (Ghostty / iTerm2 / Claude Code etc. stay as-is)
6. `npm run tauri build` passes
7. No Chinese comments remain in the code

---

## 8. Explicitly out of scope

- ❌ in-app language-switch dropdown (follow the system; no setting item in v1.2)
- ❌ languages other than English/Chinese
- ❌ any i18n framework or translation platform
- ❌ persisting a language preference to config.json
- ❌ English versions of README / CONTRIBUTING (documentation internationalization scheduled separately)
