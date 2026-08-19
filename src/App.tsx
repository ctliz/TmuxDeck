import { useState, useEffect, useRef, lazy, Suspense } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { t, tPlural, translateError } from "./i18n";
import {
  applyPaneContents,
  preservePaneContent,
  reorderIds,
  resizePaneAgents,
  sanitizeNameFrontend,
} from "./utils";
import {
  ClaudeMode,
  Config,
  CreateOpts,
  CustomAgent,
  Environment,
  ManagedClaudeStatus,
  TmuxSession,
  WorkspaceInstallPlan,
} from "./types";
import { TmuxMissingScreen } from "./components/TmuxMissingScreen";
import { SearchHeader } from "./components/SearchHeader";
import { CardGrid } from "./components/CardGrid";
import { CreateWorkspaceModal } from "./components/CreateWorkspaceModal";
import { AdapterConsentModal } from "./components/AdapterConsentModal";
import { MobilePairingModal } from "./components/MobilePairingModal";

const AgentTerminal = lazy(() =>
  import("./components/AgentTerminalCanvas").then((m) => ({ default: m.AgentTerminalCanvas }))
);

export default function App() {
  const [sessions, setSessions] = useState<TmuxSession[]>([]);
  const [activeTerminal, setActiveTerminal] = useState<{ session: TmuxSession; paneId: string } | null>(null);
  const [showMobilePairingModal, setShowMobilePairingModal] = useState(false);
  const sessionsRef = useRef<TmuxSession[]>([]);
  const failedPaneCountsRef = useRef(new Map<string, number>());
  const sessionPollPromiseRef = useRef<Promise<void> | null>(null);
  const captureInFlightRef = useRef(false);

  // Card reordering & Drag state
  const [, setCardOrder] = useState<string[]>([]);
  const cardOrderRef = useRef<string[]>([]);

  const [env, setEnv] = useState<Environment | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [managedClaude, setManagedClaude] = useState<ManagedClaudeStatus | null>(null);
  const [managedClaudeBusy, setManagedClaudeBusy] = useState(false);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");
  const [copiedBrew, setCopiedBrew] = useState(false);

  // Modal & Form State
  const [showCreateModal, setShowCreateModal] = useState(false);
  const showCreateModalRef = useRef(false);
  const [newSessionName, setNewSessionName] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [selectedAgent, setSelectedAgent] = useState("pi");
  const [selectedPanes, setSelectedPanes] = useState(4);
  // Per-pane overrides; normalized against selectedPanes/selectedAgent on every render.
  const [paneAgentIds, setPaneAgentIds] = useState<string[]>([]);
  const [selectedTerminal, setSelectedTerminal] = useState("ghostty");
  const [showCustomAgentForm, setShowCustomAgentForm] = useState(false);
  const [customAgentName, setCustomAgentName] = useState("");
  const [customAgentCmd, setCustomAgentCmd] = useState("");
  const [adapterPlan, setAdapterPlan] = useState<WorkspaceInstallPlan | null>(null);
  const [showAdapterConsent, setShowAdapterConsent] = useState(false);
  const [adapterBusy, setAdapterBusy] = useState(false);
  const [highlightedSessionId, setHighlightedSessionId] = useState<string | null>(null);
  const highlightTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const effectivePaneAgentIds = resizePaneAgents(
    paneAgentIds,
    selectedPanes,
    selectedAgent
  );

  // Rename State
  const [renamingSession, setRenamingSession] = useState<string | null>(null);
  const [renamedName, setRenamedName] = useState("");

  useEffect(() => {
    showCreateModalRef.current = showCreateModal;
  }, [showCreateModal]);

  // Keep activeTerminal.session synchronized with polled session state
  useEffect(() => {
    if (!activeTerminal) return;
    const updated = sessions.find(
      (s) => s.id === activeTerminal.session.id || s.name === activeTerminal.session.name
    );
    if (updated && updated !== activeTerminal.session) {
      setActiveTerminal((prev) => (prev ? { ...prev, session: updated } : null));
    }
  }, [sessions, activeTerminal]);

  const sortSessionsByOrder = (list: TmuxSession[], order: string[]) => {
    const orderMap = new Map(order.map((id, idx) => [id, idx]));
    return [...list].sort((a, b) => {
      const idxA = orderMap.has(a.id) ? orderMap.get(a.id)! : Number.MAX_SAFE_INTEGER;
      const idxB = orderMap.has(b.id) ? orderMap.get(b.id)! : Number.MAX_SAFE_INTEGER;
      return idxA - idxB;
    });
  };

  /**
   * Environment and config are effectively static: detecting terminals/agents
   * shells out to the filesystem, so it runs on mount and after the few actions
   * that can change it — never on the session poll.
   */
  const loadStaticData = async () => {
    try {
      const [envData, cfgData, managedClaudeData] = await Promise.all([
        invoke<Environment>("detect_environment"),
        invoke<Config>("load_config"),
        invoke<ManagedClaudeStatus>("get_managed_claude_status"),
      ]);
      setEnv(envData);
      setConfig(cfgData);
      setManagedClaude(managedClaudeData);

      if (cfgData.custom_agent) {
        setCustomAgentName(cfgData.custom_agent.name || "");
        setCustomAgentCmd(cfgData.custom_agent.command || "");
      }
      if (!showCreateModalRef.current) {
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
      }
    } catch (err: any) {
      setErrorMsg(translateError(err) || t("val.dataRefreshFailed"));
    }
  };

  const refreshSessions = async (forceAfterCurrent = false): Promise<void> => {
    // Timed polls share one request. User actions wait for any old poll and then
    // fetch fresh state so a newly-created workspace never waits for the next tick.
    const currentPoll = sessionPollPromiseRef.current;
    if (currentPoll) {
      await currentPoll;
      if (!forceAfterCurrent) return;
    }
    if (sessionPollPromiseRef.current) {
      return refreshSessions(forceAfterCurrent);
    }

    const poll = (async () => {
      try {
        const sessionList = await invoke<TmuxSession[]>("get_tmux_sessions");
        setErrorMsg("");
        setSessions((prevSessions) => {
          const mergedList = preservePaneContent(prevSessions, sessionList);

          const activeIds = new Set(mergedList.map((s) => s.id));
          const updatedOrder = cardOrderRef.current.filter((id) => activeIds.has(id));
          mergedList.forEach((s) => {
            if (!updatedOrder.includes(s.id)) updatedOrder.push(s.id);
          });
          cardOrderRef.current = updatedOrder;
          setCardOrder(updatedOrder);

          return sortSessionsByOrder(mergedList, updatedOrder);
        });

        sessionsRef.current = sessionList;
        failedPaneCountsRef.current.clear();
      } catch (err: any) {
        setErrorMsg(translateError(err) || t("val.dataRefreshFailed"));
      }
    })();

    sessionPollPromiseRef.current = poll;
    try {
      await poll;
    } finally {
      if (sessionPollPromiseRef.current === poll) {
        sessionPollPromiseRef.current = null;
      }
    }
  };

  const loadData = async () => {
    await Promise.all([loadStaticData(), refreshSessions()]);
  };

  useEffect(() => {
    loadData();
    const unlistenPromise = listen("trigger-new-workspace", () => {
      setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
      setPaneAgentIds([]);
      setShowCreateModal(true);
    });
    const unlistenFocus = listen<string>("focus-conversation", (event) => {
      const paneId = event.payload;
      const match = sessionsRef.current.find((session) =>
        session.panes.some((pane) => pane.id === paneId)
      );
      if (match) {
        setSearch("");
        const nextOrder = [
          match.id,
          ...cardOrderRef.current.filter((id) => id !== match.id),
        ];
        cardOrderRef.current = nextOrder;
        setCardOrder(nextOrder);
        setSessions((prev) => sortSessionsByOrder(prev, nextOrder));
        setHighlightedSessionId(match.id);
        if (highlightTimerRef.current) clearTimeout(highlightTimerRef.current);
        highlightTimerRef.current = setTimeout(() => {
          setHighlightedSessionId(null);
          highlightTimerRef.current = null;
        }, 4000);
      }
    });

    const sessionTimer = setInterval(() => {
      if (document.visibilityState !== "visible") return;
      refreshSessions();
    }, 4000);

    const captureTimer = setInterval(async () => {
      if (document.visibilityState !== "visible") return;
      if (captureInFlightRef.current) return;
      const current = sessionsRef.current;
      if (current.length === 0) return;

      captureInFlightRef.current = true;
      const failedPaneCounts = failedPaneCountsRef.current;
      try {
        const paneIds = current.flatMap((sess) =>
          sess.panes
            .filter((pane) => (failedPaneCounts.get(pane.id) || 0) < 3)
            .map((pane) => pane.id)
        );

        const captured = await Promise.all(
          paneIds.map(async (paneId) => {
            try {
              const content = await invoke<string>("capture_pane", { paneId, maxLines: 5 });
              failedPaneCounts.set(paneId, 0);
              return [paneId, content] as const;
            } catch {
              failedPaneCounts.set(paneId, (failedPaneCounts.get(paneId) || 0) + 1);
              return null;
            }
          })
        );

        // One state write per round instead of one per pane.
        const contents = new Map(
          captured.filter((entry): entry is readonly [string, string] => entry !== null)
        );
        if (contents.size > 0) {
          setSessions((prev) => applyPaneContents(prev, contents));
        }
      } finally {
        captureInFlightRef.current = false;
      }
    }, 8000);

    // Polling is paused while hidden, so catch up as soon as the window returns.
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") refreshSessions();
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
      unlistenFocus.then((unlisten) => unlisten());
      if (highlightTimerRef.current) clearTimeout(highlightTimerRef.current);
      clearInterval(sessionTimer);
      clearInterval(captureTimer);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
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

  /**
   * One entry point for every Claude messaging choice. Picking "managed" when
   * the adapter is missing or broken installs/repairs it first, so the UI only
   * ever has to offer a single button.
   */
  const handleClaudeAction = async (mode: ClaudeMode) => {
    setManagedClaudeBusy(true);
    try {
      if (mode === "standard") {
        await invoke("use_standard_claude");
      } else if (managedClaude?.state === "healthy") {
        await invoke("use_managed_claude");
      } else {
        await invoke<ManagedClaudeStatus>("install_managed_claude");
      }
      await loadStaticData();
      setSelectedAgent("claude");
    } catch (err: any) {
      alert(translateError(err));
    } finally {
      setManagedClaudeBusy(false);
    }
  };

  const handleSaveCustomAgent = async () => {
    if (!customAgentCmd.trim()) return alert(t("val.enterCustomCmd"));
    const newCustom: CustomAgent = { name: customAgentName.trim() || t("agent.custom"), command: customAgentCmd.trim() };
    try {
      const currentConfig = config || { default_terminal: selectedTerminal, default_agent: "custom", default_panes: selectedPanes, recent_dirs: [], use_standard_claude: false, panel_bypass_permissions: true, desktop_notifications: true };
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

  const handlePanelBypassChange = async (enabled: boolean) => {
    const currentConfig = config || {
      default_terminal: selectedTerminal,
      default_agent: "custom",
      default_panes: selectedPanes,
      recent_dirs: [],
      use_standard_claude: false,
      panel_bypass_permissions: true,
      desktop_notifications: true,
    };
    const updatedConfig = { ...currentConfig, panel_bypass_permissions: enabled };
    try {
      await invoke("save_config", { config: updatedConfig });
      setConfig(updatedConfig);
    } catch (err: any) {
      alert(t("val.saveConfigFailed") + ": " + translateError(err));
    }
  };

  const handleDesktopNotificationsChange = async (enabled: boolean) => {
    const currentConfig = config || {
      default_terminal: selectedTerminal,
      default_agent: "custom",
      default_panes: selectedPanes,
      recent_dirs: [],
      use_standard_claude: false,
      panel_bypass_permissions: true,
      desktop_notifications: true,
    };
    const updatedConfig = { ...currentConfig, desktop_notifications: enabled };
    try {
      await invoke("save_config", { config: updatedConfig });
      setConfig(updatedConfig);
    } catch (err: any) {
      alert(t("val.saveConfigFailed") + ": " + translateError(err));
    }
  };

  const handleOpenSession = async (sessionName: string, termId?: string) => {
    const isNative = sessionsRef.current.some(
      (session) => session.name === sessionName && session.native_split
    );
    const targetTerminal = isNative
      ? "ghostty"
      : termId || selectedTerminal || (env?.terminals[0]?.id || "terminal");
    try {
      await invoke("open_session", { name: sessionName, terminalId: targetTerminal });
    } catch (err: any) {
      alert(t("val.openTerminalFailed") + ": " + translateError(err));
    }
  };

  const createWorkspace = async () => {
    const cleanName = sanitizeNameFrontend(newSessionName);
    if (!cleanName) throw new Error(t("val.enterName"));
    const opts: CreateOpts = {
      name: cleanName,
      dir: workingDir.trim() || null,
      agent_id: selectedAgent,
      pane_agent_ids: effectivePaneAgentIds,
      panes: selectedPanes,
      terminal_id: selectedTerminal,
      headless: true,
    };
    await invoke("create_session", { opts });
    setShowCreateModal(false);
    setShowAdapterConsent(false);
    setAdapterPlan(null);
    setNewSessionName("");
    setWorkingDir("");
    setPaneAgentIds([]);
    // Creation only changes sessions and recent defaults. Avoid a full agent/
    // terminal rescan, which is intentionally much more expensive.
    const [cfgData] = await Promise.all([
      invoke<Config>("load_config"),
      refreshSessions(true),
    ]);
    setConfig(cfgData);
  };

  const handleCreate = async () => {
    const cleanName = sanitizeNameFrontend(newSessionName);
    if (!cleanName) return alert(t("val.enterName"));
    setLoading(true);
    try {
      const plan = await invoke<WorkspaceInstallPlan>("check_workspace_adapters", {
        paneAgentIds: effectivePaneAgentIds,
      });
      if (plan.requiresConsent) {
        setAdapterPlan(plan);
        setShowAdapterConsent(true);
        return;
      }
      await createWorkspace();
    } catch (err: any) {
      alert(t("val.createFailed") + ": " + translateError(err));
    } finally {
      setLoading(false);
    }
  };

  const handleInstallAndCreate = async () => {
    if (!adapterPlan) return;
    setAdapterBusy(true);
    try {
      await invoke("apply_workspace_install_plan", {
        planId: adapterPlan.planId,
        planFingerprint: adapterPlan.planFingerprint,
      });
      await createWorkspace();
    } catch (err: any) {
      alert(t("val.createFailed") + ": " + translateError(err));
    } finally {
      setAdapterBusy(false);
    }
  };

  const handleRecheckAdapters = async () => {
    setAdapterBusy(true);
    try {
      const plan = await invoke<WorkspaceInstallPlan>("check_workspace_adapters", {
        paneAgentIds: effectivePaneAgentIds,
      });
      if (!plan.requiresConsent) {
        setShowAdapterConsent(false);
        setAdapterPlan(null);
        return;
      }
      setAdapterPlan(plan);
      setShowAdapterConsent(true);
    } catch (err: any) {
      alert(t("val.createFailed") + ": " + translateError(err));
    } finally {
      setAdapterBusy(false);
    }
  };

  const handleCreateWithoutInstalling = async () => {
    setAdapterBusy(true);
    try {
      await createWorkspace();
    } catch (err: any) {
      alert(t("val.createFailed") + ": " + translateError(err));
    } finally {
      setAdapterBusy(false);
    }
  };

  const handleKill = async (sessionName: string, paneCount: number) => {
    const isNative = sessionsRef.current.some(
      (session) => session.name === sessionName && session.native_split
    );
    const confirmKey = isNative ? "confirm.destroyNative" : "confirm.destroy";
    if (!confirm(tPlural(confirmKey, paneCount, { name: sessionName }))) return;
    try {
      await invoke("kill_session", { sessionName });
      await refreshSessions(true);
    } catch (err: any) {
      alert(t("val.destroyFailed") + ": " + translateError(err));
    }
  };

  /** One batched call: the backend adds every pane and re-lays out natives once. */
  const handleAddPane = async (
    sessionName: string,
    agentId: string,
    count: number
  ) => {
    try {
      const currentTargetSession = sessionsRef.current.find(
        (s) => s.name === sessionName || s.id === sessionName
      );
      const existingPaneIds = new Set(currentTargetSession?.panes.map((p) => p.id) ?? []);

      await invoke("add_panes", { sessionName, agentId, count });
      await refreshSessions(true);

      const updatedSession = sessionsRef.current.find(
        (s) => s.name === sessionName || s.id === sessionName
      );
      if (updatedSession && updatedSession.panes.length > 0) {
        // Automatically switch and focus to the newly added tab
        const newPane =
          updatedSession.panes.find((p) => !existingPaneIds.has(p.id)) ||
          updatedSession.panes[updatedSession.panes.length - 1];
        if (newPane) {
          setActiveTerminal({
            session: updatedSession,
            paneId: newPane.id,
          });
        }
      }
    } catch (err: any) {
      alert(t("val.createFailed") + ": " + translateError(err));
    }
  };

  const handleKillPane = async (paneId: string, sessionTarget?: string) => {
    const nativeWorkspace = sessionTarget
      ? sessionsRef.current.find((session) =>
          session.native_split && session.panes.some((pane) => pane.session_target === sessionTarget)
        )
      : undefined;
    const confirmKey = nativeWorkspace
      ? nativeWorkspace.panes_count === 1
        ? "card.confirmKillLastSlot"
        : "card.confirmKillSlot"
      : "card.confirmKillPane";
    if (!confirm(t(confirmKey))) return;
    try {
      if (sessionTarget) {
        await invoke("kill_slot", { sessionTarget });
      } else {
        await invoke("kill_pane", { paneId });
      }
      await refreshSessions(true);
    } catch (err: any) {
      alert(translateError(err));
    }
  };

  const handleSwapPane = async (
    paneIdA: string,
    paneIdB: string,
    sessionTargetA?: string,
    sessionTargetB?: string
  ) => {
    try {
      if (sessionTargetA && sessionTargetB) {
        await invoke("swap_native_slots", {
          sessionTargetA,
          sessionTargetB,
        });
      } else {
        await invoke("swap_pane", { paneIdA, paneIdB });
      }
      await refreshSessions(true);
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
      await refreshSessions(true);
    } catch (err: any) {
      alert(t("val.renameFailed") + ": " + translateError(err));
    }
  };

  const handleReorderCards = (sourceSessionId: string, targetSessionId: string) => {
    const currentOrder = reorderIds(cardOrderRef.current, sourceSessionId, targetSessionId);
    cardOrderRef.current = currentOrder;
    setCardOrder(currentOrder);
    setSessions((prev) => sortSessionsByOrder(prev, currentOrder));
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
    <div className="td-canvas flex flex-col h-screen text-slate-100 font-sans select-none overflow-hidden">
      {activeTerminal ? (
        <Suspense
          fallback={
            <div className="flex-1 flex items-center justify-center bg-slate-950 text-slate-500 text-xs font-mono">
              Loading Agent Terminal…
            </div>
          }
        >
          <AgentTerminal
            session={activeTerminal.session}
            activePaneId={activeTerminal.paneId}
            onSelectPane={(paneId) =>
              setActiveTerminal((prev) => (prev ? { ...prev, paneId } : null))
            }
            onBack={() => setActiveTerminal(null)}
            env={env}
            selectedTerminal={selectedTerminal}
            onOpenExternalTerminal={handleOpenSession}
            onAddPane={handleAddPane}
          />
        </Suspense>
      ) : (
        <>
          <SearchHeader
            search={search}
            onSearchChange={setSearch}
            totalSessions={sessions.length}
            runningSessions={sessions.filter((s) => s.attached).length}
            onOpenMobilePairing={() => setShowMobilePairingModal(true)}
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
              onNewWorkspaceClick={() => {
                setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
                setPaneAgentIds([]);
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
              onOpenChat={(session, paneId) => setActiveTerminal({ session, paneId })}
              onSwapPane={handleSwapPane}
              onReorderCards={handleReorderCards}
              highlightedSessionId={highlightedSessionId}
            />

            {filteredSessions.length === 0 && search && (
              <div className="text-center text-xs text-slate-400 mt-4 font-mono animate-fade-in-up">
                {t("empty.title")}
              </div>
            )}
          </main>
        </>
      )}

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
        paneAgentIds={effectivePaneAgentIds}
        setPaneAgentIds={setPaneAgentIds}
        showCustomAgentForm={showCustomAgentForm}
        setShowCustomAgentForm={setShowCustomAgentForm}
        customAgentName={customAgentName}
        setCustomAgentName={setCustomAgentName}
        customAgentCmd={customAgentCmd}
        setCustomAgentCmd={setCustomAgentCmd}
        env={env}
        config={config}
        managedClaude={managedClaude}
        managedClaudeBusy={managedClaudeBusy}
        loading={loading}
        onPickDirectory={handlePickDirectory}
        onSaveCustomAgent={handleSaveCustomAgent}
        onClaudeAction={handleClaudeAction}
        onPanelBypassChange={handlePanelBypassChange}
        onDesktopNotificationsChange={handleDesktopNotificationsChange}
        onCreate={handleCreate}
      />

      <AdapterConsentModal
        show={showAdapterConsent}
        plan={adapterPlan}
        loading={adapterBusy}
        onClose={() => {
          if (!adapterBusy) {
            setShowAdapterConsent(false);
            setAdapterPlan(null);
          }
        }}
        onInstallAndCreate={handleInstallAndCreate}
        onCreateWithoutInstalling={handleCreateWithoutInstalling}
        onRecheck={handleRecheckAdapters}
      />

      <MobilePairingModal
        show={showMobilePairingModal}
        onClose={() => setShowMobilePairingModal(false)}
      />
    </div>
  );
}
