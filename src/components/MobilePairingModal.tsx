import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { QrCode, Copy, Check, RefreshCw, Smartphone, Wifi, WifiOff, X, ShieldAlert } from "lucide-react";
import { t, tPlural } from "../i18n";
import { BridgePairingStatus } from "../types";
import { QRCodeView } from "./QRCodeView";

interface MobilePairingModalProps {
  show: boolean;
  onClose: () => void;
}

export function MobilePairingModal({ show, onClose }: MobilePairingModalProps) {
  const [status, setStatus] = useState<BridgePairingStatus | null>(null);
  const [selectedUrlIndex, setSelectedUrlIndex] = useState<number>(0);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(false);

  const fetchStatus = async () => {
    setLoading(true);
    try {
      // Call ONLY bridge_pairing Tauri command per developer spec
      const res = await invoke<BridgePairingStatus>("bridge_pairing");
      setStatus(res);
    } catch {
      // If bridge_pairing is unavailable or broker is offline, do NOT fake service online
      setStatus(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (show) {
      fetchStatus();
      let unlistenFn: (() => void) | null = null;
      listen("mobile-clients-changed", (event: any) => {
        const payload = event?.payload;
        const count = typeof payload === "number" ? payload : payload?.connectedClients;
        if (typeof count === "number") {
          setStatus((prev) => (prev ? { ...prev, connectedClients: count } : null));
        } else {
          fetchStatus();
        }
      }).then((fn) => {
        unlistenFn = fn;
      });

      const timer = setInterval(fetchStatus, 5000);
      return () => {
        if (unlistenFn) unlistenFn();
        clearInterval(timer);
      };
    }
  }, [show]);

  if (!show) return null;

  const urls = status?.httpUrls || (status?.httpUrl ? [status.httpUrl] : []);
  const currentUrl = urls[selectedUrlIndex] || urls[0] || "";

  const urlKind = (url: string) => {
    try {
      const host = new URL(url).hostname;
      if (host === "127.0.0.1" || host === "localhost") return t("mobile.ipLocal");
      if (/^100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\./.test(host) || host.endsWith(".ts.net")) {
        return t("mobile.ipTailscale");
      }
      return t("mobile.ipLan");
    } catch {
      return t("mobile.ipLan");
    }
  };

  const handleRefresh = async () => {
    setLoading(true);
    try {
      const res = await invoke<BridgePairingStatus>("refresh_bridge_pairing");
      setStatus(res);
    } catch {
      setStatus(null);
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = () => {
    if (!currentUrl) return;
    navigator.clipboard.writeText(currentUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-md animate-fade-in">
      <div className="relative w-full max-w-md bg-slate-900 border border-slate-800 rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800/80 bg-slate-900/60">
          <div className="flex items-center space-x-2 text-cyan-400">
            <QrCode className="w-5 h-5" />
            <h2 className="text-base font-semibold text-slate-100">{t("mobile.pairingTitle")}</h2>
            <span className="flex items-center space-x-1 px-2 py-0.5 text-[10px] bg-amber-500/10 border border-amber-500/30 text-amber-400 font-medium rounded-full">
              <ShieldAlert className="w-3 h-3 mr-0.5" />
              {t("mobile.trustedLanOnly")}
            </span>
          </div>
          <div className="flex items-center space-x-1">
            <button
              type="button"
              onClick={handleRefresh}
              disabled={loading}
              title={t("mobile.refreshPairing")}
              className="p-1 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800/60 transition disabled:opacity-40"
            >
              <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
            </button>
            <button
              onClick={onClose}
              className="p-1 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800/60 transition"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 overflow-y-auto max-h-[80vh]">
          <p className="text-xs text-slate-400">{t("mobile.pairingDesc")}</p>

          {/* QR Code Section */}
          <div className="flex flex-col items-center justify-center space-y-3">
            {currentUrl ? (
              <QRCodeView text={currentUrl} size={190} />
            ) : (
              <div className="w-[190px] h-[190px] bg-slate-800/40 rounded-xl flex items-center justify-center text-xs text-slate-500">
                {loading ? t("btn.creating") : t("mobile.brokerOffline")}
              </div>
            )}
          </div>

          {/* Status Bar */}
          <div className="flex items-center justify-between p-3 rounded-xl bg-slate-800/40 border border-slate-800 text-xs">
            <div className="flex items-center space-x-2">
              {status?.enabled ? (
                <>
                  <Wifi className="w-4 h-4 text-emerald-400 animate-pulse" />
                  <span className="text-emerald-400 font-medium">{t("mobile.brokerOnline")}</span>
                </>
              ) : (
                <>
                  <WifiOff className="w-4 h-4 text-rose-400" />
                  <span className="text-rose-400 font-medium">{t("mobile.brokerOffline")}</span>
                </>
              )}
            </div>
            <div className="flex items-center space-x-1.5 text-slate-300">
              <Smartphone className="w-4 h-4 text-cyan-400" />
              <span>
                {status?.connectedClients && status.connectedClients > 0
                  ? tPlural("mobile.connectedClients", status.connectedClients)
                  : t("mobile.noClients")}
              </span>
            </div>
          </div>

          {/* URL Selection & Copy */}
          {urls.length > 0 && (
            <div className="space-y-2">
              <label className="block text-xs font-medium text-slate-400">{t("mobile.lanUrls")}</label>
              {urls.length > 1 && (
                <div className="flex flex-wrap gap-1.5 mb-2">
                  {urls.map((_, idx) => (
                    <button
                      key={idx}
                      onClick={() => setSelectedUrlIndex(idx)}
                      className={`px-2.5 py-1 text-[11px] rounded-lg transition font-mono ${
                        selectedUrlIndex === idx
                          ? "bg-cyan-500/20 border border-cyan-500/50 text-cyan-300"
                          : "bg-slate-800/60 border border-slate-700/50 text-slate-400 hover:text-slate-200"
                      }`}
                    >
                      {urlKind(urls[idx])}
                    </button>
                  ))}
                </div>
              )}
              <div className="flex items-center space-x-2">
                <input
                  type="text"
                  readOnly
                  value={currentUrl}
                  className="flex-1 px-3 py-2 text-xs font-mono bg-slate-950 border border-slate-800 rounded-xl text-slate-200 focus:outline-none select-all"
                />
                <button
                  onClick={handleCopy}
                  className="flex items-center space-x-1.5 px-3 py-2 text-xs font-medium bg-cyan-600 hover:bg-cyan-500 text-white rounded-xl transition shadow-lg shadow-cyan-600/20"
                >
                  {copied ? (
                    <>
                      <Check className="w-3.5 h-3.5 text-emerald-300" />
                      <span>{t("mobile.copiedLink")}</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      <span>{t("mobile.copyLink")}</span>
                    </>
                  )}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
