import { Bot, Folder, Plus, Settings } from "lucide-react";
import { t, tPlural, translateName } from "../i18n";
import { sanitizeNameFrontend } from "../utils";
import { Config, Environment } from "../types";

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
  loading: boolean;
  onPickDirectory: () => void;
  onSaveCustomAgent: () => void;
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
  loading,
  onPickDirectory,
  onSaveCustomAgent,
  onCreate,
}: CreateWorkspaceModalProps) {
  if (!show) return null;

  const currentTerminalObj =
    env?.terminals.find((term) => term.id === selectedTerminal) ||
    env?.terminals[0];
  const currentAgentObj =
    env?.agents.find((agent) => agent.id === selectedAgent) || env?.agents[0];

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
