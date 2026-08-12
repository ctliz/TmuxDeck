import { useState } from "react";
import { agentDisplayName, t, tPlural } from "../i18n";
import { ToolInfo, TmuxSession } from "../types";
import { dominantAgentId, resolvePaneAgentId } from "../utils";

interface Props {
  sessions: TmuxSession[];
  agents: ToolInfo[];
  busy: string | null;
  onOpen: (name: string) => void;
  /** The tray stays a one-pane quick action; count is fixed at 1. */
  onAddPane: (name: string, agentId: string, count: number) => void;
}

function agentLabel(session: TmuxSession, agents: ToolInfo[]): string | null {
  // 复用主界面同一套解析：启动时记录的 agent_id 优先，其次才按命令匹配。
  const ids = session.panes
    .map((pane) => resolvePaneAgentId(pane, agents))
    .filter((id): id is string => Boolean(id));
  const dominant = dominantAgentId(ids);
  if (!dominant) return null;
  const matched = agents.find((a) => a.id === dominant);
  return matched ? agentDisplayName(matched) : dominant;
}

export function SessionList({ sessions, agents, busy, onOpen, onAddPane }: Props) {
  const [agentPicker, setAgentPicker] = useState<string | null>(null);

  return (
    <section className="min-h-0 flex-1 px-3 pt-3">
      <h2 className="mb-2 px-1 text-[11px] font-semibold uppercase tracking-wider text-white/60">
        {t("panel.workspaces")}
      </h2>

      {sessions.length === 0 ? (
        <div className="rounded-xl border border-dashed border-white/10 px-3 py-6 text-center text-[11px] text-white/45">
          {t("panel.noWorkspaces")}
        </div>
      ) : (
        <ul className="space-y-1.5">
          {sessions.map((session) => {
            const agent = agentLabel(session, agents);
            const runningAgentIds = session.panes
              .map((pane) => resolvePaneAgentId(pane, agents))
              .filter((id): id is string => Boolean(id));
            const recommendedAgentId = dominantAgentId(runningAgentIds);
            const isBusy = busy === session.name;
            const pickerOpen = agentPicker === session.name;
            return (
              <li
                key={session.id || session.name}
                className="group rounded-xl border border-white/10 bg-white/[0.05] px-3 py-2 transition-colors hover:bg-white/[0.1]"
              >
                <div className="flex items-center gap-2">
                  <span
                    className={`size-1.5 shrink-0 rounded-full ${
                      session.attached
                        ? "bg-cyan-400 shadow-[0_0_6px] shadow-cyan-400/60"
                        : "bg-white/25"
                    }`}
                    title={session.attached ? t("panel.running") : t("panel.idle")}
                  />
                  <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-white/95">
                    {session.name}
                  </span>
                  <span className="shrink-0 text-[10px] tabular-nums text-white/45">
                    {tPlural("panel.panes", session.panes_count)}
                  </span>
                </div>

                <div className="mt-1 flex items-center gap-2 pl-3.5">
                  <span className="min-w-0 flex-1 truncate text-[10px] text-white/45">
                    {agent ?? t("agent.shell")}
                  </span>
                  <button
                    type="button"
                    disabled={isBusy || agents.length === 0}
                    onClick={() => setAgentPicker(pickerOpen ? null : session.name)}
                    className="shrink-0 rounded-md px-1.5 py-0.5 text-[10px] text-white/60 opacity-0 transition hover:bg-white/10 hover:text-white/85 focus:opacity-100 group-hover:opacity-100 disabled:opacity-40"
                  >
                    {t("panel.addPane")}
                  </button>
                  <button
                    type="button"
                    disabled={isBusy}
                    onClick={() => onOpen(session.name)}
                    className="shrink-0 rounded-md bg-gradient-to-r from-cyan-500 to-blue-600 px-2 py-0.5 text-[10px] font-medium text-white transition hover:from-cyan-400 hover:to-blue-500 disabled:opacity-40"
                  >
                    {t("btn.open")}
                  </button>
                </div>

                {pickerOpen && (
                  <div className="mt-2 grid grid-cols-2 gap-1 border-t border-white/10 pt-2">
                    {agents.map((candidate) => (
                      <button
                        key={candidate.id}
                        type="button"
                        disabled={isBusy}
                        onClick={() => {
                          setAgentPicker(null);
                          onAddPane(session.name, candidate.id, 1);
                        }}
                        title={t("card.addPaneWith_one", {
                          agent: agentDisplayName(candidate),
                        })}
                        className="flex min-w-0 items-center justify-between gap-1 rounded-md bg-white/[0.06] px-2 py-1 text-left text-[10px] text-white/70 transition hover:bg-white/[0.12] hover:text-white disabled:opacity-40"
                      >
                        <span className="truncate">{agentDisplayName(candidate)}</span>
                        {candidate.id === recommendedAgentId && (
                          <span className="shrink-0 text-[8px] text-cyan-300">
                            {t("card.addPaneRecommended")}
                          </span>
                        )}
                      </button>
                    ))}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
