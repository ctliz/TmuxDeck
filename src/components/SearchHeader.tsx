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
  return (
    <div className="flex items-center justify-between pt-6 pb-2 px-6 shrink-0 max-w-7xl mx-auto w-full">
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
