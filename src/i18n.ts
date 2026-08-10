const en: Record<string, string> = {
  // App Header
  "app.title": "TmuxDeck",
  "app.subtitle": "Multi-agent workspace console for tmux",
  "app.version": "v1.2",
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
  "card.idle": "Idle",
  "card.rename": "Rename",
  "card.destroy": "Destroy workspace",
  "card.panePreview": "Pane Layout",
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

  // Built-in Names
  "terminal.system": "Terminal (System)",
  "agent.shell": "Plain Shell",
  "agent.custom": "Custom Agent",

  // Confirmations
  "confirm.destroy": "Destroy workspace \"{name}\"?",

  // Error Code Mappings (from Rust)
  "ERR_NAME_EMPTY": "Workspace name cannot be empty",
  "ERR_NAME_INVALID": "Invalid workspace name (only letters, numbers, underscores, and hyphens supported)",
  "ERR_TMUX_NOT_FOUND": "tmux executable not found",
  "ERR_TMUX_LIST_FAILED": "Failed to list tmux sessions",
  "ERR_TMUX_GENERIC": "tmux error",
  "ERR_CREATE_FAILED": "Failed to create tmux session",
  "ERR_CREATE_OUTPUT_ERR": "Error creating tmux session",
  "ERR_KILL_FAILED": "Failed to destroy workspace",
  "ERR_KILL_OUTPUT_ERR": "Error destroying workspace",
  "ERR_RENAME_FAILED": "Failed to rename workspace",
  "ERR_RENAME_OUTPUT_ERR": "Error renaming workspace",
  "ERR_SCRIPT_WRITE_FAILED": "Failed to write launch script",
  "ERR_TERMINAL_LAUNCH_FAILED": "Failed to launch terminal",
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
  "app.version": "v1.2",
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
  "card.idle": "空闲 (Idle)",
  "card.rename": "重命名",
  "card.destroy": "销毁工作区",
  "card.panePreview": "分屏预览",
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

  // Built-in Names
  "terminal.system": "终端 (系统)",
  "agent.shell": "纯 Shell",
  "agent.custom": "自定义 Agent",

  // Confirmations
  "confirm.destroy": "确定要销毁工作区「{name}」吗？",

  // Error Code Mappings (from Rust)
  "ERR_NAME_EMPTY": "项目名称不能为空",
  "ERR_NAME_INVALID": "非法的项目名称 (仅支持字母、数字、下划线和连字符)",
  "ERR_TMUX_NOT_FOUND": "未找到 tmux 安装",
  "ERR_TMUX_LIST_FAILED": "无法运行 tmux list-sessions",
  "ERR_TMUX_GENERIC": "tmux 错误",
  "ERR_CREATE_FAILED": "创建 tmux session 失败",
  "ERR_CREATE_OUTPUT_ERR": "创建会话报错",
  "ERR_KILL_FAILED": "销毁 session 失败",
  "ERR_KILL_OUTPUT_ERR": "销毁会话失败",
  "ERR_RENAME_FAILED": "重命名 session 失败",
  "ERR_RENAME_OUTPUT_ERR": "重命名会话失败",
  "ERR_SCRIPT_WRITE_FAILED": "写入脚本失败",
  "ERR_TERMINAL_LAUNCH_FAILED": "打开终端失败",
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
  const [code, details] = raw.split("|");
  const translated = t(code);
  if (details) {
    return `${translated}: ${details}`;
  }
  return translated;
}
