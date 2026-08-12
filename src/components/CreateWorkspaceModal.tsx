import { useState } from "react";
import { Bot, ChevronDown, Folder, Plus, Settings, TriangleAlert } from "lucide-react";
import { agentDisplayName, t, tPlural, translateName } from "../i18n";
import { sanitizeNameFrontend, summarizePaneAgents } from "../utils";
import {
  ClaudeMode,
  Config,
  Environment,
  ManagedClaudeStatus,
  claudeHint,
  claudeSwitchTarget,
} from "../types";

interface CreateWorkspaceModalProps {
  show: boolean;
  onClose: () => void;
  newSessionName: string;
  setNewSessionName: (name: string) => void;
  workingDir: string;
  setWorkingDir: (dir: string) => void;
  selectedAgent: string;
  setSelectedAgent: (agent: string) => void;
  selectedPanes: number;
  setSelectedPanes: (panes: number) => void;
  /** Always exactly `selectedPanes` long — the caller normalizes it. */
  paneAgentIds: string[];
  setPaneAgentIds: (ids: string[]) => void;
  selectedTerminal: string;
  setSelectedTerminal: (term: string) => void;
  showCustomAgentForm: boolean;
  setShowCustomAgentForm: (show: boolean) => void;
  customAgentName: string;
  setCustomAgentName: (name: string) => void;
  customAgentCmd: string;
  setCustomAgentCmd: (cmd: string) => void;
  env: Environment | null;
  config: Config | null;
  managedClaude: ManagedClaudeStatus | null;
  managedClaudeBusy: boolean;
  loading: boolean;
  onPickDirectory: () => void;
  onSaveCustomAgent: () => void;
  /** "managed" installs, repairs or re-selects the enhanced link; "standard" opts out. */
  onClaudeAction: (mode: ClaudeMode) => void;
  onCreate: () => void;
}

export function CreateWorkspaceModal({
  show,
  onClose,
  newSessionName,
  setNewSessionName,
  workingDir,
  setWorkingDir,
  selectedAgent,
  setSelectedAgent,
  selectedPanes,
  setSelectedPanes,
  paneAgentIds,
  setPaneAgentIds,
  selectedTerminal,
  setSelectedTerminal,
  showCustomAgentForm,
  setShowCustomAgentForm,
  customAgentName,
  setCustomAgentName,
  customAgentCmd,
  setCustomAgentCmd,
  env,
  config,
  managedClaude,
  managedClaudeBusy,
  loading,
  onPickDirectory,
  onSaveCustomAgent,
  onClaudeAction,
  onCreate,
}: CreateWorkspaceModalProps) {
  const [showClaudeMenu, setShowClaudeMenu] = useState(false);

  if (!show) return null;

  // Exactly one of these is ever non-null, so Claude never shouts twice.
  const hint = claudeHint(managedClaude);
  const switchTarget = claudeSwitchTarget(managedClaude);
  const claudeModeLabel = managedClaude?.usingStandard
    ? t("claude.modeStandard")
    : t("claude.modeManaged");
  const switchLabel =
    switchTarget === "standard"
      ? t("claude.useStandard")
      : managedClaude?.state === "healthy"
        ? t("claude.useManaged")
        : managedClaude?.state === "needs-repair"
          ? t("claude.repair")
          : t("claude.enable");

  const runClaudeAction = (mode: ClaudeMode) => {
    setShowClaudeMenu(false);
    onClaudeAction(mode);
  };

  const currentTerminalObj =
    env?.terminals.find((term) => term.id === selectedTerminal) ||
    env?.terminals[0];
  const currentAgentObj =
    env?.agents.find((agent) => agent.id === selectedAgent) || env?.agents[0];

  const agentNameFor = (agentId: string) => {
    const matched = env?.agents.find((agent) => agent.id === agentId);
    return matched ? agentDisplayName(matched) : agentId;
  };

  const paneAgentSummary = summarizePaneAgents(paneAgentIds);
  const agentMixText = paneAgentSummary.groups
    .map((group) =>
      t("modal.agentMixItem", {
        agent: agentNameFor(group.agentId),
        n: group.count,
      })
    )
    .join(t("modal.agentMixSeparator"));

  const setPaneAgentAt = (index: number, agentId: string) => {
    setPaneAgentIds(
      paneAgentIds.map((current, idx) => (idx === index ? agentId : current))
    );
  };

  const showPerPaneAgents = Boolean(env && env.agents.length > 1);
  const alreadyUniform =
    paneAgentSummary.uniform && paneAgentSummary.agentId === selectedAgent;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-md p-4 animate-fade-in-up">
      <div className="w-full max-w-lg rounded-3xl bg-slate-900/90 backdrop-blur-2xl border border-white/20 shadow-2xl p-6">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center space-x-2">
            <div className="p-2 rounded-lg bg-cyan-950 border border-cyan-800 text-cyan-400">
              <Bot className="w-5 h-5" />
            </div>
            <h3 className="text-lg font-bold text-slate-100">
              {t("modal.createTitle")}
            </h3>
          </div>
          <button
            onClick={onClose}
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
            {newSessionName &&
              sanitizeNameFrontend(newSessionName) !== newSessionName && (
                <p className="text-[10px] text-amber-400 mt-1">
                  {t("modal.sessionNameHint", {
                    name: sanitizeNameFrontend(newSessionName),
                  })}
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
                onClick={onPickDirectory}
                className="px-3 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium transition flex items-center space-x-1 shrink-0 cursor-pointer"
              >
                <Folder className="w-3.5 h-3.5 text-cyan-400" />
                <span>{t("btn.browse")}</span>
              </button>
            </div>

            {config && config.recent_dirs && config.recent_dirs.length > 0 && (
              <div className="flex items-center space-x-1.5 mt-2 flex-wrap gap-y-1">
                <span className="text-[10px] text-slate-500">
                  {t("modal.recentDirs")}
                </span>
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
                  // The mode switch lives inside the chip: no extra row while healthy.
                  const hasClaudeMenu =
                    agent.id === "claude" && isSelected && switchTarget !== null;
                  return (
                    <div
                      key={agent.id}
                      className="relative"
                      onKeyDown={(e) => {
                        if (e.key === "Escape" && showClaudeMenu) {
                          e.stopPropagation();
                          setShowClaudeMenu(false);
                        }
                      }}
                    >
                      <button
                        type="button"
                        title={
                          agent.id === "claude" && managedClaude?.state !== "unavailable"
                            ? claudeModeLabel
                            : undefined
                        }
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
                        <span>{agentDisplayName(agent)}</span>
                        {agent.id === "custom" && (
                          <Settings
                            className="w-3 h-3 ml-1 opacity-75 hover:opacity-100"
                            onClick={(e) => {
                              e.stopPropagation();
                              setShowCustomAgentForm(true);
                            }}
                          />
                        )}
                        {hasClaudeMenu && (
                          <ChevronDown
                            aria-label={t("claude.menuLabel")}
                            className="w-3 h-3 ml-0.5 opacity-70 hover:opacity-100"
                            onClick={(e) => {
                              e.stopPropagation();
                              setShowClaudeMenu(!showClaudeMenu);
                            }}
                          />
                        )}
                      </button>

                      {hasClaudeMenu && showClaudeMenu && (
                        <>
                          <div
                            className="fixed inset-0 z-20"
                            onClick={(e) => {
                              e.stopPropagation();
                              setShowClaudeMenu(false);
                            }}
                          />
                          <div
                            role="menu"
                            className="absolute z-30 top-full left-0 mt-1 min-w-[11rem] py-1 rounded-xl bg-slate-900/90 backdrop-blur-xl border border-white/15 shadow-xl shadow-black/40"
                          >
                            <div className="px-2.5 py-1 text-[9px] uppercase tracking-wide text-slate-500">
                              {t("claude.modeCurrent", { mode: claudeModeLabel })}
                            </div>
                            <button
                              type="button"
                              role="menuitem"
                              disabled={managedClaudeBusy}
                              onClick={(e) => {
                                e.stopPropagation();
                                runClaudeAction(switchTarget);
                              }}
                              className="w-full text-left px-2.5 py-1 text-[10px] text-slate-300 hover:bg-white/10 transition cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                              {managedClaudeBusy ? t("claude.working") : switchLabel}
                            </button>
                          </div>
                        </>
                      )}
                    </div>
                  );
                })}

                {!env.agents.some((a) => a.id === "custom") && (
                  <button
                    type="button"
                    onClick={() =>
                      setShowCustomAgentForm(!showCustomAgentForm)
                    }
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

              {/* One compact line, only when Claude needs a decision. */}
              {selectedAgent === "claude" && hint && (
                <div className="flex items-center gap-2 mt-2 text-[11px]">
                  {hint === "repair" && (
                    <TriangleAlert className="w-3.5 h-3.5 shrink-0 text-amber-400" />
                  )}
                  <span className={hint === "repair" ? "text-amber-400" : "text-slate-400"}>
                    {hint === "repair" ? t("claude.hintRepair") : t("claude.hintInstall")}
                  </span>
                  <button
                    type="button"
                    disabled={managedClaudeBusy}
                    onClick={() => runClaudeAction("managed")}
                    className="px-2 py-0.5 rounded-lg bg-cyan-600 hover:bg-cyan-500 text-white font-medium transition shrink-0 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    {managedClaudeBusy
                      ? t("claude.working")
                      : hint === "repair"
                        ? t("claude.repair")
                        : t("claude.enable")}
                  </button>
                  {managedClaude?.standardClaudeAvailable && (
                    <button
                      type="button"
                      disabled={managedClaudeBusy}
                      onClick={() => runClaudeAction("standard")}
                      className="text-slate-500 hover:text-slate-300 transition shrink-0 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      {t("claude.useStandard")}
                    </button>
                  )}
                </div>
              )}

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
                      <label className="block text-[10px] text-slate-400 mb-1">
                        {t("modal.customAgentNameLabel")}
                      </label>
                      <input
                        type="text"
                        placeholder={t("modal.customAgentNamePlaceholder")}
                        value={customAgentName}
                        onChange={(e) => setCustomAgentName(e.target.value)}
                        className="w-full px-2.5 py-1.5 text-xs bg-slate-900 border border-slate-800 rounded-lg text-slate-200 focus:outline-none focus:border-cyan-500"
                      />
                    </div>
                    <div>
                      <label className="block text-[10px] text-slate-400 mb-1">
                        {t("modal.customAgentCmdLabel")}
                      </label>
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
                      onClick={onSaveCustomAgent}
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

          {/* Per-pane Agent Assignment */}
          {showPerPaneAgents && (
            <div>
              <div className="flex items-center justify-between mb-1.5">
                <label className="block text-xs font-medium text-slate-400">
                  {t("modal.perPaneAgentLabel")}
                </label>
                <button
                  type="button"
                  onClick={() =>
                    setPaneAgentIds(paneAgentIds.map(() => selectedAgent))
                  }
                  disabled={alreadyUniform}
                  title={t("modal.applyToAllTitle", {
                    agent: agentNameFor(selectedAgent),
                  })}
                  className="px-2 py-0.5 rounded-lg border border-slate-700 text-[10px] text-slate-300 hover:text-cyan-300 hover:border-cyan-600 transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:text-slate-300 disabled:hover:border-slate-700"
                >
                  {t("modal.applyToAll")}
                </button>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 max-h-40 overflow-y-auto pr-0.5">
                {paneAgentIds.map((agentId, idx) => (
                  <div
                    key={idx}
                    className="flex items-center space-x-2 px-2 py-1.5 rounded-xl bg-slate-950 border border-slate-800"
                  >
                    <span className="text-[10px] text-slate-500 font-mono shrink-0">
                      {t("modal.paneIndexLabel", { n: idx + 1 })}
                    </span>
                    <select
                      value={agentId}
                      aria-label={t("modal.paneIndexLabel", { n: idx + 1 })}
                      onChange={(e) => setPaneAgentAt(idx, e.target.value)}
                      className="flex-1 min-w-0 bg-transparent text-xs text-slate-200 focus:outline-none cursor-pointer"
                    >
                      {env?.agents.map((agent) => (
                        <option
                          key={agent.id}
                          value={agent.id}
                          className="bg-slate-900"
                        >
                          {agentDisplayName(agent)}
                        </option>
                      ))}
                      {!env?.agents.some((agent) => agent.id === agentId) && (
                        <option value={agentId} className="bg-slate-900">
                          {agentId}
                        </option>
                      )}
                    </select>
                  </div>
                ))}
              </div>
            </div>
          )}

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
            {paneAgentSummary.uniform
              ? t("modal.summary", {
                  panesText: tPlural("modal.panesCount", selectedPanes),
                  agent: agentNameFor(
                    paneAgentSummary.agentId ?? currentAgentObj?.id ?? selectedAgent
                  ),
                  terminal: translateName(
                    currentTerminalObj?.name || selectedTerminal
                  ),
                })
              : t("modal.summaryMixed", {
                  panesText: tPlural("modal.panesCount", selectedPanes),
                  mix: agentMixText,
                  terminal: translateName(
                    currentTerminalObj?.name || selectedTerminal
                  ),
                })}
          </div>
        </div>

        <div className="flex items-center justify-end space-x-3 mt-6">
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-xl text-sm font-medium text-slate-400 hover:text-slate-200 cursor-pointer"
          >
            {t("btn.cancel")}
          </button>
          <button
            onClick={onCreate}
            disabled={loading}
            className="flex items-center space-x-2 px-5 py-2 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium text-sm transition shadow-lg shadow-cyan-500/20 disabled:opacity-50 cursor-pointer"
          >
            <Plus className="w-4 h-4" />
            <span>{loading ? t("btn.creating") : t("btn.create")}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
