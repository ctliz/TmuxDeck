# TmuxDeck v1.7.0 Windows 实机验收清单

> 目的：验证 Windows (WSL 桥接) 分支在真实机器上的可用性。
> 主机：`tsiji@192.168.1.17`（OpenSSH:22，见 server-deploy skill「Windows host access」）
> 被测包：`TmuxDeck_1.7.0_x64-setup.exe` / `TmuxDeck_1.7.0_x64_en-US.msi`（GitHub v1.7.0 release）
> 前置：WSL + 默认发行版已装 tmux；WSL 内至少一个 agent（claude/pi/codex 任一）
> 参考：docs/PRD-v1.3-windows.md 验收标准、PRD-v1.11 第 2.5 节

## A. 环境预检（SSH 可执行）

- [x] A1 `wsl.exe -- tmux -V` 输出版本号 → **PASS**（tmux 3.4，2026-08-10 实测）
- [x] A2 `wsl.exe -- which <agent-bin>` 找到至少一个 agent → **PASS**（codex、opencode，路径 /mnt/c/Users/80763/AppData/Roaming/npm/）
- [x] A3 终端外壳存在：`wt.exe` / `cmd.exe` / `powershell.exe` → **PASS**（三者齐全）

## B. 安装（SSH 静默安装 + 文件检查）

- [ ] B1 `.\TmuxDeck_1.7.0_x64-setup.exe /S` 静默安装无报错
- [ ] B2 安装产物存在：`%LOCALAPPDATA%\Programs\TmuxDeck\TmuxDeck.exe`

## C. tmux 桥接功能（SSH 直接验证，与应用无关的桥接正确性）

- [ ] C1 创建：`wsl.exe -- tmux new-session -d -s win-test -c /mnt/c/Users/tsiji`
- [ ] C2 分屏：连续 split 到 4 个 pane + `select-layout tiled`，`list-panes -s -t win-test | wc -l` = 4
- [ ] C3 列出：`wsl.exe -- tmux list-sessions` 包含 win-test
- [ ] C4 打开语义：`wsl.exe -- tmux attach-session -t win-test` 能进入（attach 后 detach）
- [ ] C5 销毁：`wsl.exe -- tmux kill-session -t win-test` 后列表为空
- [ ] C6 路径转换：`wsl.exe wslpath -u 'C:\Users\tsiji'` = `/mnt/c/Users/tsiji`

## D. 应用内验证（GUI，需在 Windows 机器上操作或 RDP 配合）

- [ ] D1 启动应用：无 tmux server 时显示空卡片网格，无红色错误条、无原始英文/临时路径
- [ ] D2 新建工作区弹窗：终端行显示已装外壳（wt/cmd/powershell）；Agent 行显示 WSL 内探测到的 agent
- [ ] D3 创建 4 分屏：`wsl.exe -- tmux list-panes -s -t <name> | wc -l` = 4，每个 pane 跑所选 agent
- [ ] D4 文件夹选择器：选 `C:\...` 路径后，输入框显示转换后的 `/mnt/c/...`
- [ ] D5 卡片点击打开：终端 attach 成功
- [ ] D6 重复点击卡片 5 次：终端窗口数量不变（AppActivate 聚焦，v1.11 防重复开窗）
- [ ] D7 配置持久化：`%APPDATA%\tmuxdeck\config.json` 生成，重启后默认值带出
- [ ] D8 WSL 缺失引导：若 WSL 未装，引导页显示 `wsl --install` + `sudo apt install tmux` 且可复制（无法卸载 WSL 实测则代码审查兜底）

## 执行分工

- A / B / C：SSH 可完整执行
- D：需要 Windows 机器 GUI 操作（本机用户或 RDP）；SSH 可部分验证（如 D7 配置文件、D3 的 tmux 侧断言）

## 记录要求

执行后回报：逐项 PASS/FAIL/跳过 + 失败项的实际输出，FAIL 项按 P0/P1/P2 分级。全部通过或仅 D8 跳过 → 可宣告 Windows 支持从「编译级」升级为「实机可用」。
