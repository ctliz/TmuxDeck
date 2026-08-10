# TmuxDeck v1.10 Pane 级管理（新增/删除分屏）PRD

> 目标：卡片内直接管理 pane——新增分屏格、删除指定分屏格。
> 背景：v1.9 精简卡片头部后内容区变大，有操作空间；v1.7 的 add_pane 已在 tray 可用，搬到卡片并补删除。
> 后端：复用 add_pane，新增 kill_pane。

---

## 1. 交互形态（已确认）

```
┌─────────────────────────────┐
│ [●] project-alpha      [✕]  │  ← ✕ = 删除整个 session（现有，不变）
│ ┌──────┬──────┐             │
│ │ cmd  │ cmd  │×←hover 出现  │  ← 每个 pane 格 hover 出小 ×
│ ├──────┼──────┤             │
│ │ cmd  │ cmd  │             │
│ └──────┴──────┘             │
│ [+ 新增分屏]                │  ← 底部小按钮，tiled 重排
└─────────────────────────────┘
```

- **删除**：pane 预览格 hover 出现小 × → 点击 → confirm 弹窗 → 删除该 pane
- **新增**：卡片底部小按钮 → 新 pane（默认 Shell，目录继承）→ tiled 重排
- 右上角 ✕（删除 session）**不变**，两者是不同层级

---

## 2. 后端

### 2.1 复用 `add_pane`（v1.7 已有）

无需改动。逻辑已正确：sanitize → 取第一个 pane 目录 → split-window → tiled。

### 2.2 新增 `kill_pane(pane_id)`

```rust
#[tauri::command]
fn kill_pane(pane_id: String) -> Result<(), String> {
    // 1. 校验 pane_id 格式（tmux pane id 形如 %1，只允许 %\d+）
    // 2. run_tmux(&["kill-pane", "-t", &pane_id])
    // 3. 失败返回 ERR_KILL_PANE_FAILED
}
```

**pane_id 校验**：tmux pane id 是 `%数字`，注入风险低但必须校验格式
（`^%\d+$`），不能用 session 名的 sanitize（那是另一个格式）。

**注意**：`kill_pane` 只接收 pane_id，不接收 session——删除后如果该 pane
是最后一个（session 会随之销毁？），tmux 行为：kill-pane 最后一个 pane
会销毁整个 window/session。**前端需保证「只剩 1 个 pane 时禁用删除按钮」**（见 3.3）。

---

## 3. 前端

### 3.1 Pane 格 hover 删除

- 每个 pane 预览格右上角，`group-hover` 显示小 ×（`opacity-0 group-hover:opacity-100`）
- 点击 → `confirm`（现有确认模式）→ `invoke("kill_pane", { paneId })`
- 删除成功后 4s 轮询自然刷新（无需手动）
- i18n：复用现有 `card.destroy`？——不，语义不同（那是删 session）。
  新增 `card.killPane`：en "Kill this pane" / zh "删除此分屏"

### 3.2 底部新增按钮

- 卡片底部（现「打开图标行」下方或旁边）加一个小按钮 `[+ 分屏]`
- 点击 → `invoke("add_pane", { sessionName })`（复用）
- i18n：新增 `card.addPane`：en "Add pane" / zh "新增分屏"

### 3.3 边界：单 pane 禁用

- `session.panes_count <= 1` 时：
  - 该 pane 格不显示删除 ×（或禁用）
  - 底部新增仍可用（1 → 2 是合法操作）
- 逻辑：`pane 删除按钮仅当 panes_count > 1 时渲染`

### 3.4 布局

- 预览格 hover ×：绝对定位右上角（`absolute top-1 right-1`），格子需 `relative`
- 底部新增按钮：`text-xs` 小按钮，不喧宾夺主

---

## 4. 验收标准

1. 多 pane 会话：每个 pane 格 hover 出现 ×，点击 confirm 后该 pane 删除，网格 tiled 重排
2. 单 pane 会话：无删除 ×（禁用），新增仍可用（1→2）
3. 底部新增按钮：点击后多一个 pane（Shell），目录继承，tiled 重排
4. 删除 confirm 文案与 session 删除的区分（killPane vs destroy）
5. pane 删除后 4s 内列表自动刷新（无需手动）
6. kill_pane 的 pane_id 格式校验生效（非法 id 报错不 panic）
7. macOS build + CI 双平台通过
8. i18n 三方对齐（新增 key 双语）

---

## 5. 明确不做

- ❌ pane 拖拽排序/调整大小
- ❌ pane 重命名（tmux 不支持 pane 命名，跳过）
- ❌ 撤销删除（confirm 已够，不搞二次确认/undo）
- ❌ pane 内容迁移（把 pane 移到别的 session）
- ❌ 删除时区分「确认运行中进程」（confirm 统一处理）

---

## 6. 工作量预估

| 项 | 估算 |
|---|---|
| 后端 kill_pane + 校验 | 0.25 天 |
| 前端 hover × + confirm + 单 pane 禁用 | 0.5 天 |
| 底部新增按钮 + i18n | 0.25 天 |
| **合计** | **约 1 人日** |
