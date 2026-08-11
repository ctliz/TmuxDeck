const en: Record<string, string> = {
  // App Header
  "app.title": "TmuxDeck",
  "app.subtitle": "Multi-agent workspace console for tmux",
  "app.version": "v1.9.3",
  "env.tmux_ok": "Tmux ✓",
  "env.terminals_one": "{n} available terminal",
  "env.terminals_other": "{n} available terminals",
  "env.agents_one": "{n} Agent",
  "env.agents_other": "{n} Agents",

  // Action Buttons & Tools
  "btn.refresh": "Refresh list",
  "btn.newWorkspace": "New Workspace",
  "btn.open": "Open",
  "btn.openWith": "Open ({terminal})",
  "btn.cancel": "Cancel",
  "btn.create": "Create & Start",
  "btn.creating": "Creating...",
  "btn.browse": "Browse...",
  "btn.saveAndApply": "Save & Apply",
  "btn.collapse": "Collapse",
  "btn.copy": "Copy",
  "btn.copied": "Copied",
  "btn.recheck": "I've installed it, recheck",

  // Search & Stats
  "search.placeholder": "Search workspace name...",
  "search.hint": "{total} workspaces · {running} running",
  "stats.total_one": "{n} total workspace",
  "stats.total_other": "{n} total workspaces",
  "stats.running": "{n} running",

  // Empty State
  "empty.title": "No workspaces found",
  "empty.hint": "Click \"New Workspace\" to quickly create a project deck with AI agents",
  "empty.createNow": "New Workspace Now",

  // Cards
  "card.windows_one": "{n} window",
  "card.windows_other": "{n} windows",
  "card.panes_one": "{n} pane",
  "card.panes_other": "{n} panes",
  "card.attached": "Active (Attached)",
  "card.runningDetached": "Running · Detached",
  "card.bgActive": "Running in background",
  "card.idle": "Idle",
  "card.openWithTerminal": "Open with {name}",
  "card.rename": "Rename",
  "card.nativeRenameUnsupported": "Native-split workspaces cannot be renamed while running.",
  "card.drag": "Drag to reorder workspace",
  "card.destroy": "Destroy workspace",
  "card.killPane": "Kill this pane",
  "card.confirmKillPane": "Kill this pane?",
  "card.confirmKillSlot": "Terminate this Agent slot? Its tmux session and process will end.",
  "card.confirmKillLastSlot": "Terminate the final Agent and delete this workspace? Its tmux session and process will end.",
  "card.addPane": "Add pane",
  "card.agentReady": "Agent Ready",
  "card.selectTerminal": "Launch terminal:",

  // Create Workspace Modal
  "modal.createTitle": "New Workspace",
  "modal.sessionNameLabel": "Workspace Name *",
  "modal.sessionNamePlaceholder": "e.g. my-ai-project",
  "modal.sessionNameHint": "Name will be normalized to: {name}",
  "modal.workingDirLabel": "Working Directory",
  "modal.workingDirPlaceholder": "Default: Home directory",
  "modal.recentDirs": "Recent:",
  "modal.agentLabel": "Agent Engine",
  "modal.panesLabel": "Pane Count",
  "modal.panesCount_one": "{n} pane",
  "modal.panesCount_other": "{n} panes",
  "modal.terminalLabel": "Launch Terminal",
  "modal.customAgentChip": "+ Custom",
  "modal.customAgentTitle": "Configure Custom Agent Command",
  "modal.customAgentNameLabel": "Display Name (Optional)",
  "modal.customAgentNamePlaceholder": "e.g. Claude Opus",
  "modal.customAgentCmdLabel": "Command *",
  "modal.customAgentCmdPlaceholder": "e.g. claude --model opus",
  "modal.summary": "Will create {panes} {panesText}, running {agent} in each, and open with {terminal}.",

  // Missing Tmux Warning
  "tmux.missing.title": "tmux is required",
  "tmux.missing.hint": "TmuxDeck relies on tmux to manage agent sessions. Please install tmux via Homebrew first:",
  "tmux.missing.win": "WSL or tmux not found in WSL",
  "tmux.missing.win_hint": "TmuxDeck runs tmux inside WSL. Please run these commands in Command Prompt / PowerShell:",

  // Built-in Names
  "terminal.system": "Terminal (System)",
  "agent.shell": "Plain Shell",
  "agent.custom": "Custom Agent",

  // Tray Menu
  "tray.activeHeader": "● Active: {name}",
  "tray.activeIdleHeader": "○ Active: {name}",
  "tray.open": "Open ({terminal})",
  "tray.addPane": "Add Pane",
  "tray.newWorkspace": "+ New Workspace...",
  "tray.showMain": "TmuxDeck Main Window",
  "tray.quit": "Quit TmuxDeck",
  "tray.viewMore": "View All ({n} total)...",

  // Confirmations
  "confirm.destroy_one": "Destroy workspace \"{name}\"? This will terminate its {n} tmux pane.\n\nIf you only want to leave temporarily, close the terminal window (Cmd+W). Any Close Window warning comes from the terminal and refers to the terminal connection, not this tmux workspace; choosing Close should only detach while the workspace keeps running in the background.",
  "confirm.destroy_other": "Destroy workspace \"{name}\"? This will terminate its {n} tmux panes.\n\nIf you only want to leave temporarily, close the terminal window (Cmd+W). Any Close Window warning comes from the terminal and refers to the terminal connection, not this tmux workspace; choosing Close should only detach while the workspace keeps running in the background.",
  "confirm.destroyNative_one": "Destroy workspace \"{name}\"? This will terminate its final Agent and delete the workspace.\n\nCmd+W only closes the current Ghostty split and detaches; the Agent keeps running in the background.",
  "confirm.destroyNative_other": "Destroy workspace \"{name}\"? This will terminate all {n} Agent slots and their tmux sessions.\n\nCmd+W only closes a Ghostty split and detaches; the Agents keep running in the background.",

  // Error Code Mappings (from Rust)
  "ERR_ADD_PANE_FAILED": "Failed to add pane",
  "ERR_ADD_PANE_OUTPUT_ERR": "Error adding pane",
  "ERR_CONFIG_SAVE": "Failed to save configuration",
  "ERR_ICON_CONVERT_FAILED": "Icon conversion failed",
  "ERR_ICON_NOT_FOUND": "Icon not found",
  "ERR_NAME_EMPTY": "Workspace name cannot be empty",
  "ERR_NAME_INVALID": "Invalid workspace name (only letters, numbers, underscores, and hyphens supported)",
  "ERR_SESSION_NOT_FOUND": "Session no longer exists",
  "ERR_TMUX_NOT_FOUND": "tmux executable not found",
  "ERR_TMUX_NO_SERVER": "tmux server is not running (create a workspace or start tmux)",
  "ERR_TMUX_LIST_FAILED": "Failed to list tmux sessions",
  "ERR_TMUX_GENERIC": "tmux error",
  "ERR_CREATE_FAILED": "Failed to create tmux session",
  "ERR_CREATE_OUTPUT_ERR": "Error creating tmux session",
  "ERR_KILL_FAILED": "Failed to destroy workspace",
  "ERR_KILL_OUTPUT_ERR": "Error destroying workspace",
  "ERR_KILL_PANE_FAILED": "Failed to kill pane",
  "ERR_KILL_PANE_INVALID": "Invalid pane ID format",
  "ERR_KILL_PANE_LAST_IN_SESSION": "The last pane cannot be killed. Destroy the workspace explicitly instead.",
  "ERR_KILL_PANE_NOT_FOUND": "Pane not found",
  "ERR_KILL_PANE_OUTPUT_ERR": "Error killing pane",
  "ERR_NATIVE_WORKSPACE_RENAME_UNSUPPORTED": "Native-split workspaces cannot be renamed while running.",
  "ERR_CREATE_NATIVE_SLOT_SETUP": "Failed to initialize native Agent slot",
  "ERR_NATIVE_SLOT_AGENT_EXITED": "The Agent exited before its native slot finished starting",
  "agent.exitStatus": "exit status {value}",
  "agent.exitSignal": "signal {value}",
  "ERR_KILL_SLOT_INVALID_TARGET": "Invalid native slot target",
  "ERR_KILL_SLOT_NOT_NATIVE": "The target is not a native-split slot",
  "ERR_GHOSTTY_LAYOUT_LOCK_POISONED": "Ghostty layout control is temporarily unavailable",
  "ERR_SWAP_PANE_FAILED": "Failed to swap panes",
  "ERR_READ_ICON": "Failed to read icon",
  "ERR_RENAME_FAILED": "Failed to rename workspace",
  "ERR_RENAME_OUTPUT_ERR": "Error renaming workspace",
  "ERR_SCRIPT_WRITE_FAILED": "Failed to write launch script",
  "ERR_TERMINAL_LAUNCH_FAILED": "Failed to launch terminal",
  "ERR_TERMINAL_NOT_FOUND": "Terminal not found",
  "ERR_TERMINAL_RETURN_ERR": "Terminal exited with error status",

  // Fallbacks & Validation
  "val.enterName": "Please enter a valid workspace name",
  "val.enterCustomCmd": "Please enter a custom command (e.g. claude --model opus)",
  "val.saveCustomFailed": "Failed to save custom agent",
  "val.openTerminalFailed": "Failed to launch terminal",
  "val.createFailed": "Failed to create workspace",
  "val.destroyFailed": "Failed to destroy workspace",
  "val.renameFailed": "Failed to rename workspace",
  "val.dataRefreshFailed": "Failed to refresh data",
};

const zh: Record<string, string> = {
  // App Header
  "app.title": "TmuxDeck",
  "app.subtitle": "tmux 多 Agent 工作区控制台",
  "app.version": "v1.9.3",
  "env.tmux_ok": "Tmux ✓",
  "env.terminals_one": "{n} 个可用终端",
  "env.terminals_other": "{n} 个可用终端",
  "env.agents_one": "{n} 个 Agent",
  "env.agents_other": "{n} 个 Agent",

  // Action Buttons & Tools
  "btn.refresh": "刷新列表",
  "btn.newWorkspace": "新建工作区",
  "btn.open": "打开",
  "btn.openWith": "打开 ({terminal})",
  "btn.cancel": "取消",
  "btn.create": "创建并启动",
  "btn.creating": "创建中...",
  "btn.browse": "浏览...",
  "btn.saveAndApply": "保存并设定",
  "btn.collapse": "收起",
  "btn.copy": "复制",
  "btn.copied": "已复制",
  "btn.recheck": "我已安装，重新检测",

  // Search & Stats
  "search.placeholder": "搜索项目名称...",
  "search.hint": "{total} 个工作区 · {running} 运行中",
  "stats.total_one": "共 {n} 个项目工作区",
  "stats.total_other": "共 {n} 个项目工作区",
  "stats.running": "运行中: {n}",

  // Empty State
  "empty.title": "暂无匹配的 Tmux 工作区",
  "empty.hint": "点击右上角的“新建工作区”快速创建一个包含所需 Agent 的项目卡片",
  "empty.createNow": "立即新建工作区",

  // Cards
  "card.windows_one": "{n} 窗口",
  "card.windows_other": "{n} 窗口",
  "card.panes_one": "{n} 分屏",
  "card.panes_other": "{n} 分屏",
  "card.attached": "活动中 (Attached)",
  "card.runningDetached": "运行中 · 已分离",
  "card.idle": "空闲",
  "card.bgActive": "后台活跃",
  "card.openWithTerminal": "使用 {name} 打开",
  "card.rename": "重命名",
  "card.nativeRenameUnsupported": "原生分屏工作区运行期间暂不支持重命名。",
  "card.drag": "拖动调整工作区顺序",
  "card.destroy": "销毁工作区",
  "card.killPane": "删除此分屏",
  "card.confirmKillPane": "确定删除此分屏格？",
  "card.confirmKillSlot": "确定终止此 Agent 槽位？对应 tmux 会话和进程将结束。",
  "card.confirmKillLastSlot": "确定终止最后一个 Agent 并删除此工作区？对应 tmux 会话和进程将结束。",
  "card.addPane": "新增分屏",
  "card.agentReady": "Agent Ready",
  "card.selectTerminal": "选择启动终端:",

  // Create Workspace Modal
  "modal.createTitle": "新建工作区",
  "modal.sessionNameLabel": "项目/Session 名称 *",
  "modal.sessionNamePlaceholder": "例如: my-ai-project",
  "modal.sessionNameHint": "提示: 名称将自动规范化为: {name}",
  "modal.workingDirLabel": "工作目录",
  "modal.workingDirPlaceholder": "默认 Home 根目录",
  "modal.recentDirs": "最近历史:",
  "modal.agentLabel": "Agent 引擎",
  "modal.panesLabel": "分屏数量",
  "modal.panesCount_one": "{n} 屏",
  "modal.panesCount_other": "{n} 屏",
  "modal.terminalLabel": "启动终端",
  "modal.customAgentChip": "+ 自定义",
  "modal.customAgentTitle": "配置自定义 Agent 命令",
  "modal.customAgentNameLabel": "显示名称 (可选)",
  "modal.customAgentNamePlaceholder": "如: Claude Opus",
  "modal.customAgentCmdLabel": "执行命令 *",
  "modal.customAgentCmdPlaceholder": "如: claude --model opus",
  "modal.summary": "将创建 {panes} {panesText}，每个运行 {agent}，并用 {terminal} 打开。",

  // Missing Tmux Warning
  "tmux.missing.title": "未检测到 Tmux 安装",
  "tmux.missing.hint": "TmuxDeck 依赖 Tmux 来管理多 Agent 会话。请先使用 Homebrew 安装 Tmux：",
  "tmux.missing.win": "未检测到 WSL 或 WSL 内未安装 Tmux",
  "tmux.missing.win_hint": "TmuxDeck 通过 WSL 运行 Tmux，请在 CMD 或 PowerShell 中执行以下命令：",

  // Built-in Names
  "terminal.system": "终端 (系统)",
  "agent.shell": "纯 Shell",
  "agent.custom": "自定义 Agent",

  // Tray Menu
  "tray.activeHeader": "● 当前活跃：{name}",
  "tray.activeIdleHeader": "○ 当前活跃：{name}",
  "tray.open": "打开 ({terminal})",
  "tray.addPane": "新增分屏格",
  "tray.newWorkspace": "＋ 新建工作区…",
  "tray.showMain": "TmuxDeck 主界面",
  "tray.quit": "退出 TmuxDeck",
  "tray.viewMore": "查看全部（共 {n} 个）…",

  // Confirmations
  "confirm.destroy_one": "确定要销毁工作区「{name}」吗？这会终止其中的 {n} 个 tmux 分屏。\n\n若只是暂时离开，请关闭终端窗口（Cmd+W）。此时出现的 Close Window 提示来自终端，指的是终端连接而非该 tmux 工作区；确认关闭应只断开连接，工作区继续后台运行。",
  "confirm.destroy_other": "确定要销毁工作区「{name}」吗？这会终止其中的 {n} 个 tmux 分屏。\n\n若只是暂时离开，请关闭终端窗口（Cmd+W）。此时出现的 Close Window 提示来自终端，指的是终端连接而非该 tmux 工作区；确认关闭应只断开连接，工作区继续后台运行。",
  "confirm.destroyNative_one": "确定销毁工作区「{name}」吗？这会终止最后一个 Agent 并删除工作区。\n\nCmd+W 只会关闭当前 Ghostty 分屏并断开连接，Agent 会继续在后台运行。",
  "confirm.destroyNative_other": "确定销毁工作区「{name}」吗？这会终止全部 {n} 个 Agent 槽位及其 tmux 会话。\n\nCmd+W 只会关闭 Ghostty 分屏并断开连接，Agent 会继续在后台运行。",

  // Error Code Mappings (from Rust)
  "ERR_ADD_PANE_FAILED": "新增分屏失败",
  "ERR_ADD_PANE_OUTPUT_ERR": "新增分屏报错",
  "ERR_CONFIG_SAVE": "保存配置失败",
  "ERR_ICON_CONVERT_FAILED": "图标转换失败",
  "ERR_ICON_NOT_FOUND": "未找到图标",
  "ERR_NAME_EMPTY": "项目名称不能为空",
  "ERR_NAME_INVALID": "非法的项目名称 (仅支持字母、数字、下划线和连字符)",
  "ERR_SESSION_NOT_FOUND": "会话已不存在",
  "ERR_TMUX_NOT_FOUND": "未找到 tmux 安装",
  "ERR_TMUX_NO_SERVER": "tmux 服务未运行（新建工作区可自动启动）",
  "ERR_TMUX_LIST_FAILED": "无法运行 tmux list-sessions",
  "ERR_TMUX_GENERIC": "tmux 错误",
  "ERR_CREATE_FAILED": "创建 tmux session 失败",
  "ERR_CREATE_OUTPUT_ERR": "创建会话报错",
  "ERR_KILL_FAILED": "销毁 session 失败",
  "ERR_KILL_OUTPUT_ERR": "销毁会话失败",
  "ERR_KILL_PANE_FAILED": "删除分屏失败",
  "ERR_KILL_PANE_INVALID": "非法的分屏 ID 格式",
  "ERR_KILL_PANE_LAST_IN_SESSION": "不能删除工作区的最后一个分屏；如需销毁，请明确使用“销毁工作区”。",
  "ERR_KILL_PANE_NOT_FOUND": "未找到该分屏",
  "ERR_KILL_PANE_OUTPUT_ERR": "删除分屏报错",
  "ERR_NATIVE_WORKSPACE_RENAME_UNSUPPORTED": "原生分屏工作区运行期间暂不支持重命名。",
  "ERR_CREATE_NATIVE_SLOT_SETUP": "初始化原生 Agent 槽位失败",
  "ERR_NATIVE_SLOT_AGENT_EXITED": "Agent 在原生槽位启动完成前已退出",
  "agent.exitStatus": "退出状态 {value}",
  "agent.exitSignal": "信号 {value}",
  "ERR_KILL_SLOT_INVALID_TARGET": "原生分屏槽位目标无效",
  "ERR_KILL_SLOT_NOT_NATIVE": "目标不是原生分屏槽位",
  "ERR_GHOSTTY_LAYOUT_LOCK_POISONED": "Ghostty 布局控制暂时不可用",
  "ERR_SWAP_PANE_FAILED": "交换分屏格失败",
  "ERR_READ_ICON": "读取图标失败",
  "ERR_RENAME_FAILED": "重命名 session 失败",
  "ERR_RENAME_OUTPUT_ERR": "重命名会话失败",
  "ERR_SCRIPT_WRITE_FAILED": "写入脚本失败",
  "ERR_TERMINAL_LAUNCH_FAILED": "打开终端失败",
  "ERR_TERMINAL_NOT_FOUND": "未找到指定终端",
  "ERR_TERMINAL_RETURN_ERR": "终端打开返回错误状态",

  // Fallbacks & Validation
  "val.enterName": "请输入有效的项目名称 (支持字母、数字、下划线和连字符)",
  "val.enterCustomCmd": "请输入自定义 Agent 执行命令 (例如: claude --model opus)",
  "val.saveCustomFailed": "保存自定义 Agent 失败",
  "val.openTerminalFailed": "打开终端失败",
  "val.createFailed": "创建失败",
  "val.destroyFailed": "销毁失败",
  "val.renameFailed": "重命名失败",
  "val.dataRefreshFailed": "数据刷新失败",
};

const isZh = typeof navigator !== "undefined" && navigator.language.startsWith("zh");
const lang = isZh ? zh : en;

export function t(key: string, vars?: Record<string, string | number>): string {
  let s = lang[key] ?? en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

export function tPlural(baseKey: string, n: number, vars?: Record<string, string | number>): string {
  const key = n === 1 ? `${baseKey}_one` : `${baseKey}_other`;
  return t(key, { n, ...vars });
}

export function translateName(name: string): string {
  if (name.startsWith("terminal.") || name.startsWith("agent.")) {
    return t(name);
  }
  return name;
}

export function translateError(raw: unknown): string {
  if (typeof raw !== "string") return t("ERR_TMUX_GENERIC");
  const [code, ...details] = raw.split("|");
  const translated = t(code);
  if (code === "ERR_NATIVE_SLOT_AGENT_EXITED" && details.length >= 3) {
    const [target, kind, value] = details;
    const reason = t(kind === "signal" ? "agent.exitSignal" : "agent.exitStatus", {
      value,
    });
    return `${translated}: ${target} (${reason})`;
  }
  if (details[0]) {
    return `${translated}: ${details[0]}`;
  }
  return translated;
}
