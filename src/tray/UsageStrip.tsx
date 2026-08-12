import { t, tPlural } from "../i18n";
import { AgentUsage, UsageSnapshot } from "../types";
import { formatRelative, formatTokens } from "./format";

/** 固定顺序展示，避免后端返回顺序变化导致卡片跳位。 */
const AGENT_ORDER = ["codex", "claude", "pi", "opencode"];

/** 每家一个色相，只用于区分，不表达好坏。 */
const ACCENTS: Record<string, string> = {
  codex: "from-cyan-400/80 to-cyan-500/40",
  claude: "from-orange-400/80 to-orange-500/40",
  pi: "from-violet-400/80 to-violet-500/40",
  opencode: "from-emerald-400/80 to-emerald-500/40",
};

function AgentCard({ usage, max }: { usage: AgentUsage; max: number }) {
  const accent = ACCENTS[usage.agentId] ?? "from-slate-400/80 to-slate-500/40";

  if (!usage.available) {
    return (
      <div className="rounded-xl border border-white/10 bg-white/[0.03] px-3 py-2.5">
        <div className="text-[11px] font-medium text-white/60">{usage.displayName}</div>
        <div className="mt-1 text-[11px] text-white/35">{t("panel.notDetected")}</div>
      </div>
    );
  }

  // 条形只表示「这家在四家里占多少」，是相对量，不是额度。
  const ratio = max > 0 ? Math.min(1, usage.tokens30d / max) : 0;

  return (
    <div className="rounded-xl border border-white/10 bg-white/[0.06] px-3 py-2.5 transition-colors hover:bg-white/[0.1]">
      <div className="flex items-baseline justify-between gap-2">
        <span className="truncate text-[11px] font-medium text-white/85">
          {usage.displayName}
        </span>
        <span className="shrink-0 text-[10px] tabular-nums text-white/45">
          {tPlural("panel.sessions", usage.sessions30d)}
        </span>
      </div>

      <div className="mt-1.5 flex items-baseline gap-1.5">
        <span className="text-lg font-semibold leading-none tabular-nums text-white">
          {formatTokens(usage.todayTokens)}
        </span>
        <span className="text-[10px] text-white/45">{t("panel.today")}</span>
      </div>

      <div className="mt-2 h-1 overflow-hidden rounded-full bg-white/10">
        <div
          className={`h-full rounded-full bg-gradient-to-r ${accent}`}
          style={{ width: `${Math.max(ratio * 100, ratio > 0 ? 4 : 0)}%` }}
        />
      </div>

      <div className="mt-1.5 flex items-baseline justify-between">
        <span className="text-[10px] text-white/45">{t("panel.last30d")}</span>
        <span className="text-[11px] font-medium tabular-nums text-white/75">
          {formatTokens(usage.tokens30d)}
        </span>
      </div>
    </div>
  );
}

export function UsageStrip({ snapshot }: { snapshot: UsageSnapshot | null }) {
  const agents = snapshot?.agents ?? [];
  const ordered = [...agents].sort(
    (a, b) => AGENT_ORDER.indexOf(a.agentId) - AGENT_ORDER.indexOf(b.agentId)
  );
  const max = Math.max(0, ...ordered.map((a) => a.tokens30d));
  // updatedAt 为 0 = 首轮采集仍在后台跑（冷启动要扫近 1GB 日志）。
  const collecting = !snapshot || snapshot.updatedAt === 0;

  return (
    <section className="px-3 pt-3">
      <header className="mb-2 flex items-baseline justify-between px-1">
        <h2 className="text-[11px] font-semibold uppercase tracking-wider text-white/60">
          {t("panel.usageTitle")}
        </h2>
        <span className="text-[10px] text-white/45">
          {collecting
            ? t("panel.collecting")
            : t("panel.updatedAt", { time: formatRelative(snapshot.updatedAt) })}
        </span>
      </header>

      {collecting ? (
        <div className="grid grid-cols-2 gap-2">
          {AGENT_ORDER.map((id) => (
            <div
              key={id}
              className="h-[86px] animate-pulse rounded-xl border border-white/10 bg-white/[0.04]"
            />
          ))}
        </div>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-2">
            {ordered.map((usage) => (
              <AgentCard key={usage.agentId} usage={usage} max={max} />
            ))}
          </div>
          <div className="mt-2 grid grid-cols-2 divide-x divide-white/10 rounded-xl border border-white/10 bg-gradient-to-r from-cyan-500/15 to-blue-600/10">
            <div className="px-3 py-2">
              <div className="text-[10px] text-white/50">{t("panel.today")}</div>
              <div className="text-[15px] font-semibold leading-tight tabular-nums text-white">
                {formatTokens(snapshot.totalToday)}
              </div>
            </div>
            <div className="px-3 py-2">
              <div className="text-[10px] text-white/50">{t("panel.last30d")}</div>
              <div className="text-[15px] font-semibold leading-tight tabular-nums text-white">
                {formatTokens(snapshot.total30d)}
              </div>
            </div>
          </div>
        </>
      )}
    </section>
  );
}
