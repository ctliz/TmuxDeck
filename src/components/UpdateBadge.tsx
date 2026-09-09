import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Sparkles, RefreshCw, ArrowUpCircle, AlertCircle } from "lucide-react";
import { t } from "../i18n";

type UpdateStatus = "idle" | "available" | "downloading" | "ready" | "error";

export function UpdateBadge() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    let unmounted = false;
    async function checkForUpdate() {
      try {
        const result = await check();
        if (result && !unmounted) {
          setUpdate(result);
          setStatus("available");
        }
      } catch {
        // Dev environment or offline: silently ignore
      }
    }
    checkForUpdate();
    return () => {
      unmounted = true;
    };
  }, []);

  if (status === "idle" || !update) {
    return null;
  }

  const handleAction = async () => {
    if (status === "ready") {
      try {
        await relaunch();
      } catch (err) {
        console.error("Failed to relaunch:", err);
      }
      return;
    }

    if (status === "available" || status === "error") {
      try {
        setStatus("downloading");
        setProgress(0);

        let downloaded = 0;
        let total = 0;

        await update.downloadAndInstall((event) => {
          if (event.event === "Started") {
            total = event.data.contentLength ?? 0;
          } else if (event.event === "Progress") {
            downloaded += event.data.chunkLength;
            if (total > 0) {
              setProgress(Math.min(99, Math.round((downloaded / total) * 100)));
            }
          } else if (event.event === "Finished") {
            setProgress(100);
            setStatus("ready");
          }
        });

        setStatus("ready");
      } catch (err) {
        console.error("Update failed:", err);
        setStatus("error");
      }
    }
  };

  return (
    <button
      onClick={handleAction}
      disabled={status === "downloading"}
      className={`flex items-center space-x-1.5 px-3 py-1.5 text-xs rounded-full transition shadow-sm border ${
        status === "ready"
          ? "bg-emerald-500/20 border-emerald-500/50 hover:bg-emerald-500/30 text-emerald-300 animate-pulse"
          : status === "downloading"
          ? "bg-cyan-500/20 border-cyan-500/40 text-cyan-300 cursor-wait"
          : status === "error"
          ? "bg-rose-500/20 border-rose-500/40 hover:bg-rose-500/30 text-rose-300"
          : "bg-amber-500/20 border-amber-500/40 hover:bg-amber-500/30 text-amber-300"
      }`}
      title={update.body || update.version}
    >
      {status === "ready" ? (
        <>
          <ArrowUpCircle className="w-3.5 h-3.5 text-emerald-400" />
          <span>{t("update.ready")}</span>
        </>
      ) : status === "downloading" ? (
        <>
          <RefreshCw className="w-3.5 h-3.5 animate-spin" />
          <span>{t("update.downloading", { progress })}</span>
        </>
      ) : status === "error" ? (
        <>
          <AlertCircle className="w-3.5 h-3.5 text-rose-400" />
          <span>{t("update.error")}</span>
        </>
      ) : (
        <>
          <Sparkles className="w-3.5 h-3.5 text-amber-400 animate-bounce" />
          <span>{t("update.available", { version: update.version })}</span>
        </>
      )}
    </button>
  );
}
