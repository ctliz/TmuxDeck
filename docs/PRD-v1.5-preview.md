# TmuxDeck v1.5 实时 Pane 预览 PRD

> 目标：卡片上的分屏预览格子从「静态命令标签」升级为「实时输出窗口」，
> 用户不打开终端就能看到每个 Agent 的进展。
> 技术可行性已验证（实测 capture-pane 单次 ~6ms）。

---

## 1. 背景

现在卡片的分屏预览格子只显示 `pane_current_command`（pi / node / vim），是静态标签。
用户想知道「里面在干嘛」必须点开终端。

v1.5 让格子显示 pane 的**实时输出尾部**（最近几行），四个 Agent 的进展一屏尽收。

---

## 2. 技术基础（已验证）

### 2.1 capture-pane

tmux 原生 `capture-pane` 抓取 pane 当前屏幕内容，用 pane_id 定位：

```sh
tmux capture-pane -p -t %1        # %1 是 pane_id（list-panes 得到）
```

**性能实测**：100 次 capture 耗时 0.58s（单次 ~6ms），可接受。

### 2.2 需要处理的输出噪声

capture-pane 输出包含：
- **ANSI 转义序列**（颜色等）→ 需剥离
- **tmux 状态栏行**（session 信息）→ 需过滤
- **多余空行** → 需压缩

---

## 3. 设计

### 3.1 后端新增命令（Rust）

```rust
// 抓取单个 pane 的屏幕内容（尾部若干行）
#[tauri::command]
fn capture_pane(pane_id: String, max_lines: usize) -> Result<String, String>
```

实现：
1. `tmux capture-pane -p -t <pane_id>` 拿到原始输出
2. **剥离 ANSI 转义**：`\x1b\[[0-9;]*[a-zA-Z]` 正则替换为空（Rust 侧用 `regex` crate，或手写轻量 strip）
3. 过滤空行/状态栏行（以 `───` 开头的分隔行、含大量空格的纯装饰行）
4. 只保留**尾部 `max_lines` 行**（如 5）
5. 返回拼接后的纯文本

> 若 pane 已不存在（session 被删），返回空字符串，不报错。
> `regex` crate 已在本项目依赖树中（tauri 依赖），可直接用，或避免新增依赖手写 strip。

### 3.2 前端

**轮询策略（方案 C：全卡片 8s）**：
- 复用现有 4s 轮询 `get_tmux_sessions` 的定时器，在其内**追加** pane 内容抓取
- 每个可见 session 的每个 pane 调 `capture_pane(pane_id, 5)`
- 频率：与 sessions 刷新同步（4s）或独立 8s——**PRD 定 8s**（每轮 4s 是 sessions 元数据，8s 是内容抓取，避免同步放大开销）
- **窗口失焦时暂停抓取**（`document.visibilityState`）——隐私 + 省资源

**渲染**：
- 预览格子内显示尾部 ≤5 行，**小号等宽字体**（`text-[9px] font-mono`），灰调（`text-slate-500`）
- 超长行截断（`truncate` 或 CSS `line-clamp`）
- 保留当前「格子高亮 + agent 名」逻辑：有内容时高亮更明显，无内容回退显示命令名
- 卡片 hover 时格子内容**不**停止刷新（区别于「hover 才实时」的方案 A——我们已选 C）

**结构**：`TmuxPane` 前端类型加 `content: string` 字段（默认空串）。

### 3.3 i18n

无新增用户可见文案（预览是内容不是文案）。若需要 aria-label，用现有 key。

---

## 4. 性能预算

| 项 | 量级 |
|---|---|
| 单次 capture-pane | ~6ms |
| 典型场景：10 session × 3 pane × 每 8s 抓 | 30 次 / 8s ≈ **每秒 3.75 次 capture** |
| CPU 占用 | 可忽略（进程开销为主，每次 <10ms） |
| 失焦暂停 | 后台不抓，进一步省 |

**上限保护**：单轮抓取失败 3 次（pane 消失）即停抓该 pane，直到下次 sessions 刷新发现它。

---

## 5. 隐私

pane 内容可能含 API key、密码等敏感输出，会显示在卡片上。

缓解：
- **窗口失焦/最小化时暂停全部抓取**（硬性要求）
- 不做持久化（内容只存在于内存，关闭应用即消失）
- 未来可加「敏感内容遮罩」开关，本期不做（PRD 第 7 节）

---

## 6. 验收标准

1. 卡片预览格子显示对应 pane 的实时输出尾部（≤5 行），更新滞后 ≤8s
2. pane 内输出变化后，格子内容随之变化（无需手动刷新）
3. ANSI 转义/状态栏/空行被正确过滤，无花屏
4. pane 被销毁后格子回退显示命令名，不报错、不卡死
5. 窗口最小化/失焦后，网络/命令调用停止（可用 Activity Monitor 或抓包确认）
6. 10 个 session 场景下 CPU 占用无明显上升
7. macOS build + CI 双平台通过
8. i18n 三方对齐脚本跑通（en/zh/App.tsx）

---

## 7. 明确不做

- ❌ hover 才实时（方案 A）——已选定全卡片 8s 轮询
- ❌ 点击格子直接 attach 到该 pane（交互扩展，另议）
- ❌ 预览内容持久化/缓存到磁盘
- ❌ 敏感内容遮罩开关
- ❌ 行数可配置（5 行硬编码 v1.5）
- ❌ ANSI 颜色渲染（纯文本，极简）

---

## 8. 工作量预估

| 项 | 估算 |
|---|---|
| Rust: capture_pane + ANSI strip | 0.5 天 |
| 前端: 轮询 + 渲染 + 失焦暂停 | 0.5-1 天 |
| 性能/隐私验证 | 0.5 天 |
| **合计** | **约 2 人日** |
