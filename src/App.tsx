import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Terminal,
  Plus,
  Folder,
  Search,
  Zap,
  Bot,
  Copy,
  Check,
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
  content?: string;
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

  // Refs shared between loadData (4s refresh) and the capture timer (8s).
  // sessionsRef avoids stale closures in setInterval; failedPaneCountsRef is
  // reset on every successful session refresh so a pane that briefly failed
  // 3 times recovers instead of being disabled forever.
  const sessionsRef = useRef<TmuxSession[]>([]);
  const failedPaneCountsRef = useRef(new Map<string, number>());
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

  // Terminal Icons Cache State
  const [terminalIconUrls, setTerminalIconUrls] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!env || !env.terminals) return;
    env.terminals.forEach((term) => {
      if (terminalIconUrls[term.id]) return;
      invoke<number[]>("get_terminal_icon", { terminalId: term.id })
        .then((bytes) => {
          if (!bytes || bytes.length === 0) return;
          const binary = String.fromCharCode(...new Uint8Array(bytes));
          const base64 = btoa(binary);
          const dataUrl = `data:image/png;base64,${base64}`;
          setTerminalIconUrls((prev) => ({ ...prev, [term.id]: dataUrl }));
        })
        .catch(() => {
          // Fallback to SVG icon in public/terminal-icons/
        });
    });
  }, [env]);

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
      setSessions((prevSessions) => {
        return sessionList.map((newSess) => {
          const oldSess = prevSessions.find((s) => s.id === newSess.id || s.name === newSess.name);
          if (!oldSess) return newSess;
          return {
            ...newSess,
            panes: newSess.panes.map((newPane) => {
              const oldPane = oldSess.panes.find((p) => p.id === newPane.id);
              return oldPane?.content ? { ...newPane, content: oldPane.content } : newPane;
            }),
          };
        });
      });

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

      // Successful refresh: keep the ref snapshot fresh and reset the
      // per-pane circuit breaker so recovered panes resume capture.
      sessionsRef.current = sessionList;
      failedPaneCountsRef.current.clear();
    } catch (err: any) {
      setErrorMsg(translateError(err) || t("val.dataRefreshFailed"));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();

    const unlistenPromise = listen("trigger-new-workspace", () => {
      setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
      setShowCreateModal(true);
    });

    const sessionTimer = setInterval(() => {
      loadData();
    }, 4000);

    const captureTimer = setInterval(() => {
      if (document.visibilityState !== "visible") {
        return; // Pause capture when tab or app window is hidden/minimized
      }

      const current = sessionsRef.current;
      if (current.length === 0) return;
      const failedPaneCounts = failedPaneCountsRef.current;

      for (const sess of current) {
        for (const pane of sess.panes) {
          const failCount = failedPaneCounts.get(pane.id) || 0;
          if (failCount >= 3) continue;

          invoke<string>("capture_pane", {
            paneId: pane.id,
            maxLines: 5,
          })
            .then((content) => {
              failedPaneCounts.set(pane.id, 0);
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sess.id
                    ? {
                        ...s,
                        panes: s.panes.map((p) =>
                          p.id === pane.id ? { ...p, content } : p
                        ),
                      }
                    : s
                )
              );
            })
            .catch(() => {
              failedPaneCounts.set(pane.id, failCount + 1);
            });
        }
      }
    }, 8000);

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
      clearInterval(sessionTimer);
      clearInterval(captureTimer);
    };
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
    <div className="flex flex-col h-screen bg-gradient-to-br from-slate-900 via-slate-950 to-indigo-950/60 text-slate-100 font-sans select-none overflow-hidden">
      {/* Floating Centered Liquid Glass Search Pill */}
      <div className="flex items-center justify-center pt-6 pb-2 px-6 shrink-0">
        <div className="relative group w-full max-w-xs transition-all duration-300 focus-within:max-w-sm">
          <Search className="w-4 h-4 absolute left-3.5 top-2.5 text-white/40 group-focus-within:text-cyan-400 transition" />
          <input
            type="text"
            placeholder={t("search.placeholder")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full pl-9 pr-4 py-1.5 text-xs bg-white/10 backdrop-blur-xl border border-white/15 rounded-full text-slate-100 placeholder-white/40 focus:outline-none focus:border-cyan-500/60 focus:bg-white/15 focus:shadow-lg focus:shadow-cyan-500/10 transition-all duration-300"
            title={t("search.hint", { total: sessions.length, running: sessions.filter((s) => s.attached).length })}
          />
        </div>
      </div>

      {/* Main Workspace Grid */}
      <main className="flex-1 overflow-y-auto p-6">
        {errorMsg && (
          <div className="mb-6 p-4 rounded-xl bg-rose-950/40 border border-rose-800/60 text-rose-300 text-sm">
            {errorMsg}
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-5">
          {/* Card #1: New Workspace Dashed Glass Card */}
          <div
            onClick={() => {
              setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
              setShowCreateModal(true);
            }}
            className="flex flex-col items-center justify-center min-h-[14rem] rounded-2xl border-2 border-dashed border-white/20 bg-white/5 backdrop-blur-xl hover:bg-white/10 hover:border-cyan-400/50 transition-all duration-300 cursor-pointer group shadow-lg shadow-black/5 animate-fade-in-up"
          >
            <div className="p-3 rounded-2xl bg-white/10 border border-white/15 group-hover:scale-110 group-hover:bg-cyan-500/20 group-hover:border-cyan-400/40 transition-all duration-300 mb-3">
              <Plus className="w-6 h-6 text-white/70 group-hover:text-cyan-300 transition" />
            </div>
            <span className="text-sm font-semibold text-slate-200 group-hover:text-cyan-300 transition">
              {t("btn.newWorkspace")}
            </span>
            <span className="text-[11px] text-slate-400/80 mt-1 px-4 text-center">
              {t("empty.hint")}
            </span>
          </div>

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
                className="flex flex-col justify-between rounded-2xl bg-white/10 backdrop-blur-xl border border-white/15 hover:border-cyan-500/50 transition-all duration-300 shadow-lg shadow-black/5 hover:shadow-xl hover:bg-white/15 group animate-fade-in-up"
              >
                  {/* Card Header */}
                  <div className="p-4 border-b border-white/10">
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
                          <h2
                            onClick={() => {
                              setRenamingSession(session.name);
                              setRenamedName(session.name);
                            }}
                            className="font-semibold text-slate-100 truncate text-base hover:text-cyan-300 hover:underline transition cursor-pointer"
                            title={t("card.rename")}
                          >
                            {session.name}
                          </h2>
                        )}
                      </div>
                      <button
                        onClick={() => handleKill(session.name)}
                        className="p-1 rounded text-slate-400 hover:text-rose-400 hover:bg-white/10 transition cursor-pointer text-sm font-bold leading-none shrink-0"
                        title={t("card.destroy")}
                      >
                        ✕
                      </button>
                    </div>

                    <div className="flex items-center justify-between text-xs text-slate-400">
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
                        const hasContent = Boolean(pane.content && pane.content.trim().length > 0);

                        return (
                          <div
                            key={pane.id || idx}
                            className={`flex flex-col justify-between p-2 rounded-lg border text-[11px] min-h-[4.5rem] transition ${
                              hasContent
                                ? "bg-slate-950/90 border-slate-700/80 text-slate-200"
                                : isAgent
                                ? "bg-cyan-950/30 border-cyan-800/40 text-cyan-300"
                                : "bg-slate-900/60 border-slate-800 text-slate-400"
                            }`}
                          >
                            <div className="flex items-center justify-between mb-1">
                              <span className="font-mono text-[9px] text-slate-500">
                                #{idx + 1} {matchedAgent ? `· ${translateName(matchedAgent.name)}` : ""}
                              </span>
                              {isAgent && <Zap className="w-2.5 h-2.5 text-cyan-400 shrink-0" />}
                            </div>
                            {hasContent ? (
                              <pre className="font-mono text-[9px] text-slate-300 leading-tight whitespace-pre-wrap break-all overflow-hidden line-clamp-4 select-text">
                                {pane.content}
                              </pre>
                            ) : (
                              <span className="font-mono truncate font-medium">
                                {matchedAgent ? translateName(matchedAgent.name) : cmdName}
                              </span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  {/* Card Footer Actions: Row of Terminal Icons */}
                  <div className="p-3 border-t border-white/10 bg-black/20 flex items-center justify-between">
                    <span className="text-[10px] font-medium text-slate-400">
                      {t("card.selectTerminal")}
                    </span>
                    <div className="flex items-center space-x-2">
                      {env?.terminals.map((term) => {
                        const isDefault = selectedTerminal === term.id;
                        const iconSrc = terminalIconUrls[term.id] || `/terminal-icons/${term.id}.svg`;
                        return (
                          <button
                            key={term.id}
                            onClick={() => handleOpenSession(session.name, term.id)}
                            className={`p-1.5 rounded-xl transition-all duration-200 hover:scale-110 cursor-pointer relative ${
                              isDefault
                                ? "bg-cyan-500/20 border border-cyan-400/60 shadow-sm shadow-cyan-500/20"
                                : "bg-white/5 border border-white/10 hover:bg-white/15"
                            }`}
                            title={t("card.openWithTerminal", { name: translateName(term.name) })}
                          >
                            <img
                              src={iconSrc}
                              onError={(e) => {
                                e.currentTarget.src = "/terminal-icons/default.svg";
                              }}
                              alt={term.name}
                              className="w-5 h-5 rounded object-contain"
                            />
                            {isDefault && (
                              <span className="w-1.5 h-1.5 rounded-full bg-cyan-400 absolute -top-0.5 -right-0.5 shadow-sm shadow-cyan-400" />
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
          {filteredSessions.length === 0 && search && (
            <div className="text-center text-xs text-slate-400 mt-4 font-mono animate-fade-in-up">
              {t("empty.title")}
            </div>
          )}
      </main>

      {/* New Workspace Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-md p-4 animate-fade-in-up">
          <div className="w-full max-w-lg rounded-3xl bg-slate-900/90 backdrop-blur-2xl border border-white/20 shadow-2xl p-6">
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
