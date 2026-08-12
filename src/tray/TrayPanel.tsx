import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t, tPlural, translateError } from "../i18n";
import {
  BridgePairingStatus,
  Config,
  Environment,
  TmuxSession,
  UsageSnapshot,
} from "../types";
import { SessionList } from "./SessionList";
import { UsageStrip } from "./UsageStrip";

/** 面板可见时的会话轮询间隔。隐藏时完全不轮询，避免后台空转。 */
const POLL_MS = 4000;

export function TrayPanel() {
  const [snapshot, setSnapshot] = useState<UsageSnapshot | null>(null);
  const [sessions, setSessions] = useState<TmuxSession[]>([]);
  const [env, setEnv] = useState<Environment | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [pairing, setPairing] = useState<BridgePairingStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState("");
  const inFlight = useRef(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    try {
      const [nextSessions, nextSnapshot] = await Promise.all([
        invoke<TmuxSession[]>("get_tmux_sessions"),
        invoke<UsageSnapshot>("get_usage_snapshot"),
      ]);
      setSessions(nextSessions);
      setSnapshot(nextSnapshot);
      // 桥接状态是次要信息，拿不到就不显示，不该拖垮整个面板。
      invoke<BridgePairingStatus>("bridge_pairing")
        .then(setPairing)
        .catch(() => setPairing(null));
    } catch (err) {
      setErrorMsg(translateError(err));
    } finally {
      inFlight.current = false;
    }
  }, []);

  useEffect(() => {
    invoke<Environment>("detect_environment").then(setEnv).catch(() => {});
    invoke<Config>("load_config").then(setConfig).catch(() => {});
    void refresh();

    // 后台采集线程跑完一轮就推快照过来，面板不必自己轮询用量。
    const unlistenUsage = listen<UsageSnapshot>("usage-updated", (event) =>
      setSnapshot(event.payload)
    );
    // 每次左键打开面板都立刻拉一次，而不是等下一个轮询周期。
    // 同时把滚动位置归零：面板是复用的隐藏窗口，上次关闭时的滚动位置会留到下次打开。
    const unlistenOpen = listen("tray-panel-opened", () => {
      if (scrollRef.current) scrollRef.current.scrollTop = 0;
      void refresh();
    });

    return () => {
      void unlistenUsage.then((fn) => fn());
      void unlistenOpen.then((fn) => fn());
    };
  }, [refresh]);

  // 只在面板真正可见时轮询会话。
  useEffect(() => {
    if (document.visibilityState === "hidden") return;
    const id = window.setInterval(() => void refresh(), POLL_MS);
    return () => window.clearInterval(id);
  }, [refresh]);

  const runAction = async (session: string, action: () => Promise<unknown>) => {
    setBusy(session);
    setErrorMsg("");
    try {
      await action();
    } catch (err) {
      setErrorMsg(translateError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleOpen = (name: string) =>
    runAction(name, async () => {
      await invoke("open_session", {
        name,
        terminalId: config?.default_terminal || "ghostty",
      });
      await invoke("panel_hide");
    });

  const handleAddPane = (name: string, agentId: string, count: number) =>
    runAction(name, async () => {
      await invoke("add_panes", { sessionName: name, agentId, count });
      await refresh();
    });

  const sorted = [...sessions].sort((a, b) => {
    if (a.attached !== b.attached) return a.attached ? -1 : 1;
    return b.last_active_ts - a.last_active_ts;
  });

  // 配色约束：窗口是 NSVisualEffectView(popover)，底色若做成不透明会把毛玻璃盖死；
  // 但纯靠低不透明度又会变成一块中性灰、丢掉渐变。折中是中心压暗保证文字对比，
  // 两角留出 indigo/cyan 色偏，让渐变在毛玻璃上仍然读得出来。
  // 次级文字一律用 white/alpha 而非 slate-*：灰阶色会随背后桌面漂移，层级会塌。
  return (
    <div className="td-canvas-translucent animate-tray-in relative flex h-screen flex-col overflow-hidden rounded-[18px] text-white/95">
      {/* 顶部高光 + 内描边，模拟 macOS 原生材质的边缘处理 */}
      <div className="td-sheen pointer-events-none absolute inset-0 rounded-[18px]" />
      <div className="pointer-events-none absolute inset-0 rounded-[18px] ring-1 ring-inset ring-white/15" />

      <header className="relative flex items-center justify-between px-4 pb-2.5 pt-3">
        <h1 className="text-[12px] font-semibold tracking-tight text-white/95">
          {t("app.title")}
        </h1>
        <span className="text-[10px] tabular-nums text-white/40">
          {tPlural("stats.total", sessions.length)}
        </span>
      </header>

      {/* 单一滚动区：用量与工作区一起滚，避免嵌套滚动容器互相抢高度、
          把工作区列表挤成半行。header/footer 保持固定。 */}
      <div ref={scrollRef} className="relative min-h-0 flex-1 overflow-y-auto pb-2">
        <UsageStrip snapshot={snapshot} />
        <SessionList
          sessions={sorted}
          agents={env?.agents ?? []}
          busy={busy}
          onOpen={handleOpen}
          onAddPane={handleAddPane}
        />
      </div>

      {errorMsg && (
        <div className="mx-3 mb-2 rounded-lg border border-rose-800/60 bg-rose-950/50 px-3 py-1.5 text-[10px] text-rose-300">
          {errorMsg}
        </div>
      )}

      <footer className="relative border-t border-white/10 bg-black/10 px-3 py-2">
        {pairing?.enabled && (
          <div className="mb-1.5 flex items-center gap-1.5 px-1 text-[10px] text-white/45">
            <span
              className={`size-1.5 rounded-full ${
                pairing.connectedClients > 0 ? "bg-emerald-400" : "bg-white/25"
              }`}
            />
            <span>
              {pairing.connectedClients > 0
                ? tPlural("panel.mobileConnected", pairing.connectedClients)
                : t("panel.mobileIdle")}
            </span>
            {pairing.trustedLanOnly && (
              <span className="rounded bg-white/10 px-1 py-px">
                {t("panel.trustedLan")}
              </span>
            )}
          </div>
        )}

        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={() => invoke("panel_show_main", { newWorkspace: true })}
            className="flex-1 rounded-lg bg-gradient-to-r from-cyan-500 to-blue-600 px-2 py-1.5 text-[11px] font-medium text-white transition hover:from-cyan-400 hover:to-blue-500"
          >
            {t("btn.newWorkspace")}
          </button>
          <button
            type="button"
            onClick={() => invoke("panel_show_main", { newWorkspace: false })}
            className="rounded-lg border border-white/10 px-2 py-1.5 text-[11px] text-white/75 transition hover:bg-white/10"
          >
            {t("tray.showMain")}
          </button>
          <button
            type="button"
            onClick={() => invoke("panel_quit")}
            title={t("tray.quit")}
            className="rounded-lg border border-white/10 px-2 py-1.5 text-[11px] text-white/60 transition hover:bg-rose-500/20 hover:text-rose-300"
          >
            ✕
          </button>
        </div>
      </footer>
    </div>
  );
}
