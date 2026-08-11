import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { t, tPlural, translateError } from "./i18n";
import { sanitizeNameFrontend } from "./utils";
import { Config, CustomAgent, Environment, TmuxSession } from "./types";
import { TmuxMissingScreen } from "./components/TmuxMissingScreen";
import { SearchHeader } from "./components/SearchHeader";
import { CardGrid } from "./components/CardGrid";
import { CreateWorkspaceModal } from "./components/CreateWorkspaceModal";

export default function App() {
  const [sessions, setSessions] = useState<TmuxSession[]>([]);
  const sessionsRef = useRef<TmuxSession[]>([]);
  const failedPaneCountsRef = useRef(new Map<string, number>());

  // Card reordering & Drag state
  const [, setCardOrder] = useState<string[]>([]);
  const cardOrderRef = useRef<string[]>([]);

  const [env, setEnv] = useState<Environment | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");
  const [copiedBrew, setCopiedBrew] = useState(false);

  // Modal & Form State
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [newSessionName, setNewSessionName] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [selectedAgent, setSelectedAgent] = useState("pi");
  const [selectedPanes, setSelectedPanes] = useState(4);
  const [selectedTerminal, setSelectedTerminal] = useState("ghostty");
  const [showCustomAgentForm, setShowCustomAgentForm] = useState(false);
  const [customAgentName, setCustomAgentName] = useState("");
  const [customAgentCmd, setCustomAgentCmd] = useState("");

  // Rename & Icon Cache State
  const [renamingSession, setRenamingSession] = useState<string | null>(null);
  const [renamedName, setRenamedName] = useState("");
  const [terminalIconUrls, setTerminalIconUrls] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!env?.terminals) return;
    env.terminals.forEach((term) => {
      if (terminalIconUrls[term.id]) return;
      invoke<number[]>("get_terminal_icon", { terminalId: term.id })
        .then((bytes) => {
          if (!bytes?.length) return;
          const binary = String.fromCharCode(...new Uint8Array(bytes));
          const base64 = btoa(binary);
          setTerminalIconUrls((prev) => ({ ...prev, [term.id]: `data:image/png;base64,${base64}` }));
        })
        .catch(() => {});
    });
  }, [env]);

  const sortSessionsByOrder = (list: TmuxSession[], order: string[]) => {
    const orderMap = new Map(order.map((id, idx) => [id, idx]));
    return [...list].sort((a, b) => {
      const idxA = orderMap.has(a.id) ? orderMap.get(a.id)! : Number.MAX_SAFE_INTEGER;
      const idxB = orderMap.has(b.id) ? orderMap.get(b.id)! : Number.MAX_SAFE_INTEGER;
      return idxA - idxB;
    });
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
        const mergedList = sessionList.map((newSess) => {
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

        const activeIds = new Set(mergedList.map((s) => s.id));
        const updatedOrder = cardOrderRef.current.filter((id) => activeIds.has(id));
        mergedList.forEach((s) => {
          if (!updatedOrder.includes(s.id)) updatedOrder.push(s.id);
        });
        cardOrderRef.current = updatedOrder;
        setCardOrder(updatedOrder);

        return sortSessionsByOrder(mergedList, updatedOrder);
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
      if (cfgData.default_panes) setSelectedPanes(cfgData.default_panes);

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

    const sessionTimer = setInterval(loadData, 4000);
    const captureTimer = setInterval(() => {
      if (document.visibilityState !== "visible") return;
      const current = sessionsRef.current;
      if (current.length === 0) return;
      const failedPaneCounts = failedPaneCountsRef.current;

      for (const sess of current) {
        for (const pane of sess.panes) {
          const failCount = failedPaneCounts.get(pane.id) || 0;
          if (failCount >= 3) continue;

          invoke<string>("capture_pane", { paneId: pane.id, maxLines: 5 })
            .then((content) => {
              failedPaneCounts.set(pane.id, 0);
              setSessions((prev) =>
                prev.map((s) =>
                  s.id === sess.id
                    ? { ...s, panes: s.panes.map((p) => (p.id === pane.id ? { ...p, content } : p)) }
                    : s
                )
              );
            })
            .catch(() => failedPaneCounts.set(pane.id, failCount + 1));
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
      const res = await open({ directory: true, multiple: false, title: t("modal.workingDirLabel") });
      if (res && typeof res === "string") {
        setWorkingDir(await invoke<string>("to_wsl_path", { path: res }));
      }
    } catch (err) {
      console.error("Folder picker error", err);
    }
  };

  const handleSaveCustomAgent = async () => {
    if (!customAgentCmd.trim()) return alert(t("val.enterCustomCmd"));
    const newCustom: CustomAgent = { name: customAgentName.trim() || t("agent.custom"), command: customAgentCmd.trim() };
    try {
      const currentConfig = config || { default_terminal: selectedTerminal, default_agent: "custom", default_panes: selectedPanes, recent_dirs: [] };
      const updatedConfig: Config = { ...currentConfig, custom_agent: newCustom };
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
    if (!cleanName) return alert(t("val.enterName"));
    setLoading(true);
    try {
      await invoke("create_session", {
        opts: { name: cleanName, dir: workingDir.trim() || null, agent_id: selectedAgent, panes: selectedPanes, terminal_id: selectedTerminal },
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

  const handleKill = async (sessionName: string, paneCount: number) => {
    if (!confirm(tPlural("confirm.destroy", paneCount, { name: sessionName }))) return;
    try {
      await invoke("kill_session", { sessionName });
      await loadData();
    } catch (err: any) {
      alert(t("val.destroyFailed") + ": " + translateError(err));
    }
  };

  const handleAddPane = async (sessionName: string) => {
    try {
      await invoke("add_pane", { sessionName });
      await loadData();
    } catch (err: any) {
      alert(t("val.createFailed") + ": " + translateError(err));
    }
  };

  const handleKillPane = async (paneId: string) => {
    if (!confirm(t("card.confirmKillPane"))) return;
    try {
      await invoke("kill_pane", { paneId });
      await loadData();
    } catch (err: any) {
      alert(translateError(err));
    }
  };

  const handleSwapPane = async (paneIdA: string, paneIdB: string) => {
    try {
      await invoke("swap_pane", { paneIdA, paneIdB });
      await loadData();
    } catch (err: any) {
      alert(translateError(err));
    }
  };

  const handleRename = async (oldName: string) => {
    const cleanNew = sanitizeNameFrontend(renamedName);
    if (!cleanNew || cleanNew === oldName) return setRenamingSession(null);
    try {
      await invoke("rename_session", { oldName, newName: cleanNew });
      setRenamingSession(null);
      await loadData();
    } catch (err: any) {
      alert(t("val.renameFailed") + ": " + translateError(err));
    }
  };

  const handleReorderCards = (sourceSessionId: string, targetSessionId: string) => {
    const currentOrder = [...cardOrderRef.current];
    const fromIndex = currentOrder.indexOf(sourceSessionId);
    const toIndex = currentOrder.indexOf(targetSessionId);
    if (fromIndex !== -1 && toIndex !== -1) {
      currentOrder.splice(fromIndex, 1);
      currentOrder.splice(toIndex, 0, sourceSessionId);
      cardOrderRef.current = currentOrder;
      setCardOrder(currentOrder);
      setSessions((prev) => sortSessionsByOrder(prev, currentOrder));
    }
  };

  const copyCommandHelper = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedBrew(true);
    setTimeout(() => setCopiedBrew(false), 2000);
  };

  const filteredSessions = sessions.filter((s) => s.name.toLowerCase().includes(search.toLowerCase()));

  if (env && env.tmux === null) {
    return (
      <TmuxMissingScreen
        env={env}
        copiedBrew={copiedBrew}
        onCopyBrew={() => copyCommandHelper("brew install tmux")}
        onCopyWslInstall={() => copyCommandHelper("wsl --install")}
        onCopyWslApt={() => copyCommandHelper("wsl sudo apt install tmux")}
        onRecheck={loadData}
      />
    );
  }

  return (
    <div className="flex flex-col h-screen bg-gradient-to-br from-slate-900 via-slate-950 to-indigo-950/60 text-slate-100 font-sans select-none overflow-hidden">
      <SearchHeader
        search={search}
        onSearchChange={setSearch}
        totalSessions={sessions.length}
        runningSessions={sessions.filter((s) => s.attached).length}
      />

      <main className="flex-1 overflow-y-auto p-6">
        {errorMsg && (
          <div className="mb-6 p-4 rounded-xl bg-rose-950/40 border border-rose-800/60 text-rose-300 text-sm">
            {errorMsg}
          </div>
        )}

        <CardGrid
          sessions={filteredSessions}
          env={env}
          selectedTerminal={selectedTerminal}
          renamingSession={renamingSession}
          renamedName={renamedName}
          terminalIconUrls={terminalIconUrls}
          onNewWorkspaceClick={() => {
            setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
            setShowCreateModal(true);
          }}
          onRenameStart={(name) => {
            setRenamingSession(name);
            setRenamedName(name);
          }}
          onRenameChange={setRenamedName}
          onRenameCommit={handleRename}
          onKill={handleKill}
          onAddPane={handleAddPane}
          onKillPane={handleKillPane}
          onOpenSession={handleOpenSession}
          onSwapPane={handleSwapPane}
          onReorderCards={handleReorderCards}
        />

        {filteredSessions.length === 0 && search && (
          <div className="text-center text-xs text-slate-400 mt-4 font-mono animate-fade-in-up">
            {t("empty.title")}
          </div>
        )}
      </main>

      <CreateWorkspaceModal
        show={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        newSessionName={newSessionName}
        setNewSessionName={setNewSessionName}
        workingDir={workingDir}
        setWorkingDir={setWorkingDir}
        selectedAgent={selectedAgent}
        setSelectedAgent={setSelectedAgent}
        selectedPanes={selectedPanes}
        setSelectedPanes={setSelectedPanes}
        selectedTerminal={selectedTerminal}
        setSelectedTerminal={setSelectedTerminal}
        showCustomAgentForm={showCustomAgentForm}
        setShowCustomAgentForm={setShowCustomAgentForm}
        customAgentName={customAgentName}
        setCustomAgentName={setCustomAgentName}
        customAgentCmd={customAgentCmd}
        setCustomAgentCmd={setCustomAgentCmd}
        env={env}
        config={config}
        loading={loading}
        onPickDirectory={handlePickDirectory}
        onSaveCustomAgent={handleSaveCustomAgent}
        onCreate={handleCreate}
      />
    </div>
  );
}
