import { useEffect, useRef, useState, useMemo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  ArrowLeft,
  ExternalLink,
  Radio,
} from "lucide-react";
import { agentDisplayName, t, translateName } from "../i18n";
import { Environment, TmuxSession } from "../types";
import { resolvePaneAgentId } from "../utils";

interface AgentTerminalProps {
  session: TmuxSession;
  activePaneId: string;
  onSelectPane: (paneId: string) => void;
  onBack: () => void;
  env: Environment | null;
  selectedTerminal?: string;
  onOpenExternalTerminal?: (sessionName: string, termId?: string) => void;
}

function base64ToUint8Array(base64: string): Uint8Array {
  const binaryString = window.atob(base64);
  const len = binaryString.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}

export function AgentTerminal({
  session,
  activePaneId,
  onSelectPane,
  onBack,
  env,
  selectedTerminal,
  onOpenExternalTerminal,
}: AgentTerminalProps) {
  const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Macintosh");
  const [termStatus, setTermStatus] = useState<"connecting" | "attached" | "exited" | "error">("connecting");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const terminalIdRef = useRef<string | null>(null);
  const writeQueueRef = useRef<Promise<void>>(Promise.resolve());
  const inputBufferRef = useRef<string>("");
  const microtaskScheduledRef = useRef<boolean>(false);
  const activePaneIdRef = useRef(activePaneId);
  const isMountedRef = useRef(true);

  useEffect(() => {
    activePaneIdRef.current = activePaneId;
  }, [activePaneId]);

  // Find active pane info
  const activePane = useMemo(() => {
    return session.panes.find((p) => p.id === activePaneId) || session.panes[0];
  }, [session.panes, activePaneId]);

  const activePaneAgentId = activePane
    ? resolvePaneAgentId(activePane, env?.agents ?? [])
    : undefined;
  const activeAgentTool = activePaneAgentId
    ? env?.agents.find((a) => a.id === activePaneAgentId)
    : undefined;
  const activeAgentName = activeAgentTool
    ? agentDisplayName(activeAgentTool)
    : activePaneAgentId || "Agent";

  const activeTermId =
    session.terminal_id ?? (session.native_split ? "ghostty" : selectedTerminal || "ghostty");
  const matchedTerm = env?.terminals.find((t) => t.id === activeTermId);
  const termName = matchedTerm ? translateName(matchedTerm.name) : activeTermId;

  const attachPane = useCallback(async (paneId: string) => {
    if (!paneId || !isMountedRef.current) return;

    const oldTermId = terminalIdRef.current;
    if (oldTermId) {
      terminalIdRef.current = null;
      try {
        await invoke("close_agent_terminal", { terminalId: oldTermId });
      } catch (err) {
        console.warn("close_agent_terminal failed on switch:", err);
      }
    }

    if (!isMountedRef.current || activePaneIdRef.current !== paneId) {
      return;
    }

    const term = terminalRef.current;
    if (term) {
      term.reset();
    }

    setTermStatus("connecting");
    setErrorMessage(null);

    // Pre-allocate terminalId on client side so xterm listener immediately matches incoming output
    const terminalId = `term_${crypto.randomUUID()}`;
    terminalIdRef.current = terminalId;

    try {
      const cols = term && term.cols > 0 ? term.cols : 80;
      const rows = term && term.rows > 0 ? term.rows : 24;
      await invoke("open_agent_terminal", {
        terminalId,
        paneId,
        cols,
        rows,
      });

      if (isMountedRef.current && activePaneIdRef.current === paneId && terminalIdRef.current === terminalId) {
        setTermStatus("attached");
        term?.focus();
      } else {
        if (terminalIdRef.current === terminalId) {
          terminalIdRef.current = null;
        }
        invoke("close_agent_terminal", { terminalId }).catch(() => {});
      }
    } catch (err: any) {
      console.error("open_agent_terminal failed:", err);
      if (terminalIdRef.current === terminalId) {
        terminalIdRef.current = null;
      }
      invoke("close_agent_terminal", { terminalId }).catch(() => {});
      if (isMountedRef.current && activePaneIdRef.current === paneId) {
        setTermStatus("error");
        setErrorMessage(String(err?.message || err));
      }
    }
  }, []);

  // Initialize xterm and Tauri event listeners
  useEffect(() => {
    isMountedRef.current = true;

    const term = new Terminal({
      cursorBlink: true,
      scrollback: 5000,
      fontFamily: "JetBrains Mono, Menlo, Monaco, 'Courier New', monospace",
      fontSize: 13,
      lineHeight: 1.2,
      theme: {
        background: "#090d16",
        foreground: "#f1f5f9",
        cursor: "#38bdf8",
        selectionBackground: "rgba(56, 189, 248, 0.3)",
        black: "#1e293b",
        red: "#f43f5e",
        green: "#10b981",
        yellow: "#f59e0b",
        blue: "#3b82f6",
        magenta: "#a855f7",
        cyan: "#06b6d4",
        white: "#f8fafc",
        brightBlack: "#475569",
        brightRed: "#fb7185",
        brightGreen: "#34d399",
        brightYellow: "#fbbf24",
        brightBlue: "#60a5fa",
        brightMagenta: "#c084fc",
        brightCyan: "#22d3ee",
        brightWhite: "#ffffff",
      },
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);

    if (containerRef.current) {
      term.open(containerRef.current);
      try {
        fitAddon.fit();
      } catch {
        // initial fit ignore
      }
    }

    terminalRef.current = term;
    fitAddonRef.current = fitAddon;

    // Micro-batch onData within the same JS tick to avoid latency build-up for terminal queries (DA/OSC)
    const dataDisposable = term.onData((data) => {
      inputBufferRef.current += data;
      if (!microtaskScheduledRef.current) {
        microtaskScheduledRef.current = true;
        queueMicrotask(() => {
          microtaskScheduledRef.current = false;
          const payload = inputBufferRef.current;
          inputBufferRef.current = "";
          if (!payload || !isMountedRef.current) return;

          const currentTerminalId = terminalIdRef.current;
          if (currentTerminalId) {
            writeQueueRef.current = writeQueueRef.current
              .then(async () => {
                await invoke("write_agent_terminal", { terminalId: currentTerminalId, data: payload });
              })
              .catch((err) => {
                console.error("write_agent_terminal error:", err);
              });
          }
        });
      }
    });

    // Handle terminal resize events
    const resizeDisposable = term.onResize(({ cols, rows }) => {
      const currentTerminalId = terminalIdRef.current;
      if (currentTerminalId && cols > 0 && rows > 0) {
        invoke("resize_agent_terminal", { terminalId: currentTerminalId, cols, rows }).catch(() => {});
      }
    });

    // ResizeObserver for viewport changes
    const resizeObserver = new ResizeObserver(() => {
      if (!isMountedRef.current || !containerRef.current) return;
      try {
        fitAddon.fit();
        const cols = term.cols;
        const rows = term.rows;
        const currentTerminalId = terminalIdRef.current;
        if (currentTerminalId && cols > 0 && rows > 0) {
          invoke("resize_agent_terminal", { terminalId: currentTerminalId, cols, rows }).catch(() => {});
        }
      } catch {
        // ignore
      }
    });

    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
    }

    let unlistenOutput: UnlistenFn | null = null;
    let unlistenExit: UnlistenFn | null = null;

    // First register all Tauri event listeners, THEN invoke open_agent_terminal
    const init = async () => {
      const [fnOutput, fnExit] = await Promise.all([
        listen<{ terminalId: string; data: string }>("agent-terminal-output", (event) => {
          if (!isMountedRef.current) return;
          if (event.payload.terminalId === terminalIdRef.current && terminalRef.current) {
            try {
              const bytes = base64ToUint8Array(event.payload.data);
              terminalRef.current.write(bytes);
            } catch (err) {
              console.error("Terminal output decoding error:", err);
            }
          }
        }),
        listen<{ terminalId: string }>("agent-terminal-exit", (event) => {
          if (!isMountedRef.current) return;
          if (event.payload.terminalId === terminalIdRef.current) {
            setTermStatus("exited");
            if (terminalRef.current) {
              terminalRef.current.write("\r\n\x1b[33m[Agent terminal session disconnected]\x1b[0m\r\n");
            }
          }
        }),
      ]);

      if (!isMountedRef.current) {
        fnOutput();
        fnExit();
        return;
      }

      unlistenOutput = fnOutput;
      unlistenExit = fnExit;

      await attachPane(activePaneIdRef.current);
    };

    void init();

    return () => {
      isMountedRef.current = false;
      resizeObserver.disconnect();
      dataDisposable.dispose();
      resizeDisposable.dispose();

      inputBufferRef.current = "";
      microtaskScheduledRef.current = false;

      if (unlistenOutput) unlistenOutput();
      if (unlistenExit) unlistenExit();

      const termIdToClose = terminalIdRef.current;
      if (termIdToClose) {
        terminalIdRef.current = null;
        invoke("close_agent_terminal", { terminalId: termIdToClose }).catch(() => {});
      }

      term.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, [attachPane]);

  // Handle pane switching
  const prevPaneIdRef = useRef(activePaneId);
  useEffect(() => {
    if (prevPaneIdRef.current === activePaneId) return;
    prevPaneIdRef.current = activePaneId;
    if (terminalRef.current) {
      void attachPane(activePaneId);
    }
  }, [activePaneId, attachPane]);

  return (
    <div className="td-canvas flex flex-col h-screen text-slate-100 font-sans select-none overflow-hidden">
      {/* Header Bar */}
      <header
        data-tauri-drag-region
        className={`flex items-center justify-between py-3 pr-6 bg-slate-900/60 backdrop-blur-2xl border-b border-white/10 shrink-0 ${
          isMac ? "pl-20" : "pl-6"
        }`}
      >
        <div className="flex items-center space-x-3 min-w-0">
          <button
            type="button"
            onClick={onBack}
            className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-full bg-white/10 hover:bg-white/15 border border-white/15 text-slate-200 hover:text-white transition shadow-sm cursor-pointer text-xs font-medium"
            title={t("terminal.back")}
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>{t("terminal.back")}</span>
          </button>

          <div className="h-4 w-px bg-white/15 mx-1" />

          <div className="flex items-center space-x-2 min-w-0">
            <h1 className="text-sm font-semibold text-slate-100 truncate tracking-tight">
              {session.name}
            </h1>
            <span className="text-xs text-white/30">·</span>
            <span className="text-xs text-cyan-300 font-medium px-2 py-0.5 rounded-full bg-cyan-950/50 border border-cyan-500/30 truncate">
              {activeAgentName}
            </span>
          </div>

          {/* Pane Switcher Tabs */}
          {session.panes.length > 1 && (
            <div className="flex items-center space-x-1 bg-black/40 p-0.5 rounded-xl border border-white/10 ml-2">
              {session.panes.map((pane, idx) => {
                const isSelected = pane.id === activePaneId;
                const paneAgent = resolvePaneAgentId(pane, env?.agents ?? []);
                const label = pane.slot ? `Slot ${pane.slot}` : `#${idx + 1}`;
                return (
                  <button
                    key={pane.id}
                    type="button"
                    onClick={() => onSelectPane(pane.id)}
                    className={`px-2.5 py-1 rounded-lg text-xs font-medium transition cursor-pointer flex items-center space-x-1 ${
                      isSelected
                        ? "bg-gradient-to-r from-cyan-500 to-blue-500 text-slate-950 font-bold shadow-md shadow-cyan-500/20"
                        : "text-slate-400 hover:text-slate-200 hover:bg-white/5"
                    }`}
                  >
                    <span>{label}</span>
                    {paneAgent && <span className="opacity-75">· {paneAgent}</span>}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <div className="flex items-center space-x-3 shrink-0">
          {/* Status Badge */}
          <div
            className={`flex items-center space-x-1.5 px-3 py-1 rounded-full border text-xs font-medium ${
              termStatus === "attached"
                ? "bg-emerald-500/10 text-emerald-300 border-emerald-500/30 shadow-sm shadow-emerald-500/10"
                : termStatus === "connecting"
                ? "bg-amber-500/10 text-amber-300 border-amber-500/30 animate-pulse shadow-sm shadow-amber-500/10"
                : termStatus === "exited"
                ? "bg-slate-500/10 text-slate-300 border-slate-500/30"
                : "bg-rose-500/10 text-rose-300 border-rose-500/30 shadow-sm shadow-rose-500/10"
            }`}
          >
            <Radio
              className={`w-3 h-3 ${
                termStatus === "attached"
                  ? "text-emerald-400"
                  : termStatus === "connecting"
                  ? "text-amber-400 animate-spin"
                  : "text-slate-400"
              }`}
            />
            <span>
              {termStatus === "attached"
                ? t("terminal.status.connected")
                : termStatus === "connecting"
                ? t("terminal.status.connecting")
                : termStatus === "exited"
                ? t("terminal.status.exited")
                : t("terminal.status.disconnected")}
            </span>
          </div>

          {/* External Terminal Fallback Button */}
          {onOpenExternalTerminal && (
            <button
              type="button"
              onClick={() => onOpenExternalTerminal(session.name, activeTermId)}
              className="p-1.5 rounded-full bg-white/5 hover:bg-white/10 border border-white/10 text-slate-400 hover:text-slate-200 transition cursor-pointer flex items-center space-x-1 text-xs shadow-sm"
              title={`${t("terminal.openFallback")} (${termName})`}
            >
              <ExternalLink className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </header>

      {/* Error Notice */}
      {errorMessage && (
        <div className="px-6 py-2 bg-rose-950/60 border-b border-rose-800/40 text-rose-300 text-xs flex items-center justify-between shrink-0">
          <span>{errorMessage}</span>
        </div>
      )}

      {/* Native xterm.js Terminal Container Card */}
      <div className="flex-1 p-3 overflow-hidden flex flex-col min-h-0">
        <main
          className="flex-1 w-full h-full p-2.5 bg-[#090d16]/95 backdrop-blur-xl rounded-2xl border border-white/10 shadow-2xl shadow-black/80 overflow-hidden"
          onClick={() => terminalRef.current?.focus()}
        >
          <div ref={containerRef} className="w-full h-full" />
        </main>
      </div>
    </div>
  );
}
