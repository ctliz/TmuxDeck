# TmuxDeck v1.7 Tray Icon（菜单栏常驻）PRD

> 目标：TmuxDeck 常驻 macOS 菜单栏，不打开主窗口即可查看工作区状态、操作当前活跃会话、快速新建。
> 定位：「轻」的终极形态。丝滑 = 即时刷新 + 零窗口闪烁 + 状态可感知。
> 技术基础：Tauri 2 内置 TrayIconBuilder + Menu + on_menu_event，零新依赖（已验证）。

---

## 1. 背景

主窗口（v1.6 液态玻璃）再简洁也要开窗。Tray Icon 让用户**不开窗口**就能：
- 扫一眼当前有哪些工作区、活跃状态
- 一键打开/操作**当前活跃会话**
- 快速新建工作区

这是「工作区控制台」从「应用」到「常驻工具」的进化。

---

## 2. 菜单结构（点图标弹出）

```
──────────────────────────
● 当前活跃：project-alpha     ← 区块 1：当前活跃会话（高亮）
   ├─ 打开 (Ghostty)
   ├─ 新增分屏格
   └─ 最后活跃 3 分钟前
──────────────────────────
○ project-beta               ← 区块 2：全部会话（点按打开）
○ project-gamma
＋ 新建工作区…                ← 区块 3：内联快速新建
──────────────────────────
TmuxDeck 主界面              ← 打开主窗口
退出 TmuxDeck
──────────────────────────
```

### 区块 1：当前活跃会话

- 判定逻辑（v1.4 已实现的 last_active_ts + attached）：
  1. 有 attach 中的会话 → 选它
  2. 无 attach → 选 last_active_ts 最近的
- 显示：`● 会话名`（实心=运行中，空心=空闲）
- 子菜单：
  - **打开** → 复用现有 `open_session(name, terminal_id)`（用 config 的 default_terminal）
  - **新增分屏格** → 新命令 `add_pane(session_name)`（见第 4 节）
  - 只读行：`最后活跃 X 分钟前`（复用 v1.4 换算）

### 区块 2：全部会话

- 按「活跃度」排序：attach 中 > 最近活跃 > 其余
- 每项点击 → `open_session` 打开
- 会话太多时（>8）只显示前 8 + `查看全部（打开主界面）`

### 区块 3：内联快速新建

点「＋ 新建工作区…」弹出**原生二级菜单**（子菜单）：
- 名称输入…（用子菜单的文本项？→ 不，macOS 原生菜单不支持文本输入）
- **替代方案**：点「新建」→ 直接打开主窗口并聚焦到新建 Modal（主窗口已存在该 Modal）
- 或者：子菜单列出「用最近目录快速创建」几项（复用 recent_dirs）

**结论（PRD 定）**：新建走主窗口 Modal——「内联文本输入」在原生菜单里不现实（无输入框），
且主窗口 v1.6 已把新建做成虚线卡片入口。托盘「新建」= 打开主窗口 + 自动聚焦新建入口。

---

## 3. 刷新与丝滑

| 项 | 方案 |
|---|---|
| 菜单刷新 | 每 5s 后台重建菜单（Rust 侧 setInterval + 重建 Menu） |
| 图标状态 | 有运行中会话 → 实心图标；全部空闲 → 空心图标 |
| 打开操作 | `open_session` 直接拉起终端，不弹窗口 |
| 新增分屏 | `add_pane` 即时执行 + 下轮菜单刷新反映 |
| 窗口闪烁 | 菜单操作全程不聚焦主窗口（除「新建」「主界面」外） |

### Rust 侧实现要点

```rust
// lib.rs setup 中创建 tray
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem, Submenu};

TrayIconBuilder::new()
    .icon(app.default_window_icon().unwrap().clone())
    .menu(&build_tray_menu(app)?)   // 初始菜单
    .on_menu_event(|app, event| {
        match event.id().as_ref() {
            "open-session" => { /* 取 session name，调 open_session */ }
            "add-pane"     => { /* add_pane */ }
            "new-workspace"=> { /* show main window + focus create */ }
            "show-main"    => { /* show main window */ }
            "quit"         => { app.exit(0); }
            _ => {}
        }
    })
    .build(app)?;

// 5s 定时刷新
let tray = app.tray_by_id("main").unwrap();
std::thread::spawn(move || loop {
    std::thread::sleep(Duration::from_secs(5));
    let new_menu = build_tray_menu(&app_handle)?;
    tray.set_menu(Some(new_menu)).ok();
});
```

---

## 4. 新后端命令：`add_pane`

```rust
#[tauri::command]
fn add_pane(session_name: String) -> Result<(), String> {
    // 1. sanitize
    // 2. 找到该 session 的工作目录（从 config recent_dirs？No——从 tmux pane 当前目录：
    //    list-panes -F '#{pane_current_path}' 取第一个 pane 的路径）
    // 3. run_tmux(&["split-window", "-t", session, "-c", dir, "shell"]))
    //    —— 新 pane 默认 Shell（用户在 shell 里可再起 agent）
    // 4. run_tmux(&["select-layout", "-t", session, "tiled"])
}
```

**新 pane 默认 Shell**（PRD 定）：
- 原因：用户缺的是「可操作空间」，给 Shell 最通用；想再起 agent 自己敲
- 工作目录继承自 session 第一个 pane 的 `pane_current_path`（新 pane 在项目目录里，正确）

---

## 5. 主窗口联动

- 托盘「新建」「主界面」→ `app.show()` + `window.set_focus()`
- 主窗口关闭时**不退出应用**（tray 常驻）——需要改 tauri.conf.json：
  ```json
  "app": { "windows": [{ "title": "TmuxDeck", ... }] }
  ```
  加窗口事件处理：`on_window_event` 的 CloseRequested 时 `prevent_default()`（隐藏而非退出），
  或配置 `"visibleOnAllWorkspaces"` 等。**关闭 = 隐藏，托盘继续**。
- 首次启动：显示主窗口（第一次用需要看到界面），之后关闭即驻留托盘

**实现要点（Tauri 2 已验证）**：
- `tauri.conf.json` **必须**加 `app.trayIcon` 配置（icon 复用 `icons/icon.icns`），
  否则 `tray-icon` feature 不会被默认启用，`TrayIconBuilder` 编译不过
- 窗口关闭驻留：`tauri::Builder::on_window_event` 处理 `CloseRequested` → `api.prevent_close()` + 隐藏窗口，不退出应用
- 托盘菜单重建：`app.tray_by_id("main")` + `set_menu()`，5s 轮询线程需持有 `AppHandle`（`app.clone()`）

---

## 6. 验收标准

1. 应用启动后菜单栏出现图标，主窗口显示
2. 关闭主窗口 → 应用不退出，图标仍在
3. 点图标弹出菜单：当前活跃会话 + 全部会话 + 新建 + 主界面 + 退出
4. 「当前活跃」判定正确（attach 优先，否则最近活跃）
5. 点会话项 → 直接拉起终端 attach，无窗口闪烁
6. 点「新增分屏格」→ 该会话多一个 pane（Shell），tiled 重排，下轮菜单刷新可见
7. 图标状态：有运行中会话实心，全空闲空心
8. 菜单 5s 自动刷新，状态变化（新建/删除/活跃切换）秒级反映
9. macOS build + CI 双平台通过
10. i18n 三方对齐（新增 tray 文案双语齐全）

---

## 7. 明确不做

- ❌ 内联文本输入新建（macOS 原生菜单不支持，走主窗口 Modal）
- ❌ 托盘内实时 pane 预览（菜单项是文本，渲染不了终端内容；主窗口已有预览）
- ❌ 自定义图标动画/动态生成图标（用静态图标 + 明暗两态）
- ❌ Windows tray（v1.7 只做 macOS tray；Windows 托盘是系统托盘非菜单栏，形态不同，另议）
- ❌ 通知推送（会话完成提醒等，另立项）
- ❌ 多显示器多 tray（无意义）

---

## 8. 工作量预估

| 项 | 估算 |
|---|---|
| tray 初始化 + 图标 + 菜单构建 | 0.5 天 |
| 5s 动态刷新 + 状态图标 | 0.5 天 |
| add_pane 命令 + 事件分发 | 0.5 天 |
| 主窗口关闭驻留 + 新建联动 | 0.5 天 |
| **合计** | **约 2 人日** |
