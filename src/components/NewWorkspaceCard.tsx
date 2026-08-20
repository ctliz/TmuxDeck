import { Plus } from "lucide-react";
import { t } from "../i18n";

interface NewWorkspaceCardProps {
  onClick: () => void;
}

export function NewWorkspaceCard({ onClick }: NewWorkspaceCardProps) {
  return (
    <div
      onClick={onClick}
      className="flex flex-col items-center justify-center min-h-[14rem] rounded-2xl border border-dashed border-white/20 bg-slate-900/35 backdrop-blur-2xl hover:bg-slate-900/60 hover:border-cyan-400/60 transition-all duration-300 cursor-pointer group shadow-xl shadow-black/40 hover:shadow-2xl hover:shadow-cyan-500/10 animate-fade-in-up"
    >
      <div className="p-3.5 rounded-2xl bg-white/[0.06] border border-white/10 group-hover:scale-110 group-hover:bg-gradient-to-br group-hover:from-cyan-500/20 group-hover:to-blue-600/20 group-hover:border-cyan-400/50 group-hover:shadow-[0_0_20px_rgba(6,182,212,0.3)] transition-all duration-300 mb-3">
        <Plus className="w-6 h-6 text-white/60 group-hover:text-cyan-300 transition" />
      </div>
      <span className="text-sm font-semibold text-slate-200 group-hover:text-cyan-300 transition tracking-tight">
        {t("btn.newWorkspace")}
      </span>
      <span className="text-[11px] text-slate-400/70 mt-1.5 px-4 text-center">
        {t("empty.hint")}
      </span>
    </div>
  );
}
