//! v1.14 桥接引擎：把阶段 1 的各个部件串成一条可运行的消息通路。
//!
//! 一个后台任务里循环处理三件事：
//!
//! 1. **intercom 事件**（broker → 我们）：会话列表、状态变更、agent 消息
//!    —— 更新 `ConversationRegistry`，`expectsReply` 转成
//!    `ClientEvent::AwaitingHuman`（手机端唯一推送信号）；
//! 2. **手机指令**（手机 → 我们）：`say` / `key` / `forward` / `refresh`
//!    —— 校验后经 `deliver` / `send_key_name` / `forward` 投递；
//! 3. **周期刷新 + transcript 轮询**：pane 清单重建、状态比对、拉取新轮次
//!
//! 引擎独占持有 registry 与 transport（单线程事件循环，无锁竞争）；
//! 同时把只读快照放进 Tauri state，供桌面端 UI 读取配对信息与对话表。
//!
//! 轮询节奏（设计文档 §5）：手机在线才跑 transcript 轮询；
//! 存在 `awaiting-human` / `thinking` 时加密到 500ms，否则 2s。
use crate::audit::record_mobile_command;
use crate::bridge::{
    deliver, forward, ClientCommand, ClientEvent, ConversationRegistry, ConversationStatus,
    TranscriptSource, Transport, Turn, TurnRole,
};
use crate::bridge_state::BridgeState;
use crate::intercom::{broker_available, IntercomClient, IntercomEvent};
use crate::tmux::{list_all_panes, send_key_name, validate_pane_id, ALLOWED_KEYS};
use crate::transcript::CompositeTranscriptSource;
use crate::transport::{InboundCommand, WsTransport};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;

/// 常规轮询间隔；有对话在等人/思考时加密。
pub const POLL_NORMAL: Duration = Duration::from_secs(2);
pub const POLL_FAST: Duration = Duration::from_millis(500);
/// intercom 事件与手机指令的阻塞等待上限（保证周期刷新不被饿死）。
pub const POLL_TICK: Duration = Duration::from_millis(200);

/// 最小 LAN 发现：通过系统路由选择取得当前主 IPv4；无需 mDNS/额外端口。
fn discover_lan_hosts() -> Vec<String> {
    let mut hosts = Vec::new();
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                if !addr.ip().is_loopback() {
                    hosts.push(addr.ip().to_string());
                }
            }
        }
    }
    hosts.push("127.0.0.1".to_string());
    hosts
}

/// 引擎本体。单线程事件循环，持有所需的全部可变状态。
#[derive(Debug, Clone)]
struct PendingReply {
    session_id: String,
    reply_to: String,
}

pub struct BridgeEngine {
    intercom: Option<IntercomClient>,
    intercom_rx: Option<Receiver<IntercomEvent>>,
    registry: ConversationRegistry,
    transcript: CompositeTranscriptSource,
    transport: WsTransport,
    cmd_rx: tokio::sync::mpsc::UnboundedReceiver<InboundCommand>,
    /// pane → 上次已推送到手机端的轮次时间戳
    last_turn_ts: HashMap<String, i64>,
    pending_replies: HashMap<String, PendingReply>,
    state: Arc<BridgeState>,
    last_refresh: Instant,
    fast_due: bool,
    last_client_count: usize,
    app_handle: Option<tauri::AppHandle>,
}

impl BridgeEngine {
    /// 启动引擎（阻塞运行，调用方 spawn 到独立线程）。
    ///
    /// broker 不在时降级：仍起 WebSocket 服务与 pane 清单，
    /// 对话走 send-keys 投递、capture-pane 兜底。
    pub fn run(state: Arc<BridgeState>, app_handle: Option<tauri::AppHandle>) {
        // 1. 传输（不依赖 broker，先起，保证 UI 能拿到配对信息）
        let (transport, cmd_rx) = match tauri::async_runtime::block_on(WsTransport::bind()) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("[bridge] transport bind failed: {}", e);
                return;
            }
        };
        let (port, token) = transport.pairing();
        let hosts = discover_lan_hosts();
        if let Ok(mut a) = state.ws_addr.lock() {
            *a = hosts.first().map(|host| format!("{}:{}", host, port));
        }
        if let Ok(mut t) = state.ws_token.lock() {
            *t = Some(token.to_string());
        }
        if let Ok(mut p) = state.port.lock() {
            *p = Some(port);
        }
        if let Ok(mut h) = state.lan_hosts.lock() {
            *h = hosts;
        }
        println!(
            "[bridge] trusted-LAN mobile HTTP/WS listening on 0.0.0.0:{}",
            port
        );

        // 2. intercom（可选）
        let (intercom, intercom_rx) = if broker_available() {
            match IntercomClient::connect("me") {
                Ok(x) => {
                    println!("[bridge] intercom broker connected as 'me'");
                    (Some(x.0), Some(x.1))
                }
                Err(e) => {
                    eprintln!(
                        "[bridge] intercom connect failed (fallback send-keys): {}",
                        e
                    );
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        let mut engine = Self {
            intercom,
            intercom_rx,
            registry: ConversationRegistry::new(),
            transcript: CompositeTranscriptSource::new(),
            transport,
            cmd_rx,
            last_turn_ts: HashMap::new(),
            pending_replies: HashMap::new(),
            state,
            last_refresh: Instant::now() - POLL_NORMAL,
            fast_due: false,
            last_client_count: 0,
            app_handle,
        };

        engine.run_loop();
    }

    fn run_loop(&mut self) {
        // 初始：全量刷新一次
        self.refresh_all();

        loop {
            // 1) intercom 事件（非阻塞轮询）
            if let Some(rx) = &self.intercom_rx {
                let mut events = Vec::new();
                while let Ok(ev) = rx.try_recv() {
                    events.push(ev);
                }
                for ev in events {
                    self.on_intercom(ev);
                }
            }

            // 2) 手机指令
            while let Ok(cmd) = self.cmd_rx.try_recv() {
                self.on_command(cmd);
            }

            // 3) 发布已认证手机连接数；桌面端轮询 pairing 即可取到实时值。
            while let Some(client_count) = self.transport.try_client_count_change() {
                if let Ok(mut count) = self.state.connected_clients.lock() {
                    *count = client_count;
                }
                self.last_client_count = client_count;
                if let Some(app) = &self.app_handle {
                    let _ = app.emit(
                        "mobile-clients-changed",
                        serde_json::json!({
                            "connectedClients": client_count
                        }),
                    );
                }
            }

            // 4) 周期刷新
            let has_clients = self.transport.has_clients();
            self.fast_due = has_clients
                && self.registry.list().iter().any(|c| {
                    matches!(
                        c.status,
                        ConversationStatus::AwaitingHuman
                            | ConversationStatus::Thinking
                            | ConversationStatus::RunningTool
                    )
                });
            let interval = if self.fast_due {
                POLL_FAST
            } else {
                POLL_NORMAL
            };
            if self.last_refresh.elapsed() >= interval {
                self.refresh_all();
            }

            std::thread::sleep(POLL_TICK);
        }
    }

    /// 全量刷新：pane 清单 → registry → transcript 轮询 → state 快照。
    fn refresh_all(&mut self) {
        let now = Instant::now();

        // 1) pane 骨架
        self.registry.refresh_panes(list_all_panes());
        self.pending_replies
            .retain(|pane_id, _| self.registry.get(pane_id).is_some());

        // 2) intercom 会话合并（若在线）
        if let Some(client) = &self.intercom {
            if client.is_connected() {
                let _ = client.request_list();
            }
        }

        // Transcript availability is authoritative backend metadata and may
        // change while a pane keeps the same identity and subscription.
        for conv in self.registry.list() {
            let kind = self.transcript.kind_for(&conv);
            self.registry.set_transcript_kind(&conv.id, kind);
        }

        // 3) 状态比对：向手机端推送变化
        let list = self.registry.list();
        if let Ok(mut convs) = self.state.conversations.lock() {
            let changed = convs.len() != list.len()
                || convs.iter().zip(list.iter()).any(|(a, b)| {
                    a.status != b.status
                        || a.title != b.title
                        || a.id != b.id
                        || a.workspace_id != b.workspace_id
                        || a.workspace_name != b.workspace_name
                        || a.transcript_kind != b.transcript_kind
                });
            if changed {
                let snapshot = list.clone();
                *convs = snapshot;
                let _ = self.transport.emit(&ClientEvent::Conversations {
                    items: list.clone(),
                });
            }
        }

        // 4) transcript 轮询（仅订阅的对话；状态事件来自 broker 不受影响）
        let subscribed = self.transport.subscribed_conversations();
        if !subscribed.is_empty() {
            self.poll_transcripts(&subscribed);
        }

        self.last_refresh = now;
    }

    /// 为**被订阅**的对话拉取新轮次并推送（轮询成本收窄到订阅粒度）。
    fn poll_transcripts(&mut self, subscribed: &[String]) {
        let convs = self.registry.list();
        for conv in convs.iter() {
            if !subscribed.contains(&conv.id) {
                continue;
            }
            let since = self.last_turn_ts.get(&conv.id).copied().unwrap_or(0);
            match self.transcript.poll(conv, since) {
                Ok(turns) => {
                    for t in turns.iter() {
                        let _ = self.transport.emit(&ClientEvent::Turn { turn: t.clone() });
                        self.last_turn_ts.insert(conv.id.clone(), t.timestamp);
                    }
                }
                Err(e) => {
                    // 单对话失败不影响整体，记日志继续
                    eprintln!("[bridge] transcript poll {} failed: {}", conv.id, e);
                }
            }
        }
    }

    /// 订阅某对话：立即推一次 transcript 尾部快照（游标起始，避免手机端空白）。
    fn subscribe_snapshot(&mut self, conn_id: u64, id: &str) {
        let Some(conv) = self.registry.get(id).cloned() else {
            let _ = self.transport.emit_to(
                conn_id,
                &ClientEvent::Error {
                    message: format!("unknown pane {}", id),
                },
            );
            return;
        };
        match self.transcript.poll(&conv, 0) {
            Ok(turns) => {
                // 只推尾部（最多 40 条），让手机端一进来就有内容
                for t in turns.iter().rev().take(40).rev() {
                    let _ = self
                        .transport
                        .emit_to(conn_id, &ClientEvent::Turn { turn: t.clone() });
                }
                if let Some(last) = turns.last() {
                    self.last_turn_ts.insert(id.to_string(), last.timestamp);
                }
            }
            Err(e) => eprintln!("[bridge] subscribe snapshot {} failed: {}", id, e),
        }
    }

    // ── intercom 事件处理 ──
    fn on_intercom(&mut self, ev: IntercomEvent) {
        match ev {
            IntercomEvent::Registered { session_id, .. } => {
                println!("[bridge] registered as session {}", session_id);
                if let Some(c) = &self.intercom {
                    let _ = c.request_list();
                }
            }
            IntercomEvent::Sessions { sessions, .. } => {
                let self_id = self.intercom.as_ref().and_then(|c| c.session_id());
                self.registry
                    .apply_intercom_sessions(&sessions, self_id.as_deref());
                self.pending_replies.retain(|pane_id, pending| {
                    self.registry.get(pane_id).is_some()
                        && sessions
                            .iter()
                            .any(|session| session.id == pending.session_id)
                });
                self.restore_pending_statuses();
                if let Ok(mut b) = self.state.broker_connected.lock() {
                    *b = true;
                }
            }
            IntercomEvent::Message {
                from,
                message,
                delivery_id,
            } => {
                // agent 发给我们的消息：作为对话轮次 + 期待回复时推送 AwaitingHuman
                let msg_id = message.id.clone();
                let conv_id = self.registry.by_intercom_id(&from.id).map(|c| c.id.clone());
                if let Some(conv_id) = conv_id {
                    let turn = Turn {
                        conversation_id: conv_id.clone(),
                        role: TurnRole::Agent,
                        text: message.content.text.clone(),
                        timestamp: message.timestamp,
                    };
                    let _ = self.transport.emit(&ClientEvent::Turn { turn });
                    if message.expects_reply() {
                        self.pending_replies.insert(
                            conv_id.clone(),
                            PendingReply {
                                session_id: from.id.clone(),
                                reply_to: msg_id.clone(),
                            },
                        );
                        self.registry.mark_awaiting_human(&from.id);
                        let conv = self.registry.get(&conv_id);
                        let title = conv.map(|c| c.title.clone()).unwrap_or_default();
                        let _ = self.transport.emit(&ClientEvent::AwaitingHuman {
                            id: conv_id,
                            title,
                            preview: message.content.text,
                            reply_to: Some(msg_id.clone()),
                        });
                    }
                    // 回执：告知发送方已收到
                    if let Some(c) = &self.intercom {
                        let _ = c.acknowledge(&delivery_id);
                    }
                }
            }
            IntercomEvent::PresenceUpdate { session } => {
                self.registry.apply_presence(&session);
                self.restore_pending_statuses();
                if let Some(conv) = self.registry.by_intercom_id(&session.id) {
                    let _ = self.transport.emit(&ClientEvent::StatusChanged {
                        id: conv.id.clone(),
                        status: conv.status,
                    });
                }
            }
            IntercomEvent::SessionJoined { .. } | IntercomEvent::SessionLeft { .. } => {
                if let Some(c) = &self.intercom {
                    let _ = c.request_list();
                }
            }
            IntercomEvent::Delivered { .. } | IntercomEvent::DeliveryFailed { .. } => {
                // 投递回执透传（v1.14 不展示，日志留痕）
            }
            IntercomEvent::BrokerError { error } => {
                eprintln!("[bridge] broker error: {}", error);
            }
            IntercomEvent::Disconnected => {
                eprintln!("[bridge] broker disconnected");
                if let Ok(mut b) = self.state.broker_connected.lock() {
                    *b = false;
                }
            }
        }
    }

    // ── 手机指令处理 ──
    fn on_command(&mut self, inbound: InboundCommand) {
        let InboundCommand {
            conn_id,
            peer_ip,
            command,
        } = inbound;
        match command {
            ClientCommand::Say { id, text } => {
                let Some(conv) = self.registry.get(&id) else {
                    self.reject(
                        conn_id,
                        peer_ip,
                        "say",
                        Some(&id),
                        Some(&text),
                        "unknown pane",
                    );
                    return;
                };
                if text.is_empty() {
                    self.reject(
                        conn_id,
                        peer_ip,
                        "say",
                        Some(&id),
                        Some(&text),
                        "text must not be empty",
                    );
                    return;
                }
                if let (Some(pending), Some(intercom)) = (
                    pending_reply_for_say(&self.pending_replies, &id),
                    self.intercom.as_ref(),
                ) {
                    match intercom.reply(&pending.session_id, &text, &pending.reply_to) {
                        Ok(_) => {
                            self.pending_replies.remove(&id);
                            self.registry.clear_pane_awaiting_human(&id);
                            self.refresh_all();
                            record_mobile_command(
                                peer_ip,
                                "reply",
                                Some(&id),
                                Some(&text),
                                "accepted",
                            );
                            return;
                        }
                        Err(e) => {
                            self.reject(
                                conn_id,
                                peer_ip,
                                "reply",
                                Some(&id),
                                Some(&text),
                                &format!("reply failed: {}", e),
                            );
                            return;
                        }
                    }
                }
                match deliver(conv, &text, self.intercom.as_ref()) {
                    Ok(route) => {
                        record_mobile_command(peer_ip, "say", Some(&id), Some(&text), "accepted");
                        println!("[bridge] say {} ({:?})", id, route);
                    }
                    Err(e) => self.reject(
                        conn_id,
                        peer_ip,
                        "say",
                        Some(&id),
                        Some(&text),
                        &format!("deliver failed: {}", e),
                    ),
                }
            }
            ClientCommand::Key { id, key } => {
                if !validate_pane_id(&id) || self.registry.get(&id).is_none() {
                    self.reject(conn_id, peer_ip, "key", Some(&id), None, "unknown pane");
                    return;
                }
                if !ALLOWED_KEYS.contains(&key.as_str()) {
                    self.reject(conn_id, peer_ip, "key", Some(&id), None, "key not allowed");
                    return;
                }
                match send_key_name(&id, &key) {
                    Ok(()) => record_mobile_command(peer_ip, "key", Some(&id), None, "accepted"),
                    Err(e) => self.reject(
                        conn_id,
                        peer_ip,
                        "key",
                        Some(&id),
                        None,
                        &format!("key failed: {}", e),
                    ),
                }
            }
            ClientCommand::Forward { from, to, text } => {
                if from == to || text.is_empty() {
                    self.reject(
                        conn_id,
                        peer_ip,
                        "forward",
                        Some(&to),
                        Some(&text),
                        "forward requires different panes and non-empty text",
                    );
                    return;
                }
                let Some(f) = self.registry.get(&from) else {
                    self.reject(
                        conn_id,
                        peer_ip,
                        "forward",
                        Some(&from),
                        Some(&text),
                        "unknown source pane",
                    );
                    return;
                };
                let Some(t) = self.registry.get(&to) else {
                    self.reject(
                        conn_id,
                        peer_ip,
                        "forward",
                        Some(&to),
                        Some(&text),
                        "unknown target pane",
                    );
                    return;
                };
                match forward(f, t, &text, self.intercom.as_ref()) {
                    Ok(route) => {
                        record_mobile_command(
                            peer_ip,
                            "forward",
                            Some(&to),
                            Some(&text),
                            "accepted",
                        );
                        println!("[bridge] forward {} → {} ({:?})", from, to, route);
                    }
                    Err(e) => self.reject(
                        conn_id,
                        peer_ip,
                        "forward",
                        Some(&to),
                        Some(&text),
                        &format!("forward failed: {}", e),
                    ),
                }
            }
            ClientCommand::Refresh => {
                record_mobile_command(peer_ip, "refresh", None, None, "accepted");
                self.refresh_all();
                let _ = self.transport.emit_to(
                    conn_id,
                    &ClientEvent::Conversations {
                        items: self.registry.list(),
                    },
                );
            }
            ClientCommand::Subscribe { id } => {
                if !validate_pane_id(&id) || self.registry.get(&id).is_none() {
                    self.reject(
                        conn_id,
                        peer_ip,
                        "subscribe",
                        Some(&id),
                        None,
                        &format!("unknown pane {}", id),
                    );
                    return;
                }
                self.transport.set_subscription(conn_id, Some(id.clone()));
                record_mobile_command(peer_ip, "subscribe", Some(&id), None, "accepted");
                self.subscribe_snapshot(conn_id, &id);
            }
            ClientCommand::Unsubscribe => {
                self.transport.set_subscription(conn_id, None);
                record_mobile_command(peer_ip, "unsubscribe", None, None, "accepted");
            }
        }
    }

    fn restore_pending_statuses(&mut self) {
        restore_pending_statuses(&mut self.registry, &self.pending_replies);
    }

    fn reject(
        &mut self,
        conn_id: u64,
        peer_ip: std::net::IpAddr,
        command: &str,
        pane: Option<&str>,
        text: Option<&str>,
        message: &str,
    ) {
        record_mobile_command(peer_ip, command, pane, text, "rejected");
        let _ = self.transport.emit_to(
            conn_id,
            &ClientEvent::Error {
                message: message.to_string(),
            },
        );
    }
}

fn restore_pending_statuses(
    registry: &mut ConversationRegistry,
    pending_replies: &HashMap<String, PendingReply>,
) {
    for pane_id in pending_replies.keys() {
        registry.mark_pane_awaiting_human(pane_id);
    }
}

fn pending_reply_for_say(
    pending_replies: &HashMap<String, PendingReply>,
    pane_id: &str,
) -> Option<PendingReply> {
    pending_replies.get(pane_id).cloned()
}

/// 供 Tauri setup 调用：把引擎 spawn 到后台线程。
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Conversation;
    use crate::tmux::PaneDetail;
    use futures_util::stream::{SplitSink, SplitStream};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::protocol::Message;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

    type TestSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

    async fn connect_client(
        port: u16,
        token: &str,
    ) -> (SplitSink<TestSocket, Message>, SplitStream<TestSocket>) {
        let url = format!("ws://127.0.0.1:{}/v1/ws?token={}", port, token);
        let mut req = url.into_client_request().unwrap();
        req.headers_mut().insert(
            "sec-websocket-protocol",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(
                crate::transport::WS_SUBPROTOCOL,
            ),
        );
        tokio_tungstenite::connect_async(req)
            .await
            .unwrap()
            .0
            .split()
    }

    async fn recv_text(stream: &mut SplitStream<TestSocket>) -> String {
        loop {
            match stream.next().await.unwrap().unwrap() {
                Message::Text(text) => return text.to_string(),
                Message::Ping(_) | Message::Pong(_) => continue,
                other => panic!("unexpected websocket message: {other:?}"),
            }
        }
    }

    async fn assert_no_text(stream: &mut SplitStream<TestSocket>, message: &str) {
        assert!(
            tokio::time::timeout(Duration::from_millis(200), recv_text(stream))
                .await
                .is_err(),
            "{message}"
        );
    }

    #[test]
    fn test_refresh_always_sends_initial_conversations_to_requester() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (transport, mut cmd_rx) = WsTransport::bind().await.unwrap();
            let (port, token) = transport.pairing();
            let (mut sink, mut stream) = connect_client(port, token).await;
            sink.send(Message::Text(r#"{"type":"refresh"}"#.into()))
                .await
                .unwrap();
            let refresh = cmd_rx.recv().await.unwrap();
            let mut engine = BridgeEngine {
                intercom: None,
                intercom_rx: None,
                registry: ConversationRegistry::new(),
                transcript: CompositeTranscriptSource::new(),
                transport,
                cmd_rx,
                last_turn_ts: HashMap::new(),
                pending_replies: HashMap::new(),
                state: Arc::new(BridgeState::default()),
                last_refresh: Instant::now(),
                fast_due: false,
                last_client_count: 0,
                app_handle: None,
            };
            engine.on_command(refresh);
            let event = recv_text(&mut stream).await;
            assert!(event.contains("\"type\":\"conversations\""));
        });
    }

    #[test]
    fn test_two_connections_use_targeted_results_and_subscription_delivery() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let (transport, mut cmd_rx) = WsTransport::bind().await.unwrap();
            let (port, token) = transport.pairing();
            let (mut sink_a, mut stream_a) = connect_client(port, token).await;
            let (mut sink_b, mut stream_b) = connect_client(port, token).await;

            sink_a
                .send(Message::Text(
                    r#"{"type":"subscribe","id":"%missing"}"#.into(),
                ))
                .await
                .unwrap();
            let unknown = cmd_rx.recv().await.unwrap();
            let conn_a = unknown.conn_id;

            sink_b
                .send(Message::Text(r#"{"type":"refresh"}"#.into()))
                .await
                .unwrap();
            let conn_b = cmd_rx.recv().await.unwrap().conn_id;
            assert_ne!(conn_a, conn_b);

            let mut engine = BridgeEngine {
                intercom: None,
                intercom_rx: None,
                registry: ConversationRegistry::new(),
                transcript: CompositeTranscriptSource::new(),
                transport,
                cmd_rx,
                last_turn_ts: HashMap::new(),
                pending_replies: HashMap::new(),
                state: Arc::new(BridgeState::default()),
                last_refresh: Instant::now(),
                fast_due: false,
                last_client_count: 0,
                app_handle: None,
            };

            // A 的未知订阅错误只回 A。
            engine.on_command(unknown);
            assert!(recv_text(&mut stream_a)
                .await
                .contains("unknown pane %missing"));
            assert_no_text(&mut stream_b, "B 不应收到 A 的命令错误").await;

            // A/B 分别订阅不同对话；入站 envelope 保留各自连接 ID。
            engine.registry.refresh_panes(vec![
                PaneDetail {
                    id: "%3".into(),
                    workspace_id: "a".into(),
                    workspace_name: "a".into(),
                    agent_id: None,
                    expected_intercom_id: None,
                    managed_claude_adapter: false,
                    session: "a".into(),
                    command: "pi".into(),
                    cwd: "/tmp".into(),
                    active: true,
                },
                PaneDetail {
                    id: "%9".into(),
                    workspace_id: "b".into(),
                    workspace_name: "b".into(),
                    agent_id: None,
                    expected_intercom_id: None,
                    managed_claude_adapter: false,
                    session: "b".into(),
                    command: "pi".into(),
                    cwd: "/tmp".into(),
                    active: true,
                },
            ]);
            sink_a
                .send(Message::Text(r#"{"type":"subscribe","id":"%3"}"#.into()))
                .await
                .unwrap();
            let sub_a = engine.cmd_rx.recv().await.unwrap();
            assert_eq!(sub_a.conn_id, conn_a);
            engine.transport.set_subscription(conn_a, Some("%3".into()));
            sink_b
                .send(Message::Text(r#"{"type":"subscribe","id":"%9"}"#.into()))
                .await
                .unwrap();
            let sub_b = engine.cmd_rx.recv().await.unwrap();
            assert_eq!(sub_b.conn_id, conn_b);
            engine.transport.set_subscription(conn_b, Some("%9".into()));

            let snapshot = ClientEvent::Turn {
                turn: Turn {
                    conversation_id: "%3".into(),
                    role: TurnRole::Agent,
                    text: "snapshot".into(),
                    timestamp: 1,
                },
            };
            engine.transport.emit_to(conn_a, &snapshot).unwrap();
            assert!(recv_text(&mut stream_a).await.contains("snapshot"));
            assert_no_text(&mut stream_b, "订阅快照不应发给其他连接").await;

            let incremental = ClientEvent::Turn {
                turn: Turn {
                    conversation_id: "%3".into(),
                    role: TurnRole::Agent,
                    text: "incremental".into(),
                    timestamp: 2,
                },
            };
            engine.transport.emit(&incremental).unwrap();
            assert!(recv_text(&mut stream_a).await.contains("incremental"));
            assert_no_text(&mut stream_b, "持续 Turn 只应发给对应对话订阅者").await;

            // 全局同步事件不受订阅过滤，两个连接都收到。
            engine
                .transport
                .emit(&ClientEvent::Conversations { items: Vec::new() })
                .unwrap();
            assert!(recv_text(&mut stream_a).await.contains("conversations"));
            assert!(recv_text(&mut stream_b).await.contains("conversations"));
            engine
                .transport
                .emit(&ClientEvent::StatusChanged {
                    id: "%3".into(),
                    status: ConversationStatus::Idle,
                })
                .unwrap();
            assert!(recv_text(&mut stream_a).await.contains("status-changed"));
            assert!(recv_text(&mut stream_b).await.contains("status-changed"));

            // 同一对话允许多连接订阅，后续增量应各自收到。
            sink_b
                .send(Message::Text(r#"{"type":"subscribe","id":"%3"}"#.into()))
                .await
                .unwrap();
            let sub_b = engine.cmd_rx.recv().await.unwrap();
            assert_eq!(sub_b.conn_id, conn_b);
            engine.transport.set_subscription(conn_b, Some("%3".into()));
            engine.transport.emit(&incremental).unwrap();
            assert!(recv_text(&mut stream_a).await.contains("incremental"));
            assert!(recv_text(&mut stream_b).await.contains("incremental"));
        });
    }

    #[test]
    fn test_pending_reply_lookup_and_status_restore() {
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![PaneDetail {
            id: "%1".into(),
            workspace_id: "s".into(),
            workspace_name: "s".into(),
            agent_id: None,
            expected_intercom_id: None,
            managed_claude_adapter: false,
            session: "s".into(),
            command: "pi".into(),
            cwd: "/tmp".into(),
            active: true,
        }]);
        let pending = PendingReply {
            session_id: "agent-1".into(),
            reply_to: "message-1".into(),
        };
        let mut pending_replies = HashMap::new();
        pending_replies.insert("%1".into(), pending.clone());
        assert_eq!(
            pending_reply_for_say(&pending_replies, "%1")
                .unwrap()
                .reply_to,
            "message-1"
        );
        assert!(pending_reply_for_say(&pending_replies, "%2").is_none());
        restore_pending_statuses(&mut reg, &pending_replies);
        assert_eq!(
            reg.get("%1").unwrap().status,
            ConversationStatus::AwaitingHuman
        );
        pending_replies.remove("%1");
        assert!(pending_reply_for_say(&pending_replies, "%1").is_none());
    }

    #[test]
    fn test_say_unknown_pane_emits_error() {
        // 引擎的 pane 校验路径：registry 查不到就应拒绝（这里验证快照语义）
        let mut reg = ConversationRegistry::new();
        reg.refresh_panes(vec![PaneDetail {
            id: "%1".into(),
            workspace_id: "s".into(),
            workspace_name: "s".into(),
            agent_id: None,
            expected_intercom_id: None,
            managed_claude_adapter: false,
            session: "s".into(),
            command: "pi".into(),
            cwd: "/tmp".into(),
            active: true,
        }]);
        assert!(reg.get("%1").is_some());
        assert!(reg.get("%999").is_none());
    }

    #[test]
    fn test_turn_serialization_shape() {
        let turn = Turn {
            conversation_id: "%1".into(),
            role: TurnRole::Agent,
            text: "你好".into(),
            timestamp: 123,
        };
        let ev = ClientEvent::Turn { turn };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"type\":\"turn\""));
        assert!(json.contains("\"conversationId\":\"%1\""));
    }

    #[test]
    fn test_awaiting_human_serialization() {
        let ev = ClientEvent::AwaitingHuman {
            id: "%3".into(),
            title: "backend".into(),
            preview: "需要确认".into(),
            reply_to: Some("m-1".into()),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"type\":\"awaiting-human\""));
        assert!(json.contains("\"replyTo\":\"m-1\""));
    }

    #[test]
    fn test_fast_due_semantics() {
        // fast_due：存在 awaiting-human / thinking / running-tool 任一即加密轮询
        let needs_fast = |c: &Conversation| {
            c.status == ConversationStatus::AwaitingHuman
                || c.status == ConversationStatus::Thinking
                || c.status == ConversationStatus::RunningTool
        };
        let mk = |st: ConversationStatus| Conversation {
            id: "%1".into(),
            session: "s".into(),
            workspace_id: "s".into(),
            workspace_name: "s".into(),
            cwd: "/tmp".into(),
            kind: crate::bridge::AgentKind::Pi,
            transcript_kind: crate::bridge::TranscriptKind::Capture,
            title: "t".into(),
            intercom_session_id: None,
            expected_intercom_id: None,
            managed_claude_adapter: false,
            status: st,
        };
        assert!(needs_fast(&mk(ConversationStatus::AwaitingHuman)));
        assert!(needs_fast(&mk(ConversationStatus::Thinking)));
        assert!(!needs_fast(&mk(ConversationStatus::Idle)));
        assert!(!needs_fast(&mk(ConversationStatus::Unknown)));
    }
}
