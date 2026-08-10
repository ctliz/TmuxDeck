import { Search } from "lucide-react";
import { t } from "../i18n";

interface SearchHeaderProps {
  search: string;
  onSearchChange: (value: string) => void;
  totalSessions: number;
  runningSessions: number;
}

export function SearchHeader({
  search,
  onSearchChange,
  totalSessions,
  runningSessions,
}: SearchHeaderProps) {
  return (
    <div className="flex items-center justify-center pt-6 pb-2 px-6 shrink-0">
      <div className="relative group w-full max-w-xs transition-all duration-300 focus-within:max-w-sm">
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
    </div>
  );
}
