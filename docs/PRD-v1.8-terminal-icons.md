# TmuxDeck v1.8 终端图标快捷打开 PRD

> 目标：卡片底部从「打开 (Ghostty) 按钮 + 下拉选择」改为**一排终端品牌图标**，
> 每个已安装的终端一个真实图标，点击即用该终端打开会话。
> 原则：图标必须真实（品牌 logo），不用 lucide 通用图标。

---

## 1. 背景与问题

现在卡片底部是单个「打开 (Ghostty)」按钮，多终端时 hover 出下拉菜单。
问题：
- 用户要换终端，需要两次点击（展开下拉 + 选）
- 按钮占一行，视觉重
- 每次要猜「当前选中的是哪个」

目标：一排终端图标，**图标即识别、点击即打开**。

---

## 2. 图标来源（真实品牌图标）

### 2.1 方案 A（主）：从已安装 App 提取 icns（运行时）

macOS 每个 App bundle 都有真实图标：
```
/Applications/Ghostty.app/Contents/Resources/icon.icns
/Applications/iTerm.app/Contents/Resources/AppIcon.icns
/Applications/kitty.app/Contents/Resources/AppIcon.icns
...
```

**后端改造**：
- `detect_environment()` 的 `ToolInfo` 增加 `icon_path: Option<String>` 字段
- 探测到已安装终端时，同时定位其 icns 路径。**注意：文件名各终端不同且不固定**
  （Ghostty 实测是 `Ghostty.icns` 而非 `icon.icns`），因此**不能硬编码**，应扫描
  `Resources/` 目录下所有 `.icns` 文件取第一个（或匹配 `AppIcon`/`icon` 关键字的）：
  ```rust
  fn find_app_icon(app_path: &Path) -> Option<String> {
      let res = app_path.join("Contents/Resources");
      std::fs::read_dir(res).ok()?.flatten()
          .find(|e| e.path().extension().map(|x| x == "icns").unwrap_or(false))
          .map(|e| e.path().to_string_lossy().to_string())
  }
  ```
- 找不到 icns 时 `icon_path = None`（前端回退到内置资源）

**前端渲染 icns**：Tauri 前端无法直接 <img> 加载 .icns（浏览器不支持该格式）。
**必须后端转换**：新增命令 `get_terminal_icon(terminal_id) -> Vec<u8>`
内部用 `iconutil` 或 `sips` 把 icns 转成 PNG：
```sh
sips -s format png icon.icns --out /tmp/tmuxdeck-icon.png   # macOS 自带
```
返回 PNG bytes，前端转 base64 显示。

### 2.2 方案 B（备）：内置品牌图标资源

把各终端官方 logo（SVG/PNG）打包进项目 `public/terminal-icons/`，不依赖本机安装。
来源：各项目 GitHub 仓库（Ghostty 的 logo.svg、kitty 的 logo 等）。
**用于**：终端已探测到但 icns 路径找不到 / 方案 A 转换失败时的回退。

> 主用 A、备用 B。A 保证「本机真实图标」，B 兜底。

---

## 3. 前端改造

### 3.1 卡片底部：图标行

```
┌──────────────────────────────────┐
│  🖥  ▶   ⬛   ⬛      ← 已装终端图标行  │
│  （默认终端高亮边框，hover 放大）      │
└──────────────────────────────────┘
```

- 每个已安装终端一个小图标（20-24px 圆角）
- **默认终端**（config 的 default_terminal）：高亮（边框 or 背景），其余透明
- 点击图标 → `open_session(session.name, term.id)`
- 一行放不下（>6 个）→ 滚动或收进「更多」按钮（图标行极简优先，v1.8 按最多 6 个处理）

### 3.2 删除旧 UI

- 移除「打开 (Ghostty)」大按钮
- 移除下拉菜单（`activeTerminalDropdown` + `ChevronDown` + 菜单 div）
- 移除 `Play` 图标依赖（若不再使用）

### 3.3 交互

- hover：图标轻微放大（`scale-110 transition`）
- 点击：立即打开 + tooltip 显示终端名
- 默认终端图标加「●」小点或边框以示区分

---

## 4. 新增/变更接口

| 命令 | 变更 |
|---|---|
| `detect_environment` | `ToolInfo` 加 `icon_path: Option<String>` |
| `get_terminal_icon(terminal_id) -> Vec<u8>` | 新增：icns → PNG bytes |

---

## 5. 验收标准

1. 本机已装 Ghostty：卡片底部显示 Ghostty 真实图标（从 .app 提取，非通用图标）
2. 多终端时：一排图标各显示对应品牌 logo，互不混淆
3. 点击任一图标：用该终端打开会话
4. 默认终端图标有高亮标识
5. icns 找不到时回退到内置资源，不显示空白/破裂图标
6. Terminal.app 的图标也能正确提取显示
7. 无「打开」大按钮和下拉菜单残留
8. macOS build + CI 双平台通过
9. i18n 三方对齐（新增文案双语）

---

## 6. 明确不做

- ❌ 图标 hover 展开大图 / 动画
- ❌ Windows 终端图标（本期只做 macOS；Windows 形态不同，且用户已暂停 Windows 侧）
- ❌ 自定义图标（用户上传/换图标）
- ❌ 图标排序设置（按注册表顺序即可）
- ❌ 未安装终端的图标仍显示（只显示已安装的，与产品「不显示无效选项」原则一致）

---

## 7. 工作量预估

| 项 | 估算 |
|---|---|
| 后端：icon_path + icns→PNG 命令 | 0.5 天 |
| 前端：图标行 + 删旧 UI | 0.5 天 |
| 内置兜底图标 + 验证 | 0.5 天 |
| **合计** | **约 1.5 人日** |
