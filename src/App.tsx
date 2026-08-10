import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Terminal,
  Plus,
  RefreshCw,
  Play,
  Trash2,
  Edit2,
  Folder,
  CheckCircle2,
  Search,
  LayoutGrid,
  Zap,
  Bot,
  Copy,
  Check,
  ChevronDown,
  Settings,
} from "lucide-react";

interface ToolInfo {
  id: string;
  name: string;
  path: string;
}

interface Environment {
  tmux: string | null;
  terminals: ToolInfo[];
  agents: ToolInfo[];
}

interface CustomAgent {
  name: string;
  command: string;
}

interface Config {
  default_terminal: string;
  default_agent: string;
  default_panes: number;
  custom_agent?: CustomAgent;
  recent_dirs: string[];
}

interface TmuxPane {
  id: string;
  command: string;
  active: boolean;
}

interface TmuxSession {
  id: string;
  name: string;
  windows_count: number;
  panes_count: number;
  attached: boolean;
  created_at: string;
  panes: TmuxPane[];
}

export default function App() {
  const [sessions, setSessions] = useState<TmuxSession[]>([]);
  const [env, setEnv] = useState<Environment | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");
  const [copiedBrew, setCopiedBrew] = useState(false);

  // Modal State
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [newSessionName, setNewSessionName] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [selectedAgent, setSelectedAgent] = useState("pi");
  const [selectedPanes, setSelectedPanes] = useState(4);
  const [selectedTerminal, setSelectedTerminal] = useState("ghostty");

  // Custom Agent Inline Form State
  const [showCustomAgentForm, setShowCustomAgentForm] = useState(false);
  const [customAgentName, setCustomAgentName] = useState("");
  const [customAgentCmd, setCustomAgentCmd] = useState("");

  // Rename State
  const [renamingSession, setRenamingSession] = useState<string | null>(null);
  const [renamedName, setRenamedName] = useState("");

  // Terminal Selector Menu
  const [activeTerminalDropdown, setActiveTerminalDropdown] = useState<string | null>(null);

  const sanitizeNameFrontend = (name: string): string => {
    return name
      .trim()
      .replace(/[^A-Za-z0-9_-]/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-+|-+$/g, "");
  };

  const loadData = async () => {
    setLoading(true);
    setErrorMsg("");
    try {
      const [envData, cfgData, sessionList] = await Promise.all([
        invoke<Environment>("detect_environment"),
        invoke<Config>("load_config"),
        invoke<TmuxSession[]>("get_tmux_sessions"),
      ]);
      setEnv(envData);
      setConfig(cfgData);
      setSessions(sessionList);

      if (cfgData.custom_agent) {
        setCustomAgentName(cfgData.custom_agent.name || "");
        setCustomAgentCmd(cfgData.custom_agent.command || "");
      }

      if (cfgData.default_terminal && envData.terminals.some((t) => t.id === cfgData.default_terminal)) {
        setSelectedTerminal(cfgData.default_terminal);
      } else if (envData.terminals.length > 0) {
        setSelectedTerminal(envData.terminals[0].id);
      }

      if (cfgData.default_agent && envData.agents.some((a) => a.id === cfgData.default_agent)) {
        setSelectedAgent(cfgData.default_agent);
      } else if (envData.agents.length > 0) {
        setSelectedAgent(envData.agents[0].id);
      }

      if (cfgData.default_panes) {
        setSelectedPanes(cfgData.default_panes);
      }
    } catch (err: any) {
      setErrorMsg(err?.toString() || "数据刷新失败");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
    const timer = setInterval(() => {
      loadData();
    }, 4000);
    return () => clearInterval(timer);
  }, []);

  const handlePickDirectory = async () => {
    try {
      const res = await open({
        directory: true,
        multiple: false,
        title: "选择项目工作目录",
      });
      if (res && typeof res === "string") {
        setWorkingDir(res);
      }
    } catch (err) {
      console.error("选择文件夹失败", err);
    }
  };

  const handleSaveCustomAgent = async () => {
    if (!customAgentCmd.trim()) {
      alert("请输入自定义 Agent 执行命令 (例如: claude --model opus)");
      return;
    }
    const newCustom: CustomAgent = {
      name: customAgentName.trim() || "自定义 Agent",
      command: customAgentCmd.trim(),
    };
    try {
      const currentConfig = config || {
        default_terminal: selectedTerminal,
        default_agent: "custom",
        default_panes: selectedPanes,
        recent_dirs: [],
      };
      const updatedConfig: Config = {
        ...currentConfig,
        custom_agent: newCustom,
      };
      await invoke("save_config", { config: updatedConfig });
      const envData = await invoke<Environment>("detect_environment");
      setEnv(envData);
      setConfig(updatedConfig);
      setSelectedAgent("custom");
      setShowCustomAgentForm(false);
    } catch (err: any) {
      alert("保存自定义 Agent 失败: " + err);
    }
  };

  const handleOpenSession = async (sessionName: string, termId?: string) => {
    const targetTerminal = termId || selectedTerminal || (env?.terminals[0]?.id || "terminal");
    try {
      await invoke("open_session", { name: sessionName, terminalId: targetTerminal });
      setActiveTerminalDropdown(null);
    } catch (err: any) {
      alert("打开终端失败: " + err);
    }
  };

  const handleCreate = async () => {
    const cleanName = sanitizeNameFrontend(newSessionName);
    if (!cleanName) {
      alert("请输入有效的项目名称 (支持字母、数字、下划线和连字符)");
      return;
    }
    setLoading(true);
    try {
      await invoke("create_session", {
        opts: {
          name: cleanName,
          dir: workingDir.trim() || null,
          agentId: selectedAgent,
          panes: selectedPanes,
          terminalId: selectedTerminal,
        },
      });
      setShowCreateModal(false);
      setNewSessionName("");
      setWorkingDir("");
      await loadData();
    } catch (err: any) {
      alert("创建失败: " + err);
    } finally {
      setLoading(false);
    }
  };

  const handleKill = async (sessionName: string) => {
    if (!confirm(`确定要销毁工作区 [${sessionName}] 吗？`)) return;
    try {
      await invoke("kill_session", { sessionName });
      await loadData();
    } catch (err: any) {
      alert("销毁失败: " + err);
    }
  };

  const handleRename = async (oldName: string) => {
    const cleanNew = sanitizeNameFrontend(renamedName);
    if (!cleanNew || cleanNew === oldName) {
      setRenamingSession(null);
      return;
    }
    try {
      await invoke("rename_session", { oldName, newName: cleanNew });
      setRenamingSession(null);
      await loadData();
    } catch (err: any) {
      alert("重命名失败: " + err);
    }
  };

  const copyBrewCommand = () => {
    navigator.clipboard.writeText("brew install tmux");
    setCopiedBrew(true);
    setTimeout(() => setCopiedBrew(false), 2000);
  };

  const filteredSessions = sessions.filter((s) =>
    s.name.toLowerCase().includes(search.toLowerCase())
  );

  // 硬阻断：当系统缺失 tmux 时全屏引导
  if (env && env.tmux === null) {
    return (
      <div className="flex flex-col items-center justify-center h-screen bg-slate-950 text-slate-100 p-6 select-none">
        <div className="max-w-md w-full p-8 rounded-3xl bg-slate-900 border border-slate-800 shadow-2xl text-center space-y-6">
          <div className="w-16 h-16 rounded-2xl bg-rose-950/60 border border-rose-800/80 text-rose-400 flex items-center justify-center mx-auto">
            <Terminal className="w-8 h-8" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-slate-100">未检测到 Tmux 安装</h2>
            <p className="text-sm text-slate-400 mt-2">
              TmuxDeck 依赖 Tmux 来管理多 Agent 会话。请先使用 Homebrew 安装 Tmux：
            </p>
          </div>
          <div className="flex items-center justify-between p-3.5 rounded-xl bg-slate-950 border border-slate-800 font-mono text-sm">
            <span className="text-cyan-400">brew install tmux</span>
            <button
              onClick={copyBrewCommand}
              className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition flex items-center space-x-1 cursor-pointer"
            >
              {copiedBrew ? (
                <>
                  <Check className="w-4 h-4 text-emerald-400" />
                  <span className="text-xs text-emerald-400 font-sans">已复制</span>
                </>
              ) : (
                <>
                  <Copy className="w-4 h-4" />
                  <span className="text-xs font-sans">复制</span>
                </>
              )}
            </button>
          </div>
          <button
            onClick={loadData}
            className="w-full py-2.5 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium text-sm shadow-lg shadow-cyan-500/20 transition cursor-pointer"
          >
            我已安装，重新检测
          </button>
        </div>
      </div>
    );
  }

  const currentTerminalObj = env?.terminals.find((t) => t.id === selectedTerminal) || env?.terminals[0];
  const currentAgentObj = env?.agents.find((a) => a.id === selectedAgent) || env?.agents[0];

  return (
    <div className="flex flex-col h-screen bg-slate-950 text-slate-100 font-sans select-none">
      {/* 顶部 App Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900/60 backdrop-blur-md">
        <div className="flex items-center space-x-3">
          <div className="p-2 rounded-xl bg-gradient-to-tr from-cyan-500 to-blue-600 shadow-lg shadow-cyan-500/20">
            <LayoutGrid className="w-6 h-6 text-white" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <h1 className="text-xl font-bold bg-gradient-to-r from-white via-slate-200 to-slate-400 bg-clip-text text-transparent">
                TmuxDeck
              </h1>
              <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-cyan-950 text-cyan-400 border border-cyan-800">
                v1.1
              </span>
            </div>
            <p className="text-xs text-slate-400">
              tmux 多 Agent 工作区控制台
            </p>
          </div>
        </div>

        {/* 顶部环境指示器 */}
        <div className="hidden md:flex items-center space-x-4 px-3 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs">
          <div className="flex items-center space-x-1.5">
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
            <span className="text-slate-300 font-medium">Tmux ✓</span>
          </div>
          <span className="text-slate-700">|</span>
          <div className="flex items-center space-x-2 text-slate-400">
            <span>{env?.terminals.length || 0} 个可用终端</span>
            <span>·</span>
            <span>{env?.agents.length || 0} 个 Agent</span>
          </div>
        </div>

        {/* 顶部操作区 */}
        <div className="flex items-center space-x-3">
          <button
            onClick={loadData}
            disabled={loading}
            className="p-2 rounded-lg bg-slate-900 border border-slate-800 text-slate-300 hover:text-white hover:bg-slate-800 transition disabled:opacity-50 cursor-pointer"
            title="刷新列表"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
          </button>
          <button
            onClick={() => {
              setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
              setShowCreateModal(true);
            }}
            className="flex items-center space-x-2 px-4 py-2 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium shadow-lg shadow-cyan-500/25 transition text-sm cursor-pointer"
          >
            <Plus className="w-4 h-4" />
            <span>新建工作区</span>
          </button>
        </div>
      </header>

      {/* 搜索与统计栏 */}
      <div className="flex items-center justify-between px-6 py-3 bg-slate-900/40 border-b border-slate-800/60">
        <div className="relative w-72">
          <Search className="w-4 h-4 absolute left-3 top-2.5 text-slate-500" />
          <input
            type="text"
            placeholder="搜索项目名称..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full pl-9 pr-4 py-1.5 text-sm bg-slate-900 border border-slate-800 rounded-lg text-slate-200 focus:outline-none focus:border-cyan-500 transition"
          />
        </div>
        <div className="text-xs text-slate-400 flex items-center space-x-4">
          <span>共 <strong className="text-cyan-400">{sessions.length}</strong> 个项目工作区</span>
          <span>运行中: <strong className="text-emerald-400">{sessions.filter((s) => s.attached).length}</strong></span>
        </div>
      </div>

      {/* 主体卡片区域 */}
      <main className="flex-1 overflow-y-auto p-6">
        {errorMsg && (
          <div className="mb-6 p-4 rounded-xl bg-rose-950/40 border border-rose-800/60 text-rose-300 text-sm">
            {errorMsg}
          </div>
        )}

        {filteredSessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-center">
            <div className="p-4 rounded-2xl bg-slate-900/80 border border-slate-800 mb-4">
              <Terminal className="w-10 h-10 text-slate-600" />
            </div>
            <h3 className="text-lg font-semibold text-slate-300">暂无匹配的 Tmux 工作区</h3>
            <p className="text-sm text-slate-500 max-w-sm mt-1 mb-4">
              点击右上角的“新建工作区”快速创建一个包含所需 Agent 的项目卡片
            </p>
            <button
              onClick={() => {
                setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
                setShowCreateModal(true);
              }}
              className="flex items-center space-x-2 px-4 py-2 rounded-xl bg-slate-900 border border-slate-800 hover:bg-slate-800 text-cyan-400 text-sm font-medium transition cursor-pointer"
            >
              <Plus className="w-4 h-4" />
              <span>矢量新建工作区</span>
            </button>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-5">
            {filteredSessions.map((session) => {
              const isRenaming = renamingSession === session.name;
              const mainCmds = session.panes.map((p) => p.command).filter(Boolean);
              const isAgentActive = env?.agents.some((a) =>
                mainCmds.some((c) => c.includes(a.id) || (a.path && c.includes(a.path)))
              );

              const gridCols =
                session.panes_count === 1
                  ? "grid-cols-1"
                  : session.panes_count === 2
                  ? "grid-cols-2"
                  : session.panes_count === 6
                  ? "grid-cols-3"
                  : "grid-cols-2";

              return (
                <div
                  key={session.id}
                  className="flex flex-col justify-between rounded-2xl bg-slate-900/80 border border-slate-800 hover:border-cyan-500/50 transition shadow-lg group hover:shadow-cyan-500/5"
                >
                  {/* 卡片头部 */}
                  <div className="p-4 border-b border-slate-800/60">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center space-x-2 min-w-0 flex-1">
                        <span
                          className={`w-2.5 h-2.5 rounded-full shrink-0 ${
                            session.attached
                              ? "bg-emerald-400 shadow-sm shadow-emerald-400/80 animate-pulse"
                              : "bg-slate-600"
                          }`}
                          title={session.attached ? "活动中 Attached" : "空闲 Idle"}
                        />
                        {isRenaming ? (
                          <input
                            type="text"
                            value={renamedName}
                            maxLength={60}
                            onChange={(e) => setRenamedName(e.target.value)}
                            onBlur={() => handleRename(session.name)}
                            onKeyDown={(e) => e.key === "Enter" && handleRename(session.name)}
                            autoFocus
                            className="bg-slate-950 px-2 py-0.5 border border-cyan-500 rounded text-sm text-white w-full"
                          />
                        ) : (
                          <h2 className="font-semibold text-slate-100 truncate text-base group-hover:text-cyan-300 transition">
                            {session.name}
                          </h2>
                        )}
                      </div>
                      <div className="flex items-center space-x-1 shrink-0">
                        <button
                          onClick={() => {
                            setRenamingSession(session.name);
                            setRenamedName(session.name);
                          }}
                          className="p-1 rounded text-slate-500 hover:text-slate-300 hover:bg-slate-800 opacity-0 group-hover:opacity-100 transition cursor-pointer"
                          title="重命名"
                        >
                          <Edit2 className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => handleKill(session.name)}
                          className="p-1 rounded text-slate-500 hover:text-rose-400 hover:bg-slate-800 opacity-0 group-hover:opacity-100 transition cursor-pointer"
                          title="销毁 Session"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>

                    <div className="flex items-center justify-between text-xs text-slate-400">
                      <span>{session.windows_count} 窗口 · {session.panes_count} 分屏</span>
                      <span>{session.created_at}</span>
                    </div>
                  </div>

                  {/* 屏阵列布局预览 */}
                  <div className="p-4 flex-1">
                    <div className="text-xs text-slate-500 mb-2 font-medium flex items-center justify-between">
                      <span>分屏预览:</span>
                      {isAgentActive && (
                        <span className="flex items-center space-x-1 text-cyan-400 text-[10px]">
                          <Zap className="w-3 h-3" />
                          <span>Agent Ready</span>
                        </span>
                      )}
                    </div>
                    <div className={`grid ${gridCols} gap-2 p-2 rounded-xl bg-slate-950/80 border border-slate-800/80`}>
                      {session.panes.map((pane, idx) => {
                        const cmdName = pane.command || "shell";
                        const matchedAgent = env?.agents.find(
                          (a) => a.id !== "shell" && (cmdName.includes(a.id) || (a.path && cmdName.includes(a.path)))
                        );
                        const isAgent = Boolean(matchedAgent);
                        return (
                          <div
                            key={pane.id || idx}
                            className={`flex flex-col justify-between p-2 rounded-lg border text-[11px] h-12 transition ${
                              isAgent
                                ? "bg-cyan-950/30 border-cyan-800/40 text-cyan-300"
                                : "bg-slate-900/60 border-slate-800 text-slate-400"
                            }`}
                          >
                            <div className="flex items-center justify-between">
                              <span className="font-mono text-[9px] text-slate-500">#{idx + 1}</span>
                              {isAgent && <Zap className="w-2.5 h-2.5 text-cyan-400" />}
                            </div>
                            <span className="font-mono truncate font-medium">
                              {matchedAgent ? matchedAgent.name : cmdName}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  {/* 卡片底部操作按钮 */}
                  <div className="p-3 border-t border-slate-800/60 bg-slate-900/40 relative">
                    <div className="flex items-center space-x-1">
                      <button
                        onClick={() => handleOpenSession(session.name)}
                        className="flex-1 flex items-center justify-center space-x-2 py-2 px-3 rounded-xl bg-slate-800 hover:bg-gradient-to-r hover:from-cyan-600 hover:to-blue-600 text-slate-200 hover:text-white text-sm font-medium transition shadow-sm group-hover:bg-cyan-600/20 group-hover:text-cyan-300 group-hover:border group-hover:border-cyan-500/40 cursor-pointer"
                        title={`使用 ${currentTerminalObj?.name || "终端"} 打开`}
                      >
                        <Play className="w-3.5 h-3.5 fill-current" />
                        <span>打开 ({currentTerminalObj?.name || "终端"})</span>
                      </button>

                      {env && env.terminals.length > 1 && (
                        <button
                          onClick={() =>
                            setActiveTerminalDropdown(
                              activeTerminalDropdown === session.name ? null : session.name
                            )
                          }
                          className="p-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white transition cursor-pointer"
                          title="选择其他终端打开"
                        >
                          <ChevronDown className="w-4 h-4" />
                        </button>
                      )}
                    </div>

                    {activeTerminalDropdown === session.name && (
                      <div className="absolute right-3 bottom-14 z-20 w-44 rounded-xl bg-slate-900 border border-slate-700 shadow-xl py-1">
                        <div className="px-3 py-1 text-[10px] font-semibold text-slate-400 border-b border-slate-800">
                          选择启动终端:
                        </div>
                        {env?.terminals.map((term) => (
                          <button
                            key={term.id}
                            onClick={() => handleOpenSession(session.name, term.id)}
                            className="w-full text-left px-3 py-1.5 text-xs text-slate-200 hover:bg-cyan-950 hover:text-cyan-300 flex items-center justify-between cursor-pointer"
                          >
                            <span>{term.name}</span>
                            {term.id === selectedTerminal && (
                              <Check className="w-3 h-3 text-cyan-400" />
                            )}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </main>

      {/* 新建项目 Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/80 backdrop-blur-sm p-4">
          <div className="w-full max-w-lg rounded-2xl bg-slate-900 border border-slate-800 shadow-2xl p-6">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2">
                <div className="p-2 rounded-lg bg-cyan-950 border border-cyan-800 text-cyan-400">
                  <Bot className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-slate-100">新建工作区</h3>
              </div>
              <button
                onClick={() => setShowCreateModal(false)}
                className="text-slate-400 hover:text-white text-lg font-bold cursor-pointer"
              >
                ✕
              </button>
            </div>

            <div className="space-y-4">
              {/* 项目名称 */}
              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1">
                  项目/Session 名称 *
                </label>
                <input
                  type="text"
                  placeholder="例如: my-ai-project"
                  value={newSessionName}
                  maxLength={60}
                  onChange={(e) => setNewSessionName(e.target.value)}
                  className="w-full px-3 py-2 text-sm bg-slate-950 border border-slate-800 rounded-xl text-slate-100 focus:outline-none focus:border-cyan-500"
                />
                {newSessionName && sanitizeNameFrontend(newSessionName) !== newSessionName && (
                  <p className="text-[10px] text-amber-400 mt-1">
                    提示: 名称将自动规范化为 <code className="font-mono">{sanitizeNameFrontend(newSessionName)}</code>
                  </p>
                )}
              </div>

              {/* 工作目录与系统选择器 */}
              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1">
                  工作目录
                </label>
                <div className="flex items-center space-x-2">
                  <div className="relative flex-1">
                    <Folder className="w-4 h-4 absolute left-3 top-2.5 text-slate-500" />
                    <input
                      type="text"
                      placeholder="默认 Home 根目录"
                      value={workingDir}
                      onChange={(e) => setWorkingDir(e.target.value)}
                      className="w-full pl-9 pr-3 py-2 text-sm bg-slate-950 border border-slate-800 rounded-xl text-slate-100 focus:outline-none focus:border-cyan-500"
                    />
                  </div>
                  <button
                    type="button"
                    onClick={handlePickDirectory}
                    className="px-3 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium transition flex items-center space-x-1 shrink-0 cursor-pointer"
                  >
                    <Folder className="w-3.5 h-3.5 text-cyan-400" />
                    <span>浏览...</span>
                  </button>
                </div>

                {config && config.recent_dirs && config.recent_dirs.length > 0 && (
                  <div className="flex items-center space-x-1.5 mt-2 flex-wrap gap-y-1">
                    <span className="text-[10px] text-slate-500">最近历史:</span>
                    {config.recent_dirs.map((dir) => (
                      <button
                        key={dir}
                        type="button"
                        onClick={() => setWorkingDir(dir)}
                        className="px-2 py-0.5 rounded-md bg-slate-950 hover:bg-slate-800 border border-slate-800 text-[10px] text-slate-300 font-mono truncate max-w-[150px] transition cursor-pointer"
                        title={dir}
                      >
                        {dir.split("/").pop() || dir}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Agent 选择 Segmented Chips */}
              {env && (
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <label className="block text-xs font-medium text-slate-400">
                      Agent 引擎
                    </label>
                  </div>
                  <div className="flex items-center space-x-2 flex-wrap gap-y-2">
                    {env.agents.map((agent) => {
                      const isSelected = selectedAgent === agent.id;
                      return (
                        <button
                          key={agent.id}
                          type="button"
                          onClick={() => {
                            setSelectedAgent(agent.id);
                            setShowCustomAgentForm(false);
                          }}
                          className={`px-3 py-1.5 rounded-xl text-xs font-medium transition cursor-pointer flex items-center space-x-1 ${
                            isSelected
                              ? "bg-cyan-500 text-slate-950 font-bold shadow-md shadow-cyan-500/20"
                              : "bg-slate-950 hover:bg-slate-800 border border-slate-800 text-slate-300"
                          }`}
                        >
                          <span>{agent.name}</span>
                          {agent.id === "custom" && (
                            <Settings
                              className="w-3 h-3 ml-1 opacity-75 hover:opacity-100"
                              onClick={(e) => {
                                e.stopPropagation();
                                setShowCustomAgentForm(true);
                              }}
                            />
                          )}
                        </button>
                      );
                    })}

                    {/* + 自定义 Chip 按钮 */}
                    {!env.agents.some((a) => a.id === "custom") && (
                      <button
                        type="button"
                        onClick={() => setShowCustomAgentForm(!showCustomAgentForm)}
                        className={`px-3 py-1.5 rounded-xl text-xs font-medium border border-dashed transition cursor-pointer ${
                          showCustomAgentForm
                            ? "bg-cyan-950 border-cyan-500 text-cyan-300"
                            : "border-slate-700 text-slate-400 hover:text-slate-200 hover:border-slate-500"
                        }`}
                      >
                        + 自定义
                      </button>
                    )}
                  </div>

                  {/* 自定义 Agent 行内编辑面板 (PRD 2.3) */}
                  {showCustomAgentForm && (
                    <div className="mt-3 p-3 rounded-xl bg-slate-950 border border-cyan-900/60 space-y-3">
                      <div className="text-xs font-semibold text-cyan-400 flex items-center justify-between">
                        <span>配置自定义 Agent 命令</span>
                        <button
                          onClick={() => setShowCustomAgentForm(false)}
                          className="text-slate-500 hover:text-slate-300 text-xs"
                        >
                          收起
                        </button>
                      </div>
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                        <div>
                          <label className="block text-[10px] text-slate-400 mb-1">显示名称 (可选)</label>
                          <input
                            type="text"
                            placeholder="如: Claude Opus"
                            value={customAgentName}
                            onChange={(e) => setCustomAgentName(e.target.value)}
                            className="w-full px-2.5 py-1.5 text-xs bg-slate-900 border border-slate-800 rounded-lg text-slate-200 focus:outline-none focus:border-cyan-500"
                          />
                        </div>
                        <div>
                          <label className="block text-[10px] text-slate-400 mb-1">执行命令 *</label>
                          <input
                            type="text"
                            placeholder="如: claude --model opus"
                            value={customAgentCmd}
                            onChange={(e) => setCustomAgentCmd(e.target.value)}
                            className="w-full px-2.5 py-1.5 text-xs bg-slate-900 border border-slate-800 rounded-lg text-slate-200 focus:outline-none focus:border-cyan-500 font-mono"
                          />
                        </div>
                      </div>
                      <div className="flex justify-end">
                        <button
                          type="button"
                          onClick={handleSaveCustomAgent}
                          className="px-3 py-1 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-white text-xs font-medium transition cursor-pointer"
                        >
                          保存并设定
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {/* 分屏数量 Segmented Chips */}
              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1.5">
                  分屏数量
                </label>
                <div className="flex items-center space-x-2">
                  {[1, 2, 4, 6].map((p) => {
                    const isSelected = selectedPanes === p;
                    return (
                      <button
                        key={p}
                        type="button"
                        onClick={() => setSelectedPanes(p)}
                        className={`flex-1 py-1.5 rounded-xl text-xs font-medium text-center transition cursor-pointer ${
                          isSelected
                            ? "bg-cyan-500 text-slate-950 font-bold shadow-md shadow-cyan-500/20"
                            : "bg-slate-950 hover:bg-slate-800 border border-slate-800 text-slate-300"
                        }`}
                      >
                        {p} 屏
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* 终端选择 (如仅1候选则整行隐藏) */}
              {env && env.terminals.length > 1 && (
                <div>
                  <label className="block text-xs font-medium text-slate-400 mb-1.5">
                    启动终端
                  </label>
                  <div className="flex items-center space-x-2 flex-wrap gap-y-2">
                    {env.terminals.map((term) => {
                      const isSelected = selectedTerminal === term.id;
                      return (
                        <button
                          key={term.id}
                          type="button"
                          onClick={() => setSelectedTerminal(term.id)}
                          className={`px-3 py-1.5 rounded-xl text-xs font-medium transition cursor-pointer ${
                            isSelected
                              ? "bg-cyan-500 text-slate-950 font-bold shadow-md shadow-cyan-500/20"
                              : "bg-slate-950 hover:bg-slate-800 border border-slate-800 text-slate-300"
                          }`}
                        >
                          {term.name}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* 动态配置说明 */}
              <div className="p-3 rounded-xl bg-slate-950/60 border border-slate-800/60 text-xs text-slate-400">
                将创建 <strong className="text-cyan-400">{selectedPanes}</strong> 个分屏，每个运行{" "}
                <strong className="text-cyan-400">{currentAgentObj?.name || selectedAgent}</strong>，并用{" "}
                <strong className="text-cyan-400">{currentTerminalObj?.name || selectedTerminal}</strong> 打开。
              </div>
            </div>

            <div className="flex items-center justify-end space-x-3 mt-6">
              <button
                onClick={() => setShowCreateModal(false)}
                className="px-4 py-2 rounded-xl text-sm font-medium text-slate-400 hover:text-slate-200 cursor-pointer"
              >
                取消
              </button>
              <button
                onClick={handleCreate}
                disabled={loading}
                className="flex items-center space-x-2 px-5 py-2 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium text-sm transition shadow-lg shadow-cyan-500/20 disabled:opacity-50 cursor-pointer"
              >
                <Plus className="w-4 h-4" />
                <span>{loading ? "创建中..." : "创建并启动"}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
