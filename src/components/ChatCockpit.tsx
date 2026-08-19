import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft,
  Send,
  Square,
  Terminal,
  Activity,
  Zap,
  Radio,
} from "lucide-react";
import { agentDisplayName, t, translateName } from "../i18n";
import {
  BridgePairingStatus,
  ChatTurn,
  ConversationItem,
  ConversationStatus,
  Environment,
  TmuxSession,
} from "../types";
import { resolvePaneAgentId } from "../utils";

interface ChatCockpitProps {
  session: TmuxSession;
  activePaneId: string;
  onSelectPane: (paneId: string) => void;
  onBack: () => void;
  env: Environment | null;
  selectedTerminal?: string;
  onOpenTerminal?: (sessionName: string, termId?: string) => void;
}

export function ChatCockpit({
  session,
  activePaneId,
  onSelectPane,
  onBack,
  env,
  selectedTerminal,
  onOpenTerminal,
}: ChatCockpitProps) {
  const [wsStatus, setWsStatus] = useState<"connecting" | "connected" | "disconnected">("connecting");
  const [conversations, setConversations] = useState<ConversationItem[]>([]);
  const [turns, setTurns] = useState<Record<string, ChatTurn[]>>({});
  const [inputText, setInputText] = useState("");
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const turnsEndRef = useRef<HTMLDivElement | null>(null);
  const isMountedRef = useRef(true);
  const activePaneIdRef = useRef(activePaneId);

  useEffect(() => {
    activePaneIdRef.current = activePaneId;
  }, [activePaneId]);

  // Scroll to bottom when turns update or pane changes
  useEffect(() => {
    turnsEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [turns, activePaneId]);

  // Connect to Desktop WebSocket
  useEffect(() => {
    isMountedRef.current = true;

    const connectWs = async () => {
      if (!isMountedRef.current) return;
      setWsStatus("connecting");

      try {
        const pairing = await invoke<BridgePairingStatus>("bridge_pairing");
        const wsUrl =
          pairing.desktopWsUrl ||
          (pairing.port && pairing.token
            ? `ws://127.0.0.1:${pairing.port}/v1/ws?token=${pairing.token}`
            : pairing.wsUrls && pairing.wsUrls.length > 0
            ? pairing.wsUrls[0]
            : pairing.wsUrl);

        if (!wsUrl) {
          if (isMountedRef.current) {
            setWsStatus("disconnected");
            reconnectTimeoutRef.current = setTimeout(() => {
              if (isMountedRef.current) {
                connectWs();
              }
            }, 2500);
          }
          return;
        }

        const ws = new WebSocket(wsUrl, ["tmuxdeck.v1"]);
        wsRef.current = ws;

        ws.onopen = () => {
          if (!isMountedRef.current) {
            ws.close();
            return;
          }
          setWsStatus("connected");
          ws.send(JSON.stringify({ type: "refresh" }));
          const currentTarget = activePaneIdRef.current;
          if (currentTarget) {
            // Reset turns before receiving fresh transcript snapshot to avoid duplicate turns
            setTurns((prev) => ({ ...prev, [currentTarget]: [] }));
            ws.send(JSON.stringify({ type: "subscribe", id: currentTarget }));
          }
        };

        ws.onmessage = (event) => {
          try {
            const msg = JSON.parse(event.data);
            switch (msg.type) {
              case "conversations":
                if (Array.isArray(msg.items)) {
                  setConversations(msg.items);
                }
                break;
              case "status-changed":
                setConversations((prev) =>
                  prev.map((c) => (c.id === msg.id ? { ...c, status: msg.status } : c))
                );
                break;
              case "awaiting-human":
                setConversations((prev) =>
                  prev.map((c) =>
                    c.id === msg.id ? { ...c, status: "awaiting-human" as ConversationStatus } : c
                  )
                );
                break;
              case "turn":
                if (msg.turn) {
                  const cid =
                    msg.turn.conversationId ||
                    msg.turn.conversation_id ||
                    activePaneId;
                  setTurns((prev) => {
                    const existing = prev[cid] ? [...prev[cid]] : [];
                    existing.push(msg.turn);
                    return { ...prev, [cid]: existing };
                  });
                }
                break;
              default:
                break;
            }
          } catch (err) {
            console.error("WS message parse error:", err);
          }
        };

        ws.onerror = () => {
          if (isMountedRef.current) {
            setWsStatus("disconnected");
          }
        };

        ws.onclose = () => {
          if (isMountedRef.current) {
            setWsStatus("disconnected");
            // Retry connection after 2.5s
            reconnectTimeoutRef.current = setTimeout(() => {
              if (isMountedRef.current) {
                connectWs();
              }
            }, 2500);
          }
        };
      } catch (err) {
        console.error("Failed to read bridge pairing for WebSocket:", err);
        if (isMountedRef.current) {
          setWsStatus("disconnected");
          reconnectTimeoutRef.current = setTimeout(() => {
            if (isMountedRef.current) {
              connectWs();
            }
          }, 3000);
        }
      }
    };

    connectWs();

    return () => {
      isMountedRef.current = false;
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        try {
          if (wsRef.current.readyState === WebSocket.OPEN) {
            wsRef.current.send(JSON.stringify({ type: "unsubscribe" }));
          }
          wsRef.current.close();
        } catch {
          // ignore
        }
        wsRef.current = null;
      }
    };
  }, []);

  // Handle pane switch: unsubscribe previous, clear current pane turns, subscribe next
  const prevPaneIdRef = useRef(activePaneId);
  useEffect(() => {
    if (prevPaneIdRef.current !== activePaneId) {
      if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ type: "unsubscribe" }));
        setTurns((prev) => ({ ...prev, [activePaneId]: [] }));
        wsRef.current.send(JSON.stringify({ type: "subscribe", id: activePaneId }));
      }
      prevPaneIdRef.current = activePaneId;
    }
  }, [activePaneId]);

  const handleSend = () => {
    const text = inputText.trim();
    if (!text || !activePaneId) return;

    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: "say", id: activePaneId, text }));
      setInputText("");
    }
  };

  const handleStop = () => {
    if (!activePaneId) return;
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: "key", id: activePaneId, key: "C-c" }));
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

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

  const activeConversation = conversations.find((c) => c.id === activePaneId);
  const currentStatus: ConversationStatus = activeConversation?.status || "unknown";

  const statusBadge = useMemo(() => {
    switch (currentStatus) {
      case "awaiting-human":
        return {
          label: t("chat.status.awaitingHuman"),
          className: "bg-rose-500/20 text-rose-300 border-rose-500/40 animate-pulse",
          dotClass: "bg-rose-400 animate-ping",
        };
      case "thinking":
        return {
          label: t("chat.status.thinking"),
          className: "bg-amber-500/20 text-amber-300 border-amber-500/40 animate-pulse",
          dotClass: "bg-amber-400",
        };
      case "running-tool":
        return {
          label: t("chat.status.runningTool"),
          className: "bg-purple-500/20 text-purple-300 border-purple-500/40",
          dotClass: "bg-purple-400",
        };
      case "idle":
        return {
          label: t("chat.status.idle"),
          className: "bg-emerald-500/20 text-emerald-300 border-emerald-500/40",
          dotClass: "bg-emerald-400",
        };
      default:
        return {
          label: t("chat.status.unknown"),
          className: "bg-slate-500/20 text-slate-300 border-slate-500/40",
          dotClass: "bg-slate-400",
        };
    }
  }, [currentStatus]);

  const currentTurns = turns[activePaneId] || [];

  const activeTermId =
    session.terminal_id ?? (session.native_split ? "ghostty" : selectedTerminal || "ghostty");
  const matchedTerm = env?.terminals.find((t) => t.id === activeTermId);
  const termName = matchedTerm ? translateName(matchedTerm.name) : activeTermId;

  return (
    <div className="flex flex-col h-full bg-slate-950 text-slate-100 font-sans overflow-hidden">
      {/* Header Bar */}
      <header className="flex items-center justify-between px-6 py-3.5 bg-slate-900/80 backdrop-blur-xl border-b border-white/10 shrink-0">
        <div className="flex items-center space-x-3 min-w-0">
          <button
            type="button"
            onClick={onBack}
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-white/5 hover:bg-white/10 border border-white/10 text-slate-300 hover:text-white transition cursor-pointer text-xs font-medium"
            title={t("chat.back")}
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>{t("chat.back")}</span>
          </button>

          <div className="h-4 w-px bg-white/15 mx-1" />

          <div className="flex items-center space-x-2 min-w-0">
            <h1 className="text-base font-semibold text-slate-100 truncate">
              {session.name}
            </h1>
            <span className="text-xs text-slate-400">·</span>
            <span className="text-xs text-cyan-300 font-medium truncate">
              {activeAgentName}
            </span>
          </div>

          {/* Pane Switcher Tabs */}
          {session.panes.length > 1 && (
            <div className="flex items-center space-x-1 bg-black/40 p-1 rounded-xl border border-white/10 ml-2">
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
                        ? "bg-cyan-500 text-slate-950 font-bold shadow-sm shadow-cyan-500/30"
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
          {/* Agent Status Badge */}
          <div
            className={`flex items-center space-x-1.5 px-2.5 py-1 rounded-full border text-xs font-medium ${statusBadge.className}`}
          >
            <span className={`w-2 h-2 rounded-full ${statusBadge.dotClass}`} />
            <span>{statusBadge.label}</span>
          </div>

          {/* WebSocket Link Status */}
          <div
            className="flex items-center space-x-1 text-[11px] text-slate-400"
            title={`WebSocket: ${wsStatus}`}
          >
            <Radio
              className={`w-3.5 h-3.5 ${
                wsStatus === "connected"
                  ? "text-emerald-400"
                  : wsStatus === "connecting"
                  ? "text-amber-400 animate-spin"
                  : "text-rose-400"
              }`}
            />
            <span className="hidden sm:inline">
              {wsStatus === "connected"
                ? t("chat.ws.connected")
                : wsStatus === "connecting"
                ? t("chat.ws.connecting")
                : t("chat.ws.disconnected")}
            </span>
          </div>

          {/* Terminal Fallback Button */}
          {onOpenTerminal && (
            <button
              type="button"
              onClick={() => onOpenTerminal(session.name, activeTermId)}
              className="p-1.5 rounded-lg bg-white/5 hover:bg-white/10 border border-white/10 text-slate-400 hover:text-slate-200 transition cursor-pointer flex items-center space-x-1 text-xs"
              title={t("chat.openInTerminal") + ` (${termName})`}
            >
              <Terminal className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </header>

      {/* Disconnection Banner if offline */}
      {wsStatus === "disconnected" && (
        <div className="px-6 py-2 bg-rose-950/60 border-b border-rose-800/40 text-rose-300 text-xs flex items-center justify-between shrink-0">
          <span>{t("chat.ws.offlineNotice")}</span>
        </div>
      )}

      {/* Turns / Message Stream Area */}
      <main className="flex-1 overflow-y-auto p-6 space-y-4">
        {currentTurns.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-center text-slate-400 space-y-3 py-16">
            <div className="w-12 h-12 rounded-2xl bg-cyan-500/10 border border-cyan-500/20 flex items-center justify-center text-cyan-400">
              <Activity className="w-6 h-6" />
            </div>
            <p className="text-sm max-w-sm">{t("chat.noTurns")}</p>
          </div>
        ) : (
          currentTurns.map((turn, index) => {
            const isHuman = turn.role === "human";
            const isPeer = turn.role === "peer";
            const isSystem = turn.role === "system";

            if (isSystem) {
              return (
                <div key={index} className="flex justify-center my-2">
                  <span className="px-3 py-1 rounded-full bg-white/5 border border-white/10 text-slate-400 text-xs font-mono">
                    {turn.text}
                  </span>
                </div>
              );
            }

            const timeStr = turn.timestamp
              ? new Date(turn.timestamp).toLocaleTimeString([], {
                  hour: "2-digit",
                  minute: "2-digit",
                })
              : "";

            return (
              <div
                key={index}
                className={`flex flex-col ${
                  isHuman ? "items-end" : "items-start"
                } max-w-3xl ${isHuman ? "ml-auto" : "mr-auto"}`}
              >
                {/* Role & Time header */}
                <div className="flex items-center space-x-1.5 px-1 mb-1 text-[11px] text-slate-400">
                  {isHuman ? (
                    <span className="font-semibold text-cyan-300">{t("chat.role.you")}</span>
                  ) : isPeer ? (
                    <span className="font-semibold text-purple-300 flex items-center space-x-1">
                      <Zap className="w-3 h-3 text-purple-400" />
                      <span>{t("chat.role.peer")}</span>
                    </span>
                  ) : (
                    <span className="font-semibold text-slate-300">{activeAgentName}</span>
                  )}
                  {timeStr && <span className="opacity-60">· {timeStr}</span>}
                </div>

                {/* Message Bubble */}
                <div
                  className={`rounded-2xl px-4 py-3 text-sm leading-relaxed border ${
                    isHuman
                      ? "bg-gradient-to-br from-cyan-600 to-blue-700 text-white border-cyan-400/30 rounded-tr-sm shadow-md shadow-cyan-900/20"
                      : isPeer
                      ? "bg-purple-950/40 text-purple-100 border-purple-800/50 rounded-tl-sm shadow-md"
                      : "bg-slate-900/90 text-slate-200 border-white/10 rounded-tl-sm shadow-md"
                  }`}
                >
                  <pre className="whitespace-pre-wrap font-sans text-sm break-words select-text">
                    {turn.text}
                  </pre>
                </div>
              </div>
            );
          })
        )}
        <div ref={turnsEndRef} />
      </main>

      {/* Composer Input Bar */}
      <footer className="p-4 bg-slate-900/90 backdrop-blur-xl border-t border-white/10 shrink-0">
        <div className="max-w-4xl mx-auto flex items-end space-x-2">
          {/* Stop / Cancel Button */}
          <button
            type="button"
            onClick={handleStop}
            className="p-3 rounded-xl bg-rose-500/15 hover:bg-rose-500/25 border border-rose-500/30 text-rose-300 hover:text-rose-200 transition cursor-pointer shrink-0 flex items-center justify-center"
            title={t("chat.stop")}
          >
            <Square className="w-4 h-4 fill-current" />
          </button>

          {/* Text Input */}
          <div className="flex-1 relative">
            <textarea
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              rows={1}
              placeholder={t("chat.inputPlaceholder")}
              className="w-full bg-slate-950 border border-white/15 focus:border-cyan-500 focus:ring-1 focus:ring-cyan-500 rounded-xl px-4 py-3 text-sm text-slate-100 placeholder-slate-500 resize-none outline-none transition"
              style={{ minHeight: "44px", maxHeight: "160px" }}
            />
          </div>

          {/* Send Button */}
          <button
            type="button"
            onClick={handleSend}
            disabled={!inputText.trim() || wsStatus !== "connected"}
            className="p-3 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium transition shadow-lg shadow-cyan-500/20 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer shrink-0 flex items-center justify-center"
            title={t("chat.send")}
          >
            <Send className="w-4 h-4" />
          </button>
        </div>
      </footer>
    </div>
  );
}
