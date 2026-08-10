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
import { t, tPlural, translateName, translateError } from "./i18n";

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
  last_active_ts: number;
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

  const getSessionActivityInfo = (session: TmuxSession) => {
    if (session.attached) {
      return {
        statusClass: "bg-emerald-400 shadow-sm shadow-emerald-400/80 animate-pulse",
        statusTooltip: t("card.attached"),
        text: t("card.activeState"),
      };
    }

    const now = Math.floor(Date.now() / 1000);
    const elapsed = session.last_active_ts > 0 ? Math.max(0, now - session.last_active_ts) : -1;

    if (elapsed >= 0 && elapsed < 600) {
      let timeText = "";
      if (elapsed < 60) {
        timeText = t("card.lastActive_now");
      } else if (elapsed < 3600) {
        timeText = t("card.lastActive", { time: `${Math.floor(elapsed / 60)}m` });
      } else if (elapsed < 86400) {
        timeText = t("card.lastActive", { time: `${Math.floor(elapsed / 3600)}h` });
      } else {
        timeText = t("card.lastActive", { time: `${Math.floor(elapsed / 86400)}d` });
      }

      return {
        statusClass: "bg-amber-400 shadow-sm shadow-amber-400/80",
        statusTooltip: t("card.bgActive"),
        text: timeText,
      };
    }

    let idleText = t("card.idle");
    if (elapsed >= 600) {
      if (elapsed < 3600) {
        idleText = t("card.lastActive", { time: `${Math.floor(elapsed / 60)}m` });
      } else if (elapsed < 86400) {
        idleText = t("card.lastActive", { time: `${Math.floor(elapsed / 3600)}h` });
      } else {
        idleText = t("card.lastActive", { time: `${Math.floor(elapsed / 86400)}d` });
      }
    }

    return {
      statusClass: "bg-slate-600",
      statusTooltip: t("card.idle"),
      text: idleText,
    };
  };

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
      setErrorMsg(translateError(err) || t("val.dataRefreshFailed"));
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
        title: t("modal.workingDirLabel"),
      });
      if (res && typeof res === "string") {
        const converted = await invoke<string>("to_wsl_path", { path: res });
        setWorkingDir(converted);
      }
    } catch (err) {
      console.error("Folder picker error", err);
    }
  };

  const handleSaveCustomAgent = async () => {
    if (!customAgentCmd.trim()) {
      alert(t("val.enterCustomCmd"));
      return;
    }
    const newCustom: CustomAgent = {
      name: customAgentName.trim() || t("agent.custom"),
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
      alert(t("val.saveCustomFailed") + ": " + translateError(err));
    }
  };

  const handleOpenSession = async (sessionName: string, termId?: string) => {
    const targetTerminal = termId || selectedTerminal || (env?.terminals[0]?.id || "terminal");
    try {
      await invoke("open_session", { name: sessionName, terminalId: targetTerminal });
      setActiveTerminalDropdown(null);
    } catch (err: any) {
      alert(t("val.openTerminalFailed") + ": " + translateError(err));
    }
  };

  const handleCreate = async () => {
    const cleanName = sanitizeNameFrontend(newSessionName);
    if (!cleanName) {
      alert(t("val.enterName"));
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
      alert(t("val.createFailed") + ": " + translateError(err));
    } finally {
      setLoading(false);
    }
  };

  const handleKill = async (sessionName: string) => {
    if (!confirm(t("confirm.destroy", { name: sessionName }))) return;
    try {
      await invoke("kill_session", { sessionName });
      await loadData();
    } catch (err: any) {
      alert(t("val.destroyFailed") + ": " + translateError(err));
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
      alert(t("val.renameFailed") + ": " + translateError(err));
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

  // Hard blocking: full-screen guidance when tmux is missing
  if (env && env.tmux === null) {
    const isWindows = env.terminals.some((t) => t.id === "wt" || t.id === "cmd" || t.id === "powershell");
    return (
      <div className="flex flex-col items-center justify-center h-screen bg-slate-950 text-slate-100 p-6 select-none">
        <div className="max-w-md w-full p-8 rounded-3xl bg-slate-900 border border-slate-800 shadow-2xl text-center space-y-6">
          <div className="w-16 h-16 rounded-2xl bg-rose-950/60 border border-rose-800/80 text-rose-400 flex items-center justify-center mx-auto">
            <Terminal className="w-8 h-8" />
          </div>
          <div>
            <h2 className="text-xl font-bold text-slate-100">
              {isWindows ? t("tmux.missing.win") : t("tmux.missing.title")}
            </h2>
            <p className="text-sm text-slate-400 mt-2">
              {isWindows ? t("tmux.missing.win_hint") : t("tmux.missing.hint")}
            </p>
          </div>
          <div className="flex flex-col space-y-2">
            {isWindows ? (
              <>
                <div className="flex items-center justify-between p-3 rounded-xl bg-slate-950 border border-slate-800 font-mono text-xs">
                  <span className="text-cyan-400">wsl --install</span>
                  <button
                    onClick={() => {
                      navigator.clipboard.writeText("wsl --install");
                      setCopiedBrew(true);
                      setTimeout(() => setCopiedBrew(false), 2000);
                    }}
                    className="p-1 text-slate-400 hover:text-white cursor-pointer"
                  >
                    {copiedBrew ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                  </button>
                </div>
                <div className="flex items-center justify-between p-3 rounded-xl bg-slate-950 border border-slate-800 font-mono text-xs">
                  <span className="text-cyan-400">wsl sudo apt install tmux</span>
                  <button
                    onClick={() => {
                      navigator.clipboard.writeText("wsl sudo apt install tmux");
                      setCopiedBrew(true);
                      setTimeout(() => setCopiedBrew(false), 2000);
                    }}
                    className="p-1 text-slate-400 hover:text-white cursor-pointer"
                  >
                    {copiedBrew ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                  </button>
                </div>
              </>
            ) : (
              <div className="flex items-center justify-between p-3.5 rounded-xl bg-slate-950 border border-slate-800 font-mono text-sm">
                <span className="text-cyan-400">brew install tmux</span>
                <button
                  onClick={copyBrewCommand}
                  className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition flex items-center space-x-1 cursor-pointer"
                >
                  {copiedBrew ? (
                    <>
                      <Check className="w-4 h-4 text-emerald-400" />
                      <span className="text-xs text-emerald-400 font-sans">{t("btn.copied")}</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-4 h-4" />
                      <span className="text-xs font-sans">{t("btn.copy")}</span>
                    </>
                  )}
                </button>
              </div>
            )}
          </div>
          <button
            onClick={loadData}
            className="w-full py-2.5 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium text-sm shadow-lg shadow-cyan-500/20 transition cursor-pointer"
          >
            {t("btn.recheck")}
          </button>
        </div>
      </div>
    );
  }

  const currentTerminalObj = env?.terminals.find((t) => t.id === selectedTerminal) || env?.terminals[0];
  const currentAgentObj = env?.agents.find((a) => a.id === selectedAgent) || env?.agents[0];

  return (
    <div className="flex flex-col h-screen bg-slate-950 text-slate-100 font-sans select-none">
      {/* App Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900/60 backdrop-blur-md">
        <div className="flex items-center space-x-3">
          <div className="p-2 rounded-xl bg-gradient-to-tr from-cyan-500 to-blue-600 shadow-lg shadow-cyan-500/20">
            <LayoutGrid className="w-6 h-6 text-white" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <h1 className="text-xl font-bold bg-gradient-to-r from-white via-slate-200 to-slate-400 bg-clip-text text-transparent">
                {t("app.title")}
              </h1>
              <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-cyan-950 text-cyan-400 border border-cyan-800">
                {t("app.version")}
              </span>
            </div>
            <p className="text-xs text-slate-400">
              {t("app.subtitle")}
            </p>
          </div>
        </div>

        {/* Environment Status Indicator */}
        <div className="hidden md:flex items-center space-x-4 px-3 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs">
          <div className="flex items-center space-x-1.5">
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
            <span className="text-slate-300 font-medium">{t("env.tmux_ok")}</span>
          </div>
          <span className="text-slate-700">|</span>
          <div className="flex items-center space-x-2 text-slate-400">
            <span>{tPlural("env.terminals", env?.terminals.length || 0)}</span>
            <span>·</span>
            <span>{tPlural("env.agents", env?.agents.length || 0)}</span>
          </div>
        </div>

        {/* Top Header Actions */}
        <div className="flex items-center space-x-3">
          <button
            onClick={loadData}
            disabled={loading}
            className="p-2 rounded-lg bg-slate-900 border border-slate-800 text-slate-300 hover:text-white hover:bg-slate-800 transition disabled:opacity-50 cursor-pointer"
            title={t("btn.refresh")}
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
            <span>{t("btn.newWorkspace")}</span>
          </button>
        </div>
      </header>

      {/* Search & Statistics Bar */}
      <div className="flex items-center justify-between px-6 py-3 bg-slate-900/40 border-b border-slate-800/60">
        <div className="relative w-72">
          <Search className="w-4 h-4 absolute left-3 top-2.5 text-slate-500" />
          <input
            type="text"
            placeholder={t("search.placeholder")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full pl-9 pr-4 py-1.5 text-sm bg-slate-900 border border-slate-800 rounded-lg text-slate-200 focus:outline-none focus:border-cyan-500 transition"
          />
        </div>
        <div className="text-xs text-slate-400 flex items-center space-x-4">
          <span>{tPlural("stats.total", sessions.length)}</span>
          <span>{t("stats.running", { n: sessions.filter((s) => s.attached).length })}</span>
        </div>
      </div>

      {/* Main Workspace Grid */}
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
            <h3 className="text-lg font-semibold text-slate-300">{t("empty.title")}</h3>
            <p className="text-sm text-slate-500 max-w-sm mt-1 mb-4">
              {t("empty.hint")}
            </p>
            <button
              onClick={() => {
                setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
                setShowCreateModal(true);
              }}
              className="flex items-center space-x-2 px-4 py-2 rounded-xl bg-slate-900 border border-slate-800 hover:bg-slate-800 text-cyan-400 text-sm font-medium transition cursor-pointer"
            >
              <Plus className="w-4 h-4" />
              <span>{t("empty.createNow")}</span>
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
              const activityInfo = getSessionActivityInfo(session);

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
                  {/* Card Header */}
                  <div className="p-4 border-b border-slate-800/60">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center space-x-2 min-w-0 flex-1">
                        <span
                          className={`w-2.5 h-2.5 rounded-full shrink-0 ${activityInfo.statusClass}`}
                          title={activityInfo.statusTooltip}
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
                          title={t("card.rename")}
                        >
                          <Edit2 className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => handleKill(session.name)}
                          className="p-1 rounded text-slate-500 hover:text-rose-400 hover:bg-slate-800 opacity-0 group-hover:opacity-100 transition cursor-pointer"
                          title={t("card.destroy")}
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>

                    <div className="flex items-center justify-between text-xs text-slate-400">
                      <span>{tPlural("card.windows", session.windows_count)} · {tPlural("card.panes", session.panes_count)}</span>
                      <span>{activityInfo.text}</span>
                    </div>
                  </div>

                  {/* Pane Layout Preview */}
                  <div className="p-4 flex-1">
                    <div className="text-xs text-slate-500 mb-2 font-medium flex items-center justify-between">
                      <span>{t("card.panePreview")}:</span>
                      {isAgentActive && (
                        <span className="flex items-center space-x-1 text-cyan-400 text-[10px]">
                          <Zap className="w-3 h-3" />
                          <span>{t("card.agentReady")}</span>
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
                              {matchedAgent ? translateName(matchedAgent.name) : cmdName}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  {/* Card Footer Actions */}
                  <div className="p-3 border-t border-slate-800/60 bg-slate-900/40 relative">
                    <div className="flex items-center space-x-1">
                      <button
                        onClick={() => handleOpenSession(session.name)}
                        className="flex-1 flex items-center justify-center space-x-2 py-2 px-3 rounded-xl bg-slate-800 hover:bg-gradient-to-r hover:from-cyan-600 hover:to-blue-600 text-slate-200 hover:text-white text-sm font-medium transition shadow-sm group-hover:bg-cyan-600/20 group-hover:text-cyan-300 group-hover:border group-hover:border-cyan-500/40 cursor-pointer"
                        title={t("btn.openWith", { terminal: translateName(currentTerminalObj?.name || "terminal") })}
                      >
                        <Play className="w-3.5 h-3.5 fill-current" />
                        <span>{t("btn.open")} ({translateName(currentTerminalObj?.name || "terminal")})</span>
                      </button>

                      {env && env.terminals.length > 1 && (
                        <button
                          onClick={() =>
                            setActiveTerminalDropdown(
                              activeTerminalDropdown === session.name ? null : session.name
                            )
                          }
                          className="p-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white transition cursor-pointer"
                          title={t("card.selectTerminal")}
                        >
                          <ChevronDown className="w-4 h-4" />
                        </button>
                      )}
                    </div>

                    {activeTerminalDropdown === session.name && (
                      <div className="absolute right-3 bottom-14 z-20 w-44 rounded-xl bg-slate-900 border border-slate-700 shadow-xl py-1">
                        <div className="px-3 py-1 text-[10px] font-semibold text-slate-400 border-b border-slate-800">
                          {t("card.selectTerminal")}
                        </div>
                        {env?.terminals.map((term) => (
                          <button
                            key={term.id}
                            onClick={() => handleOpenSession(session.name, term.id)}
                            className="w-full text-left px-3 py-1.5 text-xs text-slate-200 hover:bg-cyan-950 hover:text-cyan-300 flex items-center justify-between cursor-pointer"
                          >
                            <span>{translateName(term.name)}</span>
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

      {/* New Workspace Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/80 backdrop-blur-sm p-4">
          <div className="w-full max-w-lg rounded-2xl bg-slate-900 border border-slate-800 shadow-2xl p-6">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2">
                <div className="p-2 rounded-lg bg-cyan-950 border border-cyan-800 text-cyan-400">
                  <Bot className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-slate-100">{t("modal.createTitle")}</h3>
              </div>
              <button
                onClick={() => setShowCreateModal(false)}
                className="text-slate-400 hover:text-white text-lg font-bold cursor-pointer"
              >
                ✕
              </button>
            </div>

            <div className="space-y-4">
              {/* Workspace Name Input */}
              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1">
                  {t("modal.sessionNameLabel")}
                </label>
                <input
                  type="text"
                  placeholder={t("modal.sessionNamePlaceholder")}
                  value={newSessionName}
                  maxLength={60}
                  onChange={(e) => setNewSessionName(e.target.value)}
                  className="w-full px-3 py-2 text-sm bg-slate-950 border border-slate-800 rounded-xl text-slate-100 focus:outline-none focus:border-cyan-500"
                />
                {newSessionName && sanitizeNameFrontend(newSessionName) !== newSessionName && (
                  <p className="text-[10px] text-amber-400 mt-1">
                    {t("modal.sessionNameHint", { name: sanitizeNameFrontend(newSessionName) })}
                  </p>
                )}
              </div>

              {/* Working Directory & System File Picker */}
              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1">
                  {t("modal.workingDirLabel")}
                </label>
                <div className="flex items-center space-x-2">
                  <div className="relative flex-1">
                    <Folder className="w-4 h-4 absolute left-3 top-2.5 text-slate-500" />
                    <input
                      type="text"
                      placeholder={t("modal.workingDirPlaceholder")}
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
                    <span>{t("btn.browse")}</span>
                  </button>
                </div>

                {config && config.recent_dirs && config.recent_dirs.length > 0 && (
                  <div className="flex items-center space-x-1.5 mt-2 flex-wrap gap-y-1">
                    <span className="text-[10px] text-slate-500">{t("modal.recentDirs")}</span>
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

              {/* Agent Selection Segmented Chips */}
              {env && env.agents.length > 1 && (
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <label className="block text-xs font-medium text-slate-400">
                      {t("modal.agentLabel")}
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
                          <span>{translateName(agent.name)}</span>
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
                        {t("modal.customAgentChip")}
                      </button>
                    )}
                  </div>

                  {/* Inline Custom Agent Editor */}
                  {showCustomAgentForm && (
                    <div className="mt-3 p-3 rounded-xl bg-slate-950 border border-cyan-900/60 space-y-3">
                      <div className="text-xs font-semibold text-cyan-400 flex items-center justify-between">
                        <span>{t("modal.customAgentTitle")}</span>
                        <button
                          onClick={() => setShowCustomAgentForm(false)}
                          className="text-slate-500 hover:text-slate-300 text-xs cursor-pointer"
                        >
                          {t("btn.collapse")}
                        </button>
                      </div>
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                        <div>
                          <label className="block text-[10px] text-slate-400 mb-1">{t("modal.customAgentNameLabel")}</label>
                          <input
                            type="text"
                            placeholder={t("modal.customAgentNamePlaceholder")}
                            value={customAgentName}
                            onChange={(e) => setCustomAgentName(e.target.value)}
                            className="w-full px-2.5 py-1.5 text-xs bg-slate-900 border border-slate-800 rounded-lg text-slate-200 focus:outline-none focus:border-cyan-500"
                          />
                        </div>
                        <div>
                          <label className="block text-[10px] text-slate-400 mb-1">{t("modal.customAgentCmdLabel")}</label>
                          <input
                            type="text"
                            placeholder={t("modal.customAgentCmdPlaceholder")}
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
                          {t("btn.saveAndApply")}
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {/* Pane Count Segmented Chips */}
              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1.5">
                  {t("modal.panesLabel")}
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
                        {tPlural("modal.panesCount", p)}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Terminal Selection Segmented Chips */}
              {env && env.terminals.length > 1 && (
                <div>
                  <label className="block text-xs font-medium text-slate-400 mb-1.5">
                    {t("modal.terminalLabel")}
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
                          {translateName(term.name)}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Dynamic Summary */}
              <div className="p-3 rounded-xl bg-slate-950/60 border border-slate-800/60 text-xs text-slate-400">
                {t("modal.summary", {
                  panes: selectedPanes,
                  panesText: tPlural("modal.panesCount", selectedPanes),
                  agent: translateName(currentAgentObj?.name || selectedAgent),
                  terminal: translateName(currentTerminalObj?.name || selectedTerminal),
                })}
              </div>
            </div>

            <div className="flex items-center justify-end space-x-3 mt-6">
              <button
                onClick={() => setShowCreateModal(false)}
                className="px-4 py-2 rounded-xl text-sm font-medium text-slate-400 hover:text-slate-200 cursor-pointer"
              >
                {t("btn.cancel")}
              </button>
              <button
                onClick={handleCreate}
                disabled={loading}
                className="flex items-center space-x-2 px-5 py-2 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium text-sm transition shadow-lg shadow-cyan-500/20 disabled:opacity-50 cursor-pointer"
              >
                <Plus className="w-4 h-4" />
                <span>{loading ? t("btn.creating") : t("btn.create")}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
