import { Plus, Zap } from "lucide-react";
import { t, translateName } from "../i18n";
import { Environment, TmuxSession } from "../types";

interface SessionCardProps {
  session: TmuxSession;
  env: Environment | null;
  selectedTerminal: string;
  renamingSession: string | null;
  renamedName: string;
  terminalIconUrls: Record<string, string>;
  onRenameStart: (name: string) => void;
  onRenameChange: (val: string) => void;
  onRenameCommit: (oldName: string) => void;
  onKill: (name: string) => void;
  onAddPane: (name: string) => void;
  onKillPane: (id: string) => void;
  onOpenSession: (name: string, termId: string) => void;
}

export function getSessionActivityInfo(session: TmuxSession) {
  if (session.attached) {
    return {
      statusClass:
        "bg-emerald-400 shadow-sm shadow-emerald-400/80 animate-pulse",
      statusTooltip: t("card.attached"),
    };
  }

  const now = Math.floor(Date.now() / 1000);
  const elapsed =
    session.last_active_ts > 0
      ? Math.max(0, now - session.last_active_ts)
      : -1;

  if (elapsed >= 0 && elapsed < 600) {
    return {
      statusClass: "bg-amber-400 shadow-sm shadow-amber-400/80",
      statusTooltip: t("card.bgActive"),
    };
  }

  return {
    statusClass: "bg-slate-600",
    statusTooltip: t("card.idle"),
  };
}

export function SessionCard({
  session,
  env,
  selectedTerminal,
  renamingSession,
  renamedName,
  terminalIconUrls,
  onRenameStart,
  onRenameChange,
  onRenameCommit,
  onKill,
  onAddPane,
  onKillPane,
  onOpenSession,
}: SessionCardProps) {
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
    <div className="flex flex-col justify-between rounded-2xl bg-white/10 backdrop-blur-xl border border-white/15 hover:border-cyan-500/50 transition-all duration-300 shadow-lg shadow-black/5 hover:shadow-xl hover:bg-white/15 group animate-fade-in-up">
      {/* Card Header */}
      <div className="p-4 border-b border-white/10">
        <div className="flex items-center justify-between">
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
                onChange={(e) => onRenameChange(e.target.value)}
                onBlur={() => onRenameCommit(session.name)}
                onKeyDown={(e) =>
                  e.key === "Enter" && onRenameCommit(session.name)
                }
                autoFocus
                className="bg-slate-950 px-2 py-0.5 border border-cyan-500 rounded text-sm text-white w-full"
              />
            ) : (
              <h2
                onClick={() => onRenameStart(session.name)}
                className="font-semibold text-slate-100 truncate text-base hover:text-cyan-300 hover:underline transition cursor-pointer"
                title={t("card.rename")}
              >
                {session.name}
              </h2>
            )}
          </div>
          <button
            onClick={() => onKill(session.name)}
            className="p-1 rounded text-slate-400 hover:text-rose-400 hover:bg-white/10 transition cursor-pointer text-sm font-bold leading-none shrink-0"
            title={t("card.destroy")}
          >
            ✕
          </button>
        </div>
      </div>

      {/* Pane Layout Preview */}
      <div className="p-4 flex-1">
        <div className="flex items-center justify-between mb-2">
          <button
            onClick={() => onAddPane(session.name)}
            className="px-2 py-0.5 rounded-lg bg-white/5 hover:bg-white/15 border border-white/10 text-[10px] text-slate-300 hover:text-cyan-300 transition-all duration-200 cursor-pointer flex items-center space-x-1"
            title={t("card.addPane")}
          >
            <Plus className="w-3 h-3 text-cyan-400" />
            <span>{t("card.addPane")}</span>
          </button>
          {isAgentActive && (
            <span className="flex items-center space-x-1 text-cyan-400 text-[10px]">
              <Zap className="w-3 h-3" />
              <span>{t("card.agentReady")}</span>
            </span>
          )}
        </div>
        <div
          className={`grid ${gridCols} gap-2 p-2 rounded-xl bg-slate-950/80 border border-slate-800/80`}
        >
          {session.panes.map((pane, idx) => {
            const cmdName = pane.command || "shell";
            const matchedAgent = env?.agents.find(
              (a) =>
                a.id !== "shell" &&
                (cmdName.includes(a.id) || (a.path && cmdName.includes(a.path)))
            );
            const isAgent = Boolean(matchedAgent);
            const hasContent = Boolean(
              pane.content && pane.content.trim().length > 0
            );

            return (
              <div
                key={pane.id || idx}
                className={`relative group/pane flex flex-col justify-between p-2 rounded-lg border text-[11px] min-h-[4.5rem] transition ${
                  hasContent
                    ? "bg-slate-950/90 border-slate-700/80 text-slate-200"
                    : isAgent
                    ? "bg-cyan-950/30 border-cyan-800/40 text-cyan-300"
                    : "bg-slate-900/60 border-slate-800 text-slate-400"
                }`}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="font-mono text-[9px] text-slate-500">
                    #{idx + 1}{" "}
                    {matchedAgent ? `· ${translateName(matchedAgent.name)}` : ""}
                  </span>
                  <div className="flex items-center space-x-1">
                    {session.panes_count > 1 && (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onKillPane(pane.id);
                        }}
                        className="p-0.5 rounded text-slate-400 hover:text-rose-400 hover:bg-black/40 opacity-0 group-hover/pane:opacity-100 transition-all duration-200 cursor-pointer text-[10px] leading-none"
                        title={t("card.killPane")}
                      >
                        ✕
                      </button>
                    )}
                    {isAgent && (
                      <Zap className="w-2.5 h-2.5 text-cyan-400 shrink-0" />
                    )}
                  </div>
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
            const iconSrc =
              terminalIconUrls[term.id] || `/terminal-icons/${term.id}.svg`;
            return (
              <button
                key={term.id}
                onClick={() => onOpenSession(session.name, term.id)}
                className={`p-1.5 rounded-xl transition-all duration-200 hover:scale-110 cursor-pointer relative ${
                  isDefault
                    ? "bg-cyan-500/20 border border-cyan-400/60 shadow-sm shadow-cyan-500/20"
                    : "bg-white/5 border border-white/10 hover:bg-white/15"
                }`}
                title={t("card.openWithTerminal", {
                  name: translateName(term.name),
                })}
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
}
