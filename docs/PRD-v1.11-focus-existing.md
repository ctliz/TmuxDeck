# TmuxDeck v1.11 防重复打开（聚焦已有窗口）PRD

> 目标：session 已打开（attached）时，不重复开新终端窗口，而是**聚焦已有窗口**。
> 策略：C（精确聚焦，需辅助功能权限）+ A（降级：激活终端 App，无权限门槛）。
> 核心：已 attached → 聚焦；未 attached → 正常开新窗口。

---

## 1. 背景与问题

`open_session` 现在无条件拉起新终端窗口 attach。tmux 允许一个 session 被多个客户端 attach，
所以用户重复点击卡片 → 开一堆重复窗口，混乱且浪费。

**防呆目标**：已 attached 的 session 不新开窗口，把用户带到已有的那个窗口。

---

## 2. 方案（C + A 降级链）

```
点击打开 session
    │
    ├─ 未 attached ──→ 正常开新窗口（现有逻辑）
    │
    └─ 已 attached ──→ 尝试精确聚焦（C）
                          │
                          ├─ 有辅助功能权限 → 按窗口标题定位并聚焦（osascript System Events）
                          └─ 无权限 → 降级激活 App（A，osascript activate）
```

### 2.1 判定 attached

后端已有 `get_tmux_sessions` 返回 `attached`。新增轻量判定：
```rust
fn is_session_attached(name: &str) -> bool {
    // tmux list-sessions -F '#{session_attached}' -t <name> == "1"
}
```
或复用现有 sessions 数据（前端传 attached 状态给 open_session）。

**PRD 定**：后端判定（`open_session` 内部查一次，避免依赖前端状态可能过期）。

### 2.2 精确聚焦（C，osascript System Events）

```applescript
-- 按窗口标题定位终端窗口（tmux session 名 = 终端窗口标题）
tell application "System Events"
    tell process "Ghostty"
        repeat with w in windows
            if name of w contains "SESSION_NAME" then
                set frontmost of process "Ghostty" to true
                perform action "AXRaise" of w
                return
            end if
        end repeat
    end tell
end tell
```

- **需要辅助功能权限**（System Events 访问）：无权限时 osascript 报 `-25211`
- 执行失败 → 降级到 A

### 2.3 激活 App（A，降级）

```applescript
tell application "Ghostty" to activate
```

- 无权限要求
- 效果：激活终端 App，用户看到已有的 session 窗口（session 单窗口时足够）

### 2.4 终端差异

各终端的 AppleScript 进程名不同：

| 终端 | process 名 | activate 语法 |
|---|---|---|
| ghostty | "Ghostty" | tell application "Ghostty" to activate |
| iterm2 | "iTerm2" | tell application "iTerm2" to activate |
| terminal | "Terminal" | tell application "Terminal" to activate |
| wezterm | "WezTerm" | tell application "WezTerm" to activate |
| kitty | "kitty" | tell application "kitty" to activate |
| alacritty | "Alacritty" | tell application "Alacritty" to activate |

按 `terminal_id` 匹配，不匹配时跳过聚焦直接走原逻辑。

### 2.5 Windows（同样做防呆，AppActivate 无需权限）

Windows 上重复点击同样会开重复 tab/窗口（wt 新 tab、cmd 新窗、powershell 新窗），**防呆必须做**。

```powershell
# 已 attached → 聚焦已有窗口（按标题）
(New-Object -ComObject WScript.Shell).AppActivate("<session_name>")
```

- **AppActivate 按窗口标题激活，无需辅助功能权限**（比 macOS 的 System Events 门槛低）
- 聚焦失败（标题不匹配/窗口不存在）→ 静默返回（不新开窗口，不报错）
- 分支逻辑：`is_session_attached` → 是 → PowerShell AppActivate；否 → 现有开新窗逻辑
- Windows Terminal 的 tab 标题在 attach 时通常含 session 名，可匹配；不匹配时静默降级（用户可自行切换）

---

## 3. 前端改动

- `open_session` 命令签名不变（`name` + `terminal_id`）
- 前端无需感知 attached（后端判定）
- 点击行为不变：点击 → invoke open_session → 后端自行决定「聚焦 or 开新窗」

---

## 4. 验收标准

1. 未 attached 的 session：点击 → 正常开新终端窗口（回归，现有行为）
2. 已 attached + 有辅助功能权限：点击 → **不新开窗口**，已有窗口前置聚焦
3. 已 attached + 无权限：点击 → 不新开窗口，终端 App 被激活（activate）
4. 重复点击 5 次：终端窗口数量不变（始终 1 个）
5. 不同终端（若装了多个）：各自按 process 名正确聚焦/激活
6. osascript 失败（终端没装/进程不存在）→ 优雅报错或静默，不崩溃
7. macOS build + CI 双平台通过

---

## 5. 明确不做

- ❌ 引导用户开启辅助功能权限的引导页（失败静默降级即可）
- ❌ 权限检测（直接尝试执行，失败降级——不做预先探测，减少复杂度）
- ❌ 多窗口 session（同一 session 多窗口时聚焦第一个匹配窗口即可）

---

## 6. 工作量预估

| 项 | 估算 |
|---|---|
| 后端：attached 判定 + 聚焦/降级 osascript | 0.5 天 |
| 前端：无改动（签名不变） | 0 |
| 验证多终端 | 0.25 天 |
| **合计** | **约 0.5-1 人日** |
