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
    // （点在 input/button 上时事件目标不是本元素，不会误触发拖拽）。
    <div
      data-tauri-drag-region
      className={`flex items-center justify-between pt-4 pb-2 pr-6 shrink-0 max-w-7xl mx-auto w-full ${
        isMac ? "pl-24" : "pl-6"
      }`}
    >
      <div className="flex-1 max-w-xs transition-all duration-300 focus-within:max-w-sm relative group">
        <Search className="w-4 h-4 absolute left-3.5 top-2.5 text-white/40 group-focus-within:text-cyan-400 transition" />
        <input
          type="text"
          placeholder={t("search.placeholder")}
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          className="w-full pl-9 pr-4 py-1.5 text-xs bg-white/10 backdrop-blur-xl border border-white/15 rounded-full text-slate-100 placeholder-white/40 focus:outline-none focus:border-cyan-500/60 focus:bg-white/15 focus:shadow-lg focus:shadow-cyan-500/10 transition-all duration-300"
          title={t("search.hint", {
            total: totalSessions,
            running: runningSessions,
          })}
        />
      </div>

      {onOpenMobilePairing && (
        <button
          onClick={onOpenMobilePairing}
          className="flex items-center space-x-1.5 px-3 py-1.5 text-xs bg-cyan-600/20 border border-cyan-500/40 hover:bg-cyan-600/30 text-cyan-300 rounded-full transition shadow-sm ml-3"
          title={t("mobile.openPairing")}
        >
          <QrCode className="w-3.5 h-3.5" />
          <span>{t("mobile.openPairing")}</span>
        </button>
      )}
    </div>
  );
}
