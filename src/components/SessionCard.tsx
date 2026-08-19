import { useState, useRef } from "react";
import { Terminal, ExternalLink } from "lucide-react";
import { agentDisplayName, t, translateName } from "../i18n";
import { Environment, TmuxPane, TmuxSession } from "../types";
import { resolvePaneAgentId } from "../utils";

interface SessionCardProps {
  session: TmuxSession;
  env: Environment | null;
  selectedTerminal: string;
  renamingSession: string | null;
  renamedName: string;
  isDraggingCard?: boolean;
  isCardDragOverTarget?: boolean;
  isHighlighted?: boolean;
  onRenameStart: (name: string) => void;
  onRenameChange: (val: string) => void;
  onRenameCommit: (oldName: string) => void;
  onKill: (name: string, paneCount: number) => void;
  onAddPane: (name: string, agentId: string, count: number) => Promise<void>;
  onKillPane: (id: string, sessionTarget?: string) => void;
  onOpenSession: (name: string, termId: string) => void;
  onOpenChat: (session: TmuxSession, paneId: string) => void;
  onSwapPane: (
    paneIdA: string,
    paneIdB: string,
    sessionTargetA?: string,
    sessionTargetB?: string
  ) => void;
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
  isDraggingCard,
  isCardDragOverTarget,
  isHighlighted,
  onRenameStart,
  onRenameChange,
  onRenameCommit,
  onKill,
  onAddPane: _onAddPane,
  onKillPane,
  onOpenSession,
  onOpenChat,
  onSwapPane,
  onCardDragStart,
  onCardDragOver,
  onCardDragLeave,
  onCardDrop,
  onCardDragEnd,
}: SessionCardProps) {
  const isRenaming = renamingSession === session.name;
  const activityInfo = getSessionActivityInfo(session);

  const activeTermId =
    session.terminal_id ?? (session.native_split ? "ghostty" : selectedTerminal);
  const matchedTerm = env?.terminals.find((t) => t.id === activeTermId);
  const termName = matchedTerm ? translateName(matchedTerm.name) : activeTermId;

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
    if (session.panes_count <= 1) return;
    e.stopPropagation();
    isPaneDraggingRef.current = true;
    setDraggingPaneId(pane.id);
    e.dataTransfer.setData(
      "application/x-tmuxdeck-pane",
      JSON.stringify({
        sessionId: session.id,
        paneId: pane.id,
        sessionTarget: pane.session_target,
      })
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
        onSwapPane(
          data.paneId,
          targetPane.id,
          data.sessionTarget,
          targetPane.session_target
        );
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
          : isHighlighted
          ? "border-cyan-400/80 bg-cyan-500/10 shadow-cyan-500/10"
          : "border-white/15 hover:border-cyan-500/50"
      }`}
    >
      {/* Card Header */}
      <div className="p-4 border-b border-white/10">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center space-x-2 min-w-0 flex-1">
            <button
              type="button"
              draggable={!isRenaming}
              onDragStart={(e) => onCardDragStart?.(e, session.id)}
              onDragEnd={(e) => onCardDragEnd?.(e)}
              className="shrink-0 px-0.5 text-slate-500 hover:text-cyan-300 cursor-grab active:cursor-grabbing transition"
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
                onPointerDown={(e) => e.stopPropagation()}
                onDragStart={(e) => e.stopPropagation()}
              />
            ) : (
              <div className="flex items-center space-x-2 min-w-0">
                <h2
                  onClick={(e) => {
                    e.stopPropagation();
                    onRenameStart(session.name);
                  }}
                  onPointerDown={(e) => e.stopPropagation()}
                  onDragStart={(e) => e.stopPropagation()}
                  className="font-semibold text-slate-100 truncate text-base transition hover:text-cyan-300 hover:underline cursor-pointer"
                  title={t("card.rename")}
                >
                  {session.name}
                </h2>
              </div>
            )}
          </div>
          <div className="flex items-center space-x-1 shrink-0">
            {/* Primary Action: Open Agent Terminal */}
            <button
              type="button"
              draggable={false}
              onPointerDown={(e) => e.stopPropagation()}
              onDragStart={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                const targetPaneId = session.panes[0]?.id || "%0";
                onOpenChat(session, targetPaneId);
              }}
              className="p-1.5 rounded-lg bg-cyan-500/20 hover:bg-cyan-500/30 border border-cyan-400/40 text-cyan-300 hover:text-cyan-200 transition cursor-pointer flex items-center justify-center shadow-sm shadow-cyan-500/20"
              title={t("card.openTerminal")}
            >
              <Terminal className="w-3.5 h-3.5" />
            </button>
            {/* Fallback Action: Open in External Terminal */}
            <button
              type="button"
              draggable={false}
              onPointerDown={(e) => e.stopPropagation()}
              onDragStart={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                onOpenSession(session.name, activeTermId);
              }}
              className="p-1.5 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-slate-400 hover:text-slate-200 transition cursor-pointer flex items-center justify-center"
              title={`${t("card.openTerminalFallback")} (${termName})`}
            >
              <ExternalLink className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              draggable={false}
              onPointerDown={(e) => e.stopPropagation()}
              onDragStart={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                onKill(session.name, session.panes_count);
              }}
              className="p-1 rounded text-slate-400 hover:text-rose-400 hover:bg-white/10 transition cursor-pointer text-sm font-bold leading-none shrink-0"
              title={t("card.destroy")}
            >
              ✕
            </button>
          </div>
        </div>
      </div>

      {/* Pane Layout Preview */}
      <div className="p-4 flex-1">
        <div
          className={`grid ${gridCols} gap-2 p-2 rounded-xl bg-black/30 border border-white/10 h-[12.5rem] overflow-y-auto`}
        >
          {session.panes.map((pane, idx) => {
            const paneAgentId = resolvePaneAgentId(pane, env?.agents ?? []);
            const matchedAgent = paneAgentId
              ? env?.agents.find((agent) => agent.id === paneAgentId)
              : undefined;
            // An Agent uninstalled since launch still labels its pane by id.
            const paneAgentLabel = matchedAgent
              ? agentDisplayName(matchedAgent)
              : paneAgentId;
            const isAgent = Boolean(paneAgentId);
            const hasContent = Boolean(
              pane.content && pane.content.trim().length > 0
            );

            const isThisPaneDragging = draggingPaneId === pane.id;
            const isThisPaneDragOver = dragOverPaneId === pane.id;
            const isPaneDraggable = session.panes_count > 1;

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
                onClick={(e) => {
                  e.stopPropagation();
                  if (!isPaneDraggingRef.current) {
                    onOpenChat(session, pane.id);
                  }
                }}
                title={t("card.openPaneTerminal")}
                className={`relative group/pane flex flex-col justify-between p-2.5 rounded-lg border text-[11px] transition-all duration-200 cursor-pointer hover:border-cyan-400/60 hover:bg-white/5 ${
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
                    ? "bg-black/40 border-white/15 text-slate-200"
                    : isAgent
                    ? "bg-cyan-950/30 border-cyan-800/40 text-cyan-300"
                    : "bg-black/20 border-white/10 text-slate-400"
                }`}
              >
                <div className="flex items-center justify-between mb-1.5 select-none pointer-events-none">
                  <div className="flex items-center space-x-1.5 min-w-0">
                    <span className="font-mono text-[9px] text-slate-500 font-medium shrink-0">
                      {pane.slot ? `Slot ${pane.slot}` : `#${idx + 1}`}
                    </span>
                    <span className="text-[10px] text-slate-600">·</span>
                    <span className="font-semibold text-[11px] text-cyan-300 truncate">
                      {paneAgentLabel || "Agent"}
                    </span>
                  </div>
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
                  </div>
                </div>

                <div className="flex-1 overflow-hidden">
                  {hasContent ? (
                    <pre
                      className={`font-mono text-[9px] text-slate-300 leading-tight whitespace-pre-wrap break-all overflow-hidden select-text ${
                        isPaneFullHeight ? "line-clamp-8" : "line-clamp-4"
                      }`}
                    >
                      {pane.content}
                    </pre>
                  ) : null}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
