import { Terminal, Copy, Check } from "lucide-react";
import { t } from "../i18n";
import { Environment } from "../types";

interface TmuxMissingScreenProps {
  env: Environment;
  copiedBrew: boolean;
  onCopyBrew: () => void;
  onCopyWslInstall: () => void;
  onCopyWslApt: () => void;
  onRecheck: () => void;
}

export function TmuxMissingScreen({
  env,
  copiedBrew,
  onCopyBrew,
  onCopyWslInstall,
  onCopyWslApt,
  onRecheck,
}: TmuxMissingScreenProps) {
  const isWindows = env.terminals.some(
    (term) => term.id === "wt" || term.id === "cmd" || term.id === "powershell"
  );

  return (
    <div className="flex flex-col items-center justify-center h-screen bg-slate-950 text-slate-100 p-6 select-none">
      <div className="max-w-md w-full p-8 rounded-3xl bg-slate-900 border border-slate-800 shadow-2xl text-center space-y-6">
        <div className="w-16 h-16 rounded-2xl bg-rose-950/60 border border-rose-800/80 text-rose-400 flex items-center justify-center mx-auto">
          <Terminal className="w-8 h-8" />
        </div>
        <div>
          <h2 className="text-xl font-bold text-slate-100">
            {isWindows ? t("tmux.missing.win") : t("tmux.missing.title")}
          </h2>
          <p className="text-sm text-slate-400 mt-2">
            {isWindows ? t("tmux.missing.win_hint") : t("tmux.missing.hint")}
          </p>
        </div>
        <div className="flex flex-col space-y-2">
          {isWindows ? (
            <>
              <div className="flex items-center justify-between p-3 rounded-xl bg-slate-950 border border-slate-800 font-mono text-xs">
                <span className="text-cyan-400">wsl --install</span>
                <button
                  onClick={onCopyWslInstall}
                  className="p-1 text-slate-400 hover:text-white cursor-pointer"
                >
                  {copiedBrew ? (
                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                </button>
              </div>
              <div className="flex items-center justify-between p-3 rounded-xl bg-slate-950 border border-slate-800 font-mono text-xs">
                <span className="text-cyan-400">wsl sudo apt install tmux</span>
                <button
                  onClick={onCopyWslApt}
                  className="p-1 text-slate-400 hover:text-white cursor-pointer"
                >
                  {copiedBrew ? (
                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                </button>
              </div>
            </>
          ) : (
            <div className="flex items-center justify-between p-3.5 rounded-xl bg-slate-950 border border-slate-800 font-mono text-sm">
              <span className="text-cyan-400">brew install tmux</span>
              <button
                onClick={onCopyBrew}
                className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition flex items-center space-x-1 cursor-pointer"
              >
                {copiedBrew ? (
                  <>
                    <Check className="w-4 h-4 text-emerald-400" />
                    <span className="text-xs text-emerald-400 font-sans">
                      {t("btn.copied")}
                    </span>
                  </>
                ) : (
                  <>
                    <Copy className="w-4 h-4" />
                    <span className="text-xs font-sans">{t("btn.copy")}</span>
                  </>
                )}
              </button>
            </div>
          )}
        </div>
        <button
          onClick={onRecheck}
          className="w-full py-2.5 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium text-sm shadow-lg shadow-cyan-500/20 transition cursor-pointer"
        >
          {t("btn.recheck")}
        </button>
      </div>
    </div>
  );
}
