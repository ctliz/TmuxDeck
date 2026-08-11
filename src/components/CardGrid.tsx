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
  terminalIconUrls: Record<string, string>;
  onNewWorkspaceClick: () => void;
  onRenameStart: (name: string) => void;
  onRenameChange: (val: string) => void;
  onRenameCommit: (oldName: string) => void;
  onKill: (name: string, paneCount: number) => void;
  onAddPane: (name: string) => void;
  onKillPane: (id: string, sessionTarget?: string) => void;
  onOpenSession: (name: string, termId: string) => void;
  onSwapPane: (paneIdA: string, paneIdB: string) => void;
  onReorderCards: (sourceSessionId: string, targetSessionId: string) => void;
}

export function CardGrid({
  sessions,
  env,
  selectedTerminal,
  renamingSession,
  renamedName,
  terminalIconUrls,
  onNewWorkspaceClick,
  onRenameStart,
  onRenameChange,
  onRenameCommit,
  onKill,
  onAddPane,
  onKillPane,
  onOpenSession,
  onSwapPane,
  onReorderCards,
}: CardGridProps) {
  const [draggingCardId, setDraggingCardId] = useState<string | null>(null);
  const [cardDragOverId, setCardDragOverId] = useState<string | null>(null);

  const handleCardDragStart = (e: React.DragEvent, sessionId: string) => {
    e.dataTransfer.setData("application/x-tmuxdeck-card", sessionId);
    e.dataTransfer.effectAllowed = "move";
    setDraggingCardId(sessionId);
  };

  const handleCardDragOver = (e: React.DragEvent, sessionId: string) => {
    if (e.dataTransfer.types.includes("application/x-tmuxdeck-card")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (draggingCardId !== sessionId) {
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
    const sourceSessionId = e.dataTransfer.getData("application/x-tmuxdeck-card");
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
          terminalIconUrls={terminalIconUrls}
          isDraggingCard={draggingCardId === session.id}
          isCardDragOverTarget={cardDragOverId === session.id}
          onRenameStart={onRenameStart}
          onRenameChange={onRenameChange}
          onRenameCommit={onRenameCommit}
          onKill={onKill}
          onAddPane={onAddPane}
          onKillPane={onKillPane}
          onOpenSession={onOpenSession}
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
