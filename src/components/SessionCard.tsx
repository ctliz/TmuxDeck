import { useState, useRef } from "react";
import { Plus, Zap } from "lucide-react";
import { t, translateName } from "../i18n";
import { Environment, TmuxPane, TmuxSession } from "../types";

interface SessionCardProps {
  session: TmuxSession;
  env: Environment | null;
  selectedTerminal: string;
  renamingSession: string | null;
  renamedName: string;
  terminalIconUrls: Record<string, string>;
  isDraggingCard?: boolean;
  isCardDragOverTarget?: boolean;
  onRenameStart: (name: string) => void;
  onRenameChange: (val: string) => void;
  onRenameCommit: (oldName: string) => void;
  onKill: (name: string, paneCount: number) => void;
  onAddPane: (name: string) => void;
  onKillPane: (id: string, sessionTarget?: string) => void;
  onOpenSession: (name: string, termId: string) => void;
  onSwapPane: (paneIdA: string, paneIdB: string) => void;
  onCardDragStart?: (e: React.DragEvent, sessionId: string) => void;
  onCardDragOver?: (e: React.DragEvent, sessionId: string) => void;
  onCardDragLeave?: (e: React.DragEvent, sessionId: string) => void;
  onCardDrop?: (e: React.DragEvent, sessionId: string) => void;
  onCardDragEnd?: (e: React.DragEvent) => void;
}

export function getSessionActivityInfo(session: TmuxSession) {
  if (session.attached) {
    return {
      statusClass:
        "bg-emerald-400 shadow-sm shadow-emerald-400/80 animate-pulse",
      statusTooltip: t("card.attached"),
    };
  }

  if (session.native_split) {
    return {
      statusClass: "bg-amber-400 shadow-sm shadow-amber-400/80",
      statusTooltip: t("card.runningDetached"),
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
  isDraggingCard,
  isCardDragOverTarget,
  onRenameStart,
  onRenameChange,
  onRenameCommit,
  onKill,
  onAddPane,
  onKillPane,
  onOpenSession,
  onSwapPane,
  onCardDragStart,
  onCardDragOver,
  onCardDragLeave,
  onCardDrop,
  onCardDragEnd,
}: SessionCardProps) {
  const isRenaming = renamingSession === session.name;
  const mainCmds = session.panes.map((p) => p.command).filter(Boolean);
  const isAgentActive = env?.agents.some((a) =>
    mainCmds.some((c) => c.includes(a.id) || (a.path && c.includes(a.path)))
  );
  const activityInfo = getSessionActivityInfo(session);

  // Pane drag state inside card
  const [draggingPaneId, setDraggingPaneId] = useState<string | null>(null);
  const [dragOverPaneId, setDragOverPaneId] = useState<string | null>(null);
  const isPaneDraggingRef = useRef(false);

  const panesCount = session.panes.length || session.panes_count;

  const gridCols =
    panesCount === 1
      ? "grid-cols-1"
      : panesCount === 2
      ? "grid-cols-2"
      : panesCount === 3
      ? "grid-cols-2"
      : panesCount === 6
      ? "grid-cols-3"
      : "grid-cols-2";

  // Handlers for Pane Drag & Drop (inner-card)
  const handlePaneDragStart = (e: React.DragEvent, pane: TmuxPane) => {
    if (session.native_split || session.panes_count <= 1) return;
    e.stopPropagation();
    isPaneDraggingRef.current = true;
    setDraggingPaneId(pane.id);
    e.dataTransfer.setData(
      "application/x-tmuxdeck-pane",
      JSON.stringify({ sessionId: session.id, paneId: pane.id })
    );
    e.dataTransfer.effectAllowed = "move";
  };

  const handlePaneDragOver = (e: React.DragEvent, pane: TmuxPane) => {
    if (!isPaneDraggingRef.current || session.panes_count <= 1) return;
    if (draggingPaneId && draggingPaneId !== pane.id) {
      e.stopPropagation();
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      setDragOverPaneId(pane.id);
    }
  };

  const handlePaneDragLeave = (e: React.DragEvent, pane: TmuxPane) => {
    e.stopPropagation();
    if (dragOverPaneId === pane.id) {
      setDragOverPaneId(null);
    }
  };

  const handlePaneDrop = (e: React.DragEvent, targetPane: TmuxPane) => {
    e.stopPropagation();
    e.preventDefault();
    setDragOverPaneId(null);
    setDraggingPaneId(null);
    isPaneDraggingRef.current = false;

    const rawData = e.dataTransfer.getData("application/x-tmuxdeck-pane");
    if (!rawData) return;

    try {
      const data = JSON.parse(rawData);
      if (data.sessionId === session.id && data.paneId !== targetPane.id) {
        onSwapPane(data.paneId, targetPane.id);
      }
    } catch (err) {
      console.error("Pane drop error", err);
    }
  };

  const handlePaneDragEnd = (e: React.DragEvent) => {
    e.stopPropagation();
    setDraggingPaneId(null);
    setDragOverPaneId(null);
    isPaneDraggingRef.current = false;
  };

  return (
    <div
      onDragOver={(e) => !isPaneDraggingRef.current && onCardDragOver?.(e, session.id)}
      onDragLeave={(e) => !isPaneDraggingRef.current && onCardDragLeave?.(e, session.id)}
      onDrop={(e) => !isPaneDraggingRef.current && onCardDrop?.(e, session.id)}
      className={`flex flex-col justify-between rounded-2xl bg-white/10 backdrop-blur-xl border transition-all duration-300 shadow-lg shadow-black/5 hover:shadow-xl hover:bg-white/15 group animate-fade-in-up ${
        isDraggingCard
          ? "opacity-40 border-cyan-500/70 scale-95 cursor-grabbing"
          : isCardDragOverTarget
          ? "border-cyan-400 bg-cyan-500/20 shadow-cyan-500/20 scale-[1.02]"
          : "border-white/15 hover:border-cyan-500/50"
      }`}
    >
      {/* Card Header */}
      <div className="p-4 border-b border-white/10">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2 min-w-0 flex-1">
            <button
              type="button"
              draggable={!isRenaming}
              onDragStart={(e) => onCardDragStart?.(e, session.id)}
              onDragEnd={(e) => onCardDragEnd?.(e)}
              className="shrink-0 px-1 text-slate-500 hover:text-cyan-300 cursor-grab active:cursor-grabbing"
              title={t("card.drag")}
              aria-label={t("card.drag")}
            >
              ⠿
            </button>
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
                className="bg-slate-950 px-2 py-0.5 border border-cyan-500 rounded text-sm text-white w-full cursor-text"
                onDragStart={(e) => e.stopPropagation()}
              />
            ) : (
              <h2
                onClick={
                  session.native_split
                    ? undefined
                    : () => onRenameStart(session.name)
                }
                className={`font-semibold text-slate-100 truncate text-base transition ${
                  session.native_split
                    ? "cursor-default"
                    : "hover:text-cyan-300 hover:underline cursor-pointer"
                }`}
                title={
                  session.native_split
                    ? t("card.nativeRenameUnsupported")
                    : t("card.rename")
                }
              >
                {session.name}
              </h2>
            )}
          </div>
          <button
            draggable={false}
            onPointerDown={(e) => e.stopPropagation()}
            onDragStart={(e) => e.stopPropagation()}
            onClick={() => onKill(session.name, session.panes_count)}
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
            draggable={false}
            onPointerDown={(e) => e.stopPropagation()}
            onDragStart={(e) => e.stopPropagation()}
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
          className={`grid ${gridCols} gap-2 p-2 rounded-xl bg-slate-950/80 border border-slate-800/80 h-[11.5rem] overflow-y-auto`}
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

            const isThisPaneDragging = draggingPaneId === pane.id;
            const isThisPaneDragOver = dragOverPaneId === pane.id;
            const isPaneDraggable = !session.native_split && session.panes_count > 1;

            const isPaneColSpan = panesCount === 3 && idx === 0;
            const isPaneFullHeight = panesCount <= 2;

            return (
              <div
                key={pane.id || idx}
                draggable={isPaneDraggable}
                onDragStart={(e) => handlePaneDragStart(e, pane)}
                onDragOver={(e) => handlePaneDragOver(e, pane)}
                onDragLeave={(e) => handlePaneDragLeave(e, pane)}
                onDrop={(e) => handlePaneDrop(e, pane)}
                onDragEnd={handlePaneDragEnd}
                className={`relative group/pane flex flex-col justify-between p-2 rounded-lg border text-[11px] transition-all duration-200 ${
                  isPaneColSpan ? "col-span-2" : ""
                } ${
                  isPaneFullHeight ? "h-full min-h-[9.8rem]" : "min-h-[4.8rem]"
                } ${
                  isPaneDraggable ? "cursor-grab active:cursor-grabbing" : ""
                } ${
                  isThisPaneDragging
                    ? "opacity-30 border-cyan-500 border-dashed scale-95"
                    : isThisPaneDragOver
                    ? "border-cyan-400 bg-cyan-500/30 shadow-sm shadow-cyan-500/30 scale-[1.03]"
                    : hasContent
                    ? "bg-slate-950/90 border-slate-700/80 text-slate-200"
                    : isAgent
                    ? "bg-cyan-950/30 border-cyan-800/40 text-cyan-300"
                    : "bg-slate-900/60 border-slate-800 text-slate-400"
                }`}
              >
                <div className="flex items-center justify-between mb-1 select-none pointer-events-none">
                  <span className="font-mono text-[9px] text-slate-500 flex items-center space-x-1">
                    <span>
                      {pane.slot ? `Slot ${pane.slot}` : `#${idx + 1}`}
                      {matchedAgent ? ` · ${translateName(matchedAgent.name)}` : ""}
                    </span>
                    {pane.attached !== undefined && (
                      <span
                        className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                          pane.attached ? "bg-emerald-400" : "bg-amber-400"
                        }`}
                        title={pane.attached ? t("card.attached") : t("card.runningDetached")}
                      />
                    )}
                  </span>
                  <div className="flex items-center space-x-1 pointer-events-auto">
                    {(session.native_split || session.panes_count > 1) && (
                      <button
                        draggable={false}
                        onPointerDown={(e) => e.stopPropagation()}
                        onDragStart={(e) => e.stopPropagation()}
                        onClick={(e) => {
                          e.stopPropagation();
                          onKillPane(
                            pane.id,
                            session.native_split ? pane.session_target : undefined
                          );
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
                  <pre
                    className={`font-mono text-[9px] text-slate-300 leading-tight whitespace-pre-wrap break-all overflow-hidden select-text ${
                      isPaneFullHeight ? "line-clamp-8" : "line-clamp-4"
                    }`}
                  >
                    {pane.content}
                  </pre>
                ) : (
                  <span className="font-mono truncate font-medium pointer-events-none select-none">
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
          {env?.terminals
            .filter((term) => !session.native_split || term.id === "ghostty")
            .map((term) => {
            const isDefault = selectedTerminal === term.id;
            const iconSrc =
              terminalIconUrls[term.id] || `/terminal-icons/${term.id}.svg`;
            return (
              <button
                key={term.id}
                draggable={false}
                onPointerDown={(e) => e.stopPropagation()}
                onDragStart={(e) => e.stopPropagation()}
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
                  draggable={false}
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
