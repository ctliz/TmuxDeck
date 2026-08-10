# TmuxDeck v1.17 拖拽排序：卡片内 pane 换位 + 卡片重排 PRD

> 目标：两级拖拽——卡片**内**的 pane 格可以交换位置（真实交换 tmux 布局），
> 整张卡片也可以在网格中重排。**pane 不允许拖到另一张卡片**。
> 定位：纯交互增强，不改变数据模型与后端语义。

---

## 1. 交互形态

```
┌─ card-A ──────────────┐    ┌─ card-B ──────────────┐
│ [pane1] [pane2]  ←拖→ │    │ [pane1] [pane2]        │
│ [pane3] [pane4]       │    │ [pane3] [pane4]        │
└───────────────────────┘    └───────────────────────┘
   ↕ 整卡拖拽重排（网格内）
```

- **卡片内**：拖 pane 格到同卡另一格 → 两者交换（`swap-pane`，tmux 布局真实交换）
- **整卡**：拖卡片头部/任意空白处 → 网格内重排（纯前端状态）
- **禁止**：pane 拖出所属卡片（视觉上 drop 目标只限同卡格，拖到卡片外不响应）

## 2. 后端（tmux-backend）

### 新增 `swap_pane(pane_id_a, pane_id_b) -> Result<(), String>`

```rust
#[tauri::command]
fn swap_pane(pane_id_a: String, pane_id_b: String) -> Result<(), String> {
    // 1. 两个 id 都过 validate_pane_id（复用，格式 %\d+）
    // 2. run_tmux(&["swap-pane", "-s", &a, "-t", &b])
    // 3. 失败返回 ERR_SWAP_PANE_FAILED（含 is_no_server_err 拦截）
}
```

- 跨 session 的 pane 交换 tmux 也支持，但**前端禁止**，后端不做额外限制（极简）
- i18n：新增 `ERR_SWAP_PANE_FAILED` 双语

## 3. 前端（tmux-front）

### 3.1 卡片内 pane 拖拽

- 拖源：pane 格（`draggable` / pointer 事件，**不引新依赖**，HTML5 DnD 或手写）
- 放置目标：**仅同卡内其他 pane 格**；拖到卡外/其他卡 → 无 drop 响应（自然禁止跨卡）
- drop → `invoke("swap_pane", { paneIdA, paneIdB })` → 成功后 `loadData()`（4s 轮询也会自然刷新）
- 拖拽中视觉：源格半透明、目标格高亮
- 单 pane 卡片：无可拖对象，不显示拖拽提示

### 3.2 卡片级重排

- 拖源：整张卡片
- 实现：前端维护 `cardOrder: string[]`（session id 顺序），`loadData` 合并时**按 cardOrder 重排**，用户顺序不被 4s 轮询覆盖；新 session 追加到尾部
- 持久化：**不做**（重启后回默认顺序；后续需要再立项）

### 3.3 边界

- pane 拖拽与现有 hover 删除 ×、重命名输入框不冲突（拖拽只在 pane 格空白/命令区触发）
- 拖拽期间禁用点击（避免误触发打开会话）

## 4. 验收

1. 同卡拖 pane 格 A 到 B → 两格交换，tmux 实际布局交换（`list-panes` 顺序变化），4s 刷新后保持
2. 拖卡片到另一位置 → 网格重排，4s 轮询后顺序保持
3. 拖 pane 到另一张卡片上方 → 无任何反应（不交换、不报错）
4. 单 pane 卡：无拖拽源
5. 新增/删除 pane 后：卡片内顺序正确，cardOrder 对新 session 追加尾部
6. npm run build + npm test + cargo test 全绿；CI 双平台绿

## 5. 明确不做

- ❌ pane 跨卡片移动（需求明确禁止）
- ❌ 拖拽顺序持久化到 config（重启即默认）
- ❌ 触摸设备拖拽（手机端 v1.14 另议）
- ❌ 拖拽库依赖（手写，极简）

## 6. 工作量预估

| 项 | 估算 |
|---|---|
| 后端 swap_pane + 校验 + i18n | 0.25 天 |
| 前端 pane 拖拽 + 卡片重排 + cardOrder | 1 天 |
| 验证 | 0.5 天 |
| **合计** | **约 1.5-2 人日** |
