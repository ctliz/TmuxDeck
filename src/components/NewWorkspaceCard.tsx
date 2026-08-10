import { Plus } from "lucide-react";
import { t } from "../i18n";

interface NewWorkspaceCardProps {
  onClick: () => void;
}

export function NewWorkspaceCard({ onClick }: NewWorkspaceCardProps) {
  return (
    <div
      onClick={onClick}
      className="flex flex-col items-center justify-center min-h-[14rem] rounded-2xl border-2 border-dashed border-white/20 bg-white/5 backdrop-blur-xl hover:bg-white/10 hover:border-cyan-400/50 transition-all duration-300 cursor-pointer group shadow-lg shadow-black/5 animate-fade-in-up"
    >
      <div className="p-3 rounded-2xl bg-white/10 border border-white/15 group-hover:scale-110 group-hover:bg-cyan-500/20 group-hover:border-cyan-400/40 transition-all duration-300 mb-3">
        <Plus className="w-6 h-6 text-white/70 group-hover:text-cyan-300 transition" />
      </div>
      <span className="text-sm font-semibold text-slate-200 group-hover:text-cyan-300 transition">
        {t("btn.newWorkspace")}
      </span>
      <span className="text-[11px] text-slate-400/80 mt-1 px-4 text-center">
        {t("empty.hint")}
      </span>
    </div>
  );
}
