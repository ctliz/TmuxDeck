import type { TmuxPane, TmuxSession, ToolInfo } from "./types";

export function sanitizeNameFrontend(name: string): string {
  return name
    .trim()
    .replace(/[^A-Za-z0-9_-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function reorderIds(order: string[], sourceId: string, targetId: string): string[] {
  const next = [...order];
  const fromIndex = next.indexOf(sourceId);
  const toIndex = next.indexOf(targetId);
  if (fromIndex === -1 || toIndex === -1 || fromIndex === toIndex) return next;
  next.splice(fromIndex, 1);
  next.splice(toIndex, 0, sourceId);
  return next;
}

/**
 * Carries already-captured pane output onto a freshly polled session list, so a
 * refresh never blanks the previews before the next capture round arrives.
 */
export function preservePaneContent(
  previous: TmuxSession[],
  incoming: TmuxSession[]
): TmuxSession[] {
  return incoming.map((session) => {
    const old = previous.find(
      (prev) => prev.id === session.id || prev.name === session.name
    );
    if (!old) return session;
    return {
      ...session,
      panes: session.panes.map((pane) => {
        const oldPane = old.panes.find((prev) => prev.id === pane.id);
        return oldPane?.content ? { ...pane, content: oldPane.content } : pane;
      }),
    };
  });
}

/**
 * Applies a whole capture round in one pass. Sessions and panes that did not
 * change keep their identity, and an entirely unchanged round returns the input
 * array itself so React can skip the re-render.
 */
export function applyPaneContents(
  sessions: TmuxSession[],
  contents: Map<string, string>
): TmuxSession[] {
  if (contents.size === 0) return sessions;
  let changed = false;
  const next = sessions.map((session) => {
    let paneChanged = false;
    const panes = session.panes.map((pane) => {
      const content = contents.get(pane.id);
      if (content === undefined || content === pane.content) return pane;
      paneChanged = true;
      return { ...pane, content };
    });
    if (!paneChanged) return session;
    changed = true;
    return { ...session, panes };
  });
  return changed ? next : sessions;
}

export interface PaneAgentGroup {
  agentId: string;
  count: number;
}

export interface PaneAgentSummary {
  uniform: boolean;
  agentId: string | null;
  groups: PaneAgentGroup[];
}

/**
 * Keeps the per-pane Agent list in sync with the pane count. Shrinking truncates;
 * growing repeats the current agent when every pane already shares one, otherwise
 * the new panes fall back to the workspace default.
 */
export function resizePaneAgents(
  current: string[],
  count: number,
  fallback: string
): string[] {
  const size = Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0;
  const next = current.slice(0, size);
  const uniform =
    current.length > 0 && current.every((id) => id === current[0])
      ? current[0]
      : null;
  while (next.length < size) next.push(uniform ?? fallback);
  return next;
}

export function summarizePaneAgents(agentIds: string[]): PaneAgentSummary {
  const groups: PaneAgentGroup[] = [];
  for (const agentId of agentIds) {
    const existing = groups.find((group) => group.agentId === agentId);
    if (existing) existing.count += 1;
    else groups.push({ agentId, count: 1 });
  }
  return {
    uniform: groups.length <= 1,
    agentId: groups.length === 1 ? groups[0].agentId : null,
    groups,
  };
}

/** Most frequent agent id, ties broken by first appearance. */
export function dominantAgentId(agentIds: string[]): string | null {
  const { groups } = summarizePaneAgents(agentIds);
  if (groups.length === 0) return null;
  return groups.reduce((best, group) =>
    group.count > best.count ? group : best
  ).agentId;
}

/** Resolves which Agent a running pane command belongs to, or null for a plain shell. */
export function matchAgentIdForCommand(
  command: string | undefined,
  agents: ToolInfo[]
): string | null {
  const cmd = command || "";
  if (!cmd) return null;
  const matched = agents.find(
    (agent) =>
      agent.id !== "shell" &&
      (cmd.includes(agent.id) || (agent.path && cmd.includes(agent.path)))
  );
  return matched ? matched.id : null;
}

/**
 * Agent running in a pane. The id recorded at launch wins: a pane's live command
 * can be anything (a version string, a wrapper) and would not match by name.
 * Only panes with no recorded id fall back to matching the command.
 */
export function resolvePaneAgentId(
  pane: Pick<TmuxPane, "agent_id" | "command">,
  agents: ToolInfo[]
): string | null {
  const declared = pane.agent_id?.trim();
  if (declared) return declared === "shell" ? null : declared;
  return matchAgentIdForCommand(pane.command, agents);
}
