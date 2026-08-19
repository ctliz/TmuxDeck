import { useState } from "react";
import { Environment, TmuxSession } from "../types";
import { NewWorkspaceCard } from "./NewWorkspaceCard";
import { SessionCard } from "./SessionCard";

interface CardGridProps {
  sessions: TmuxSession[];
  env: Environment | null;
  selectedTerminal: string;
  renamingSession: string | null;
  renamedName: string;
  onNewWorkspaceClick: () => void;
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
  onReorderCards: (sourceSessionId: string, targetSessionId: string) => void;
  highlightedSessionId?: string | null;
}

export function CardGrid({
  sessions,
  env,
  selectedTerminal,
  renamingSession,
  renamedName,
  onNewWorkspaceClick,
  onRenameStart,
  onRenameChange,
  onRenameCommit,
  onKill,
  onAddPane,
  onKillPane,
  onOpenSession,
  onOpenChat,
  onSwapPane,
  onReorderCards,
  highlightedSessionId,
}: CardGridProps) {
  const [draggingCardId, setDraggingCardId] = useState<string | null>(null);
  const [cardDragOverId, setCardDragOverId] = useState<string | null>(null);

  const handleCardDragStart = (e: React.DragEvent, sessionId: string) => {
    e.dataTransfer.setData("application/x-tmuxdeck-card", sessionId);
    e.dataTransfer.setData("text/plain", sessionId);
    e.dataTransfer.effectAllowed = "move";
    setDraggingCardId(sessionId);
  };

  const handleCardDragOver = (e: React.DragEvent, sessionId: string) => {
    const types = Array.from(e.dataTransfer.types || []);
    const isCardDrag =
      draggingCardId !== null ||
      types.includes("application/x-tmuxdeck-card") ||
      types.includes("text/plain");

    if (isCardDrag) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (draggingCardId !== sessionId && cardDragOverId !== sessionId) {
        setCardDragOverId(sessionId);
      }
    }
  };

  const handleCardDragLeave = (_e: React.DragEvent, sessionId: string) => {
    if (cardDragOverId === sessionId) {
      setCardDragOverId(null);
    }
  };

  const handleCardDrop = (e: React.DragEvent, targetSessionId: string) => {
    e.preventDefault();
    setCardDragOverId(null);
    const sourceSessionId =
      e.dataTransfer.getData("application/x-tmuxdeck-card") ||
      e.dataTransfer.getData("text/plain") ||
      draggingCardId;
    setDraggingCardId(null);

    if (!sourceSessionId || sourceSessionId === targetSessionId) return;
    onReorderCards(sourceSessionId, targetSessionId);
  };

  const handleCardDragEnd = () => {
    setDraggingCardId(null);
    setCardDragOverId(null);
  };

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-5">
      <NewWorkspaceCard onClick={onNewWorkspaceClick} />

      {sessions.map((session) => (
        <SessionCard
          key={session.id}
          session={session}
          env={env}
          selectedTerminal={selectedTerminal}
          renamingSession={renamingSession}
          renamedName={renamedName}
          isDraggingCard={draggingCardId === session.id}
          isCardDragOverTarget={cardDragOverId === session.id}
          isHighlighted={highlightedSessionId === session.id}
          onRenameStart={onRenameStart}
          onRenameChange={onRenameChange}
          onRenameCommit={onRenameCommit}
          onKill={onKill}
          onAddPane={onAddPane}
          onKillPane={onKillPane}
          onOpenSession={onOpenSession}
          onOpenChat={onOpenChat}
          onSwapPane={onSwapPane}
          onCardDragStart={handleCardDragStart}
          onCardDragOver={handleCardDragOver}
          onCardDragLeave={handleCardDragLeave}
          onCardDrop={handleCardDrop}
          onCardDragEnd={handleCardDragEnd}
        />
      ))}
    </div>
  );
}
