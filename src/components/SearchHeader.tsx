import { Search, QrCode } from "lucide-react";
import { t } from "../i18n";

interface SearchHeaderProps {
  search: string;
  onSearchChange: (value: string) => void;
  totalSessions: number;
  runningSessions: number;
  onOpenMobilePairing?: () => void;
}

export function SearchHeader({
  search,
  onSearchChange,
  totalSessions,
  runningSessions,
  onOpenMobilePairing,
}: SearchHeaderProps) {
  const isMac = navigator.userAgent.includes("Macintosh");

  return (
    // 标题栏改成 Overlay 后：红绿灯浮在 webview 左上角，所以 pl-24 给搜索框让位；
    // 同时原生标题栏没了，窗口失去拖拽区，靠 data-tauri-drag-region 补回来
    <header
      data-tauri-drag-region
      className={`flex items-center justify-between pt-4 pb-3 pr-6 shrink-0 max-w-7xl mx-auto w-full ${
        isMac ? "pl-24" : "pl-6"
      }`}
    >
      <div className="flex items-center space-x-3 flex-1 min-w-0">
        <div className="flex-1 max-w-xs transition-all duration-300 focus-within:max-w-sm relative group">
          <Search className="w-4 h-4 absolute left-3.5 top-2.5 text-white/40 group-focus-within:text-cyan-300 transition" />
          <input
            type="text"
            placeholder={t("search.placeholder")}
            value={search}
            onChange={(e) => onSearchChange(e.target.value)}
            className="w-full pl-9 pr-9 py-2 text-xs bg-slate-900/60 backdrop-blur-2xl border border-white/10 rounded-2xl text-slate-100 placeholder-white/40 focus:outline-none focus:border-cyan-400/50 focus:bg-slate-900/80 focus:shadow-lg focus:shadow-cyan-500/10 transition-all duration-300 shadow-inner"
            title={t("search.hint", {
              total: totalSessions,
              running: runningSessions,
            })}
          />
          <div className="absolute right-2.5 top-2 px-1.5 py-0.5 rounded-md bg-white/5 border border-white/10 text-[10px] font-mono text-white/35 pointer-events-none select-none">
            {isMac ? "⌘K" : "Ctrl+K"}
          </div>
        </div>

        {totalSessions > 0 && (
          <div className="hidden sm:flex items-center space-x-2 px-3 py-1.5 rounded-2xl bg-white/[0.04] backdrop-blur-xl border border-white/10 text-xs font-medium">
            <span
              className={`w-2 h-2 rounded-full ${
                runningSessions > 0
                  ? "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.8)] animate-pulse"
                  : "bg-slate-500"
              }`}
            />
            <span className="text-slate-300 text-[11px]">
              {runningSessions} / {totalSessions}
            </span>
          </div>
        )}
      </div>

      <div className="flex items-center space-x-2.5 shrink-0 ml-3">
        {onOpenMobilePairing && (
          <button
            type="button"
            onClick={onOpenMobilePairing}
            className="flex items-center space-x-1.5 px-3 py-1.5 text-xs bg-gradient-to-r from-cyan-500/15 to-blue-500/15 hover:from-cyan-500/25 hover:to-blue-500/25 border border-cyan-400/30 hover:border-cyan-400/50 text-cyan-300 rounded-2xl transition shadow-lg shadow-cyan-950/30 cursor-pointer backdrop-blur-xl font-medium"
            title={t("mobile.openPairing")}
          >
            <QrCode className="w-3.5 h-3.5 text-cyan-300" />
            <span>{t("mobile.openPairing")}</span>
          </button>
        )}
      </div>
    </header>
  );
}
