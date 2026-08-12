import { t } from "../i18n";

/**
 * Token 数的紧凑写法。面板列宽很窄，19,234,567,890 这种完整数字会撑破布局，
 * 而这里的量级本来就只需要看数量级（B / M / K）。
 */
export function formatTokens(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0";
  if (n >= 1e9) return `${trim(n / 1e9)}B`;
  if (n >= 1e6) return `${trim(n / 1e6)}M`;
  if (n >= 1e3) return `${trim(n / 1e3)}K`;
  return String(Math.round(n));
}

/** 三位有效数字以内，且不留 "1.0" 这种尾巴。 */
function trim(v: number): string {
  const digits = v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return v.toFixed(digits).replace(/\.?0+$/, "");
}

/** 相对时间。updatedAt 为 0 表示首轮采集还没跑完。 */
export function formatRelative(unixSecs: number): string {
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - unixSecs);
  if (diff < 60) return t("panel.justNow");
  if (diff < 3600) return t("panel.minutesAgo", { n: Math.floor(diff / 60) });
  return t("panel.hoursAgo", { n: Math.floor(diff / 3600) });
}
