//! v1.14 手机端传输：WebSocket 服务端（`Transport` trait 的实现）。
//!
//! 安全设计见 `docs/DESIGN-v1.14-transport-security.md`，要点：
//!
//! - 只绑 `127.0.0.1:<动态端口>`，手机经 Tailscale（WireGuard E2E）接入，
//!   服务端永不直接暴露在局域网明文上
//! - 32 字节 CSPRNG token 每次启动生成、不落盘（无持久凭证 → 无吊销问题）；
//!   握手校验用常量时间比较，不区分「无 token」与「token 错」
//! - `Host` 头白名单防 DNS rebinding；子协议固定 `tmuxdeck.v1`
//! - 每 IP 每 10 秒最多 5 次握手尝试（防扫描）
//! - 单帧 64 KiB、`text` 字段 8 KiB、100 帧/秒/连接、20s 心跳 60s 超时
//!
//! 上层（引擎）通过 `Transport::emit` 广播事件，通过 `commands()` 接收
//! 手机端指令流。指令的语义校验（pane 存在性、按键白名单）在上层做，
//! 本模块只做传输层的硬限制。

use crate::bridge::{ClientCommand, ClientEvent, Transport};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};

/// WebSocket 子协议，客户端必须声明。
pub const WS_SUBPROTOCOL: &str = "tmuxdeck.v1";
/// 单帧 JSON 上限。
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
/// `text` 字段上限（对齐 `tmux::MAX_SEND_TEXT_BYTES`）。
pub const MAX_TEXT_BYTES: usize = 8 * 1024;
/// 每连接帧速率上限（帧/秒）。
pub const MAX_FRAMES_PER_SEC: usize = 100;
/// 心跳间隔与超时。
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);
/// 握手限速：每 IP 每窗口最多次数。
pub const HANDSHAKE_WINDOW: Duration = Duration::from_secs(10);
pub const HANDSHAKE_MAX: usize = 5;

/// Tailscale CGNAT 网段 `100.64.0.0/10`（手机经 tailnet 接入时的对端 IP）。
fn is_tailnet_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            octets[0] == 100 && (octets[1] & 0xc0) == 0x40
        }
        IpAddr::V6(_) => false,
    }
}

/// Host 头白名单：loopback / localhost / tailnet IP / MagicDNS 域名（*.ts.net）。
fn host_allowed(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() {
        return false;
    }
    // 去掉可能的端口段
    let bare = if let Some(idx) = host.rfind(':') {
        // IPv6 用 [..] 包裹；简单起见仅对 IPv4/域名截端口
        if host.starts_with('[') {
            host
        } else {
            &host[..idx]
        }
    } else {
        host
    };
    let bare = bare.trim_matches(['[', ']']);
    let lower = bare.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return true;
    }
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return ip.is_loopback() || is_tailnet_ip(ip);
    }
    // MagicDNS：<host>.<tailnet>.ts.net
    lower.ends_with(".ts.net")
}

/// 常量时间比较，避免时序侧信道。长度不同直接返回 false（token 定长 64 hex）。
fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// 生成 32 字节 CSPRNG token（hex 64 位）。失败时退化为时间+进程熵
/// （getrandom 在主流平台不会失败，此路径仅防御性存在）。
fn generate_token() -> String {
    let mut buf = [0u8; 32];
    if getrandom::getrandom(&mut buf).is_ok() {
        return to_hex(&buf);
    }
    let fallback = format!(
        "{}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        std::thread::current().id(),
    );
    let mut out = String::new();
    for b in fallback.bytes().take(64) {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// 一个在线连接的注册表条目：下行通道 + 订阅的对话（None = 未订阅）。
pub struct ConnState {
    tx: mpsc::UnboundedSender<String>,
    subscribed: Option<String>,
}

/// WebSocket 传输实现。
///
/// - `bind()` 在 loopback 动态端口上起监听，返回传输对象与手机指令流
/// - `emit()` 按事件类型分流：`turn` 只发给订阅了该对话的连接，其余全量广播
/// - `subscribed_conversations()` 让引擎只轮询被订阅的对话（订阅粒度收窄）
/// - 每个连接一个后台任务：收帧 → 限流校验 → 指令投递到 `cmd_tx`
pub struct WsTransport {
    /// 监听地址（127.0.0.1:<动态端口>），二维码/配对用
    addr: SocketAddr,
    token: String,
    clients: Arc<Mutex<HashMap<u64, ConnState>>>,
    cmd_tx: mpsc::UnboundedSender<ClientCommand>,
}

impl WsTransport {
    /// 绑定 loopback 动态端口并开始接受连接。
    ///
    /// 返回 `(传输对象, 手机指令接收端)`。指令流由引擎消费。
    pub async fn bind() -> Result<(Self, mpsc::UnboundedReceiver<ClientCommand>), String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("ERR_WS_BIND|{}", e))?;
        let addr = listener
            .local_addr()
            .map_err(|e| format!("ERR_WS_ADDR|{}", e))?;
        let token = generate_token();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let transport = Self {
            addr,
            token: token.clone(),
            clients: Arc::new(Mutex::new(HashMap::new())),
            cmd_tx,
        };

        let clients = transport.clients.clone();
        let next_id = Arc::new(AtomicU64::new(1));
        let cmd_tx = transport.cmd_tx.clone();
        let token_cmp = token;
        tokio::spawn(async move {
            let _ = accept_loop(listener, clients, next_id, token_cmp, cmd_tx).await;
        });

        Ok((transport, cmd_rx))
    }

    /// 配对信息：监听地址与 token（供桌面端展示二维码/文本）。
    pub fn pairing(&self) -> (SocketAddr, &str) {
        (self.addr, &self.token)
    }
}

impl Transport for WsTransport {
    fn emit(&mut self, event: &ClientEvent) -> Result<(), String> {
        // 订阅过滤：turn 只发给订阅了该对话的连接；其余事件全量广播
        let only_conv = match event {
            ClientEvent::Turn { turn } => Some(turn.conversation_id.as_str()),
            _ => None,
        };
        let json = serde_json::to_string(event).map_err(|e| format!("ERR_WS_SERIALIZE|{}", e))?;
        if json.len() > MAX_FRAME_BYTES {
            return Err("ERR_WS_FRAME_TOO_LARGE".to_string());
        }
        let clients = self.clients.lock().map_err(|_| "ERR_WS_LOCK".to_string())?;
        let mut dead = Vec::new();
        for (id, conn) in clients.iter() {
            let deliver = match only_conv {
                Some(conv) => conn.subscribed.as_deref() == Some(conv),
                None => true,
            };
            if deliver && conn.tx.send(json.clone()).is_err() {
                dead.push(*id);
            }
        }
        drop(clients);
        if !dead.is_empty() {
            if let Ok(mut c) = self.clients.lock() {
                for id in dead {
                    c.remove(&id);
                }
            }
        }
        Ok(())
    }

    fn has_clients(&self) -> bool {
        self.clients
            .lock()
            .map(|c| !c.is_empty())
            .unwrap_or(false)
    }
}

impl WsTransport {
    /// 当前被订阅的对话 ID 集合（引擎据此收窄 transcript 轮询）。
    pub fn subscribed_conversations(&self) -> Vec<String> {
        self.clients
            .lock()
            .map(|c| {
                c.values()
                    .filter_map(|conn| conn.subscribed.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 更新某连接的订阅状态（由连接任务在收到 subscribe/unsubscribe 时调用）。
    fn set_subscription(conn_id: u64, subscribed: Option<String>, clients: &Arc<Mutex<HashMap<u64, ConnState>>>) {
        if let Ok(mut c) = clients.lock() {
            if let Some(conn) = c.get_mut(&conn_id) {
                conn.subscribed = subscribed;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 接受循环与单连接处理
// ─────────────────────────────────────────────────────────────────────────────

async fn accept_loop(
    listener: TcpListener,
    clients: Arc<Mutex<HashMap<u64, ConnState>>>,
    next_id: Arc<AtomicU64>,
    token: String,
    cmd_tx: mpsc::UnboundedSender<ClientCommand>,
) -> Result<(), String> {
    // 握手限速桶：IP → 最近握手时间
    let mut handshakes: HashMap<IpAddr, Vec<Instant>> = HashMap::new();

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(x) => x,
            Err(_) => continue,
        };

        // 握手限速
        let now = Instant::now();
        let recent = handshakes.entry(peer.ip()).or_default();
        recent.retain(|t| now.duration_since(*t) < HANDSHAKE_WINDOW);
        if recent.len() >= HANDSHAKE_MAX {
            // 静默丢弃，不给探测信息
            continue;
        }
        recent.push(now);

        let clients = clients.clone();
        let token = token.clone();
        let cmd_tx = cmd_tx.clone();
        let conn_id = next_id.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            let _ = handle_connection(stream, conn_id, clients, token, cmd_tx).await;
        });
    }
}

/// 单连接：握手校验 → 事件下行 + 指令上行双向循环。
async fn handle_connection(
    stream: TcpStream,
    conn_id: u64,
    clients: Arc<Mutex<HashMap<u64, ConnState>>>,
    token: String,
    cmd_tx: mpsc::UnboundedSender<ClientCommand>,
) -> Result<(), String> {
    #[allow(clippy::result_large_err)] // tungstenite 的 ErrorResponse 是 Response 类型，API 强制
    let ws = accept_hdr_async(stream, |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
        // 1. 子协议
        let proto_ok = req
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(',').any(|p| p.trim() == WS_SUBPROTOCOL))
            .unwrap_or(false);
        // 2. Host 白名单（防 DNS rebinding）
        let host_ok = req
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .map(host_allowed)
            .unwrap_or(false);
        // 3. token：query 参数，常量时间比较
        let token_ok = req
            .uri()
            .query()
            .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("token="))) 
            .map(|t| ct_eq(t, &token))
            .unwrap_or(false);
        if !(proto_ok && host_ok && token_ok) {
            // 统一拒绝，不区分失败原因（无探测信息）
            return Err(tokio_tungstenite::tungstenite::handshake::server::ErrorResponse::new(None));
        }
        let mut resp = resp;
        resp.headers_mut().insert(
            "sec-websocket-protocol",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(WS_SUBPROTOCOL),
        );
        Ok(resp)
    })
    .await
    .map_err(|e| format!("ERR_WS_HANDSHAKE|{}", e))?;

    let mut ws: WebSocketStream<TcpStream> = ws;

    // 每连接事件通道：emit 侧 send，本任务 select 下行
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<String>();
    {
        let mut guard = clients.lock().map_err(|_| "ERR_WS_LOCK")?;
        guard.insert(
            conn_id,
            ConnState {
                tx: ev_tx.clone(),
                subscribed: None,
            },
        );
    }

    let mut last_activity = Instant::now();
    let mut frames_in_sec: Vec<Instant> = Vec::new();
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // 速率限制：1 秒窗口内帧数
                        let now = Instant::now();
                        frames_in_sec.retain(|t| now.duration_since(*t) < Duration::from_secs(1));
                        if frames_in_sec.len() >= MAX_FRAMES_PER_SEC {
                            break;
                        }
                        frames_in_sec.push(now);
                        if text.len() > MAX_FRAME_BYTES {
                            break;
                        }
                        last_activity = now;
                        match parse_command(&text) {
                            Ok(cmd) => {
                                // 订阅状态就地更新（emit 过滤用），指令仍转发引擎（推快照+收窄轮询）
                                match &cmd {
                                    ClientCommand::Subscribe { id } => {
                                        WsTransport::set_subscription(
                                            conn_id,
                                            Some(id.clone()),
                                            &clients,
                                        );
                                    }
                                    ClientCommand::Unsubscribe => {
                                        WsTransport::set_subscription(conn_id, None, &clients);
                                    }
                                    _ => {}
                                }
                                let _ = cmd_tx.send(cmd);
                            }
                            Err(_) => break, // 非法指令：断开
                        }
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                        last_activity = Instant::now();
                    }
                    Some(Ok(Message::Binary(_))) => {
                        break; // 只接受文本帧
                    }
                    Some(Ok(Message::Close(_))) | Some(Ok(Message::Frame(_))) | None => break,
                    Some(Err(_)) => break,
                }
            }
            ev = ev_rx.recv() => {
                match ev {
                    Some(json) => {
                        if ws.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break, // emit 侧 channel 关闭
                }
            }
            _ = heartbeat.tick() => {
                if last_activity.elapsed() > HEARTBEAT_TIMEOUT {
                    break;
                }
                if ws.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
        }
    }

    // 清理连接
    if let Ok(mut guard) = clients.lock() {
        guard.remove(&conn_id);
    }
    Ok(())
}

/// 解析并校验手机指令的传输层硬限制（文本长度、pane 格式留上层）。
fn parse_command(text: &str) -> Result<ClientCommand, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("ERR_WS_BAD_JSON|{}", e))?;
    let cmd: ClientCommand =
        serde_json::from_value(v).map_err(|e| format!("ERR_WS_BAD_CMD|{}", e))?;
    // 文本字段上限
    let text_len = match &cmd {
        ClientCommand::Say { text, .. } | ClientCommand::Forward { text, .. } => text.len(),
        _ => 0,
    };
    if text_len > MAX_TEXT_BYTES {
        return Err("ERR_WS_TEXT_TOO_LONG".to_string());
    }
    Ok(cmd)
}

// 连接 id 分配辅助（避免闭包内借用问题，直接用 map 自增键即可）

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ct_eq() {
        assert!(ct_eq("abc", "abc"));
        assert!(!ct_eq("abc", "abd"));
        assert!(!ct_eq("abc", "abcd"));
        assert!(!ct_eq("", "a"));
    }

    #[test]
    fn test_host_allowed() {
        assert!(host_allowed("127.0.0.1"));
        assert!(host_allowed("127.0.0.1:7420"));
        assert!(host_allowed("localhost"));
        assert!(host_allowed("localhost:7420"));
        assert!(host_allowed("[::1]"));
        assert!(host_allowed("100.64.0.5"));
        assert!(host_allowed("100.101.102.103:8080"));
        assert!(host_allowed("mac.tailnet-name.ts.net"));
        assert!(!host_allowed("192.168.1.17"));
        assert!(!host_allowed("evil.example.com"));
        assert!(!host_allowed(""));
    }

    #[test]
    fn test_to_hex() {
        assert_eq!(to_hex(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(to_hex(&[0x00, 0xff]), "00ff");
    }

    #[test]
    fn test_generate_token_shape() {
        let t = generate_token();
        assert_eq!(t.len(), 64); // 32 字节 → 64 hex
        let t2 = generate_token();
        assert_ne!(t, t2, "两次生成的 token 不应相同");
    }

    #[test]
    fn test_parse_command_text_limit() {
        let ok = r#"{"type":"say","id":"%3","text":"继续"}"#;
        assert!(parse_command(ok).is_ok());

        let long = format!(r#"{{"type":"say","id":"%3","text":"{}"}}"#, "x".repeat(9000));
        assert!(parse_command(&long).is_err());

        let bad = r#"{"type":"nope"}"#;
        assert!(parse_command(bad).is_err());

        let binary = "not json at all";
        assert!(parse_command(binary).is_err());
    }

    #[test]
    fn test_is_tailnet_ip() {
        assert!(is_tailnet_ip("100.64.0.1".parse().unwrap()));
        assert!(is_tailnet_ip("100.127.255.255".parse().unwrap()));
        assert!(!is_tailnet_ip("100.63.255.255".parse().unwrap()));
        assert!(!is_tailnet_ip("192.168.1.1".parse().unwrap()));
    }
}

// ── 端到端集成测试：真实 socket 握手 + 事件流 + 订阅过滤 ──

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::bridge::ClientEvent;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    fn tokio_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn test_ws_handshake_accepts_valid_token() {
        let rt = tokio_runtime();
        rt.block_on(async {
            let (transport, _cmd_rx) = WsTransport::bind().await.unwrap();
            let (addr, token) = transport.pairing();
            let url = format!("ws://{}/v1/ws?token={}", addr, token);
            let mut req = url.into_client_request().unwrap();
            req.headers_mut().insert(
                "sec-websocket-protocol",
                tokio_tungstenite::tungstenite::http::HeaderValue::from_static(WS_SUBPROTOCOL),
            );
            let (ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
            let (mut sink, _) = ws.split();
            sink.send(Message::Text("{\"type\":\"refresh\"}".into())).await.unwrap();
            drop(transport);
        });
    }

    #[test]
    fn test_ws_handshake_rejects_bad_token() {
        let rt = tokio_runtime();
        rt.block_on(async {
            let (transport, _cmd_rx) = WsTransport::bind().await.unwrap();
            let (addr, _token) = transport.pairing();
            let url = format!("ws://{}/v1/ws?token=wrongtoken", addr);
            let req = url.into_client_request().unwrap();
            let res = tokio_tungstenite::connect_async(req).await;
            assert!(res.is_err(), "错误 token 应被拒绝");
            drop(transport);
        });
    }

    #[test]
    fn test_ws_turn_filtered_by_subscription() {
        let rt = tokio_runtime();
        rt.block_on(async {
            let (mut transport, _cmd_rx) = WsTransport::bind().await.unwrap();
            let (addr, token) = transport.pairing();
            let url = format!("ws://{}/v1/ws?token={}", addr, token);
            let mut req = url.into_client_request().unwrap();
            req.headers_mut().insert(
                "sec-websocket-protocol",
                tokio_tungstenite::tungstenite::http::HeaderValue::from_static(WS_SUBPROTOCOL),
            );
            let (ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
            let (mut sink, mut stream) = ws.split();

            // 未订阅时：turn 不应送达，status 应送达
            transport.emit(&ClientEvent::Turn {
                turn: crate::bridge::Turn {
                    conversation_id: "%3".into(),
                    role: crate::bridge::TurnRole::Agent,
                    text: "hello".into(),
                    timestamp: 1,
                },
            }).unwrap();
            transport.emit(&ClientEvent::Error { message: "x".into() }).unwrap();

            // 收到非 turn 事件（Error 全量推）
            let msg = tokio::time::timeout(Duration::from_secs(2), stream.next()).await.unwrap().unwrap().unwrap();
            assert!(msg.to_string().contains("\"type\":\"error\""));

            // 2 秒内不应收到 turn（忽略服务端心跳 Ping/Pong）
            let mut got_turn = false;
            loop {
                let r = tokio::time::timeout(Duration::from_millis(500), stream.next()).await;
                match r {
                    Ok(Some(Ok(Message::Ping(_)))) | Ok(Some(Ok(Message::Pong(_)))) => continue,
                    Ok(Some(Ok(_))) => {
                        got_turn = true;
                        break;
                    }
                    _ => break,
                }
            }
            assert!(!got_turn, "未订阅时 turn 不应送达");

            // 订阅 %3
            sink.send(Message::Text("{\"type\":\"subscribe\",\"id\":\"%3\"}".into())).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;

            // 订阅后：turn 应送达
            transport.emit(&ClientEvent::Turn {
                turn: crate::bridge::Turn {
                    conversation_id: "%3".into(),
                    role: crate::bridge::TurnRole::Agent,
                    text: "subscribed turn".into(),
                    timestamp: 2,
                },
            }).unwrap();
            let msg = tokio::time::timeout(Duration::from_secs(2), stream.next()).await.unwrap().unwrap().unwrap();
            assert!(msg.to_string().contains("subscribed turn"));

            // 另一个对话的 turn 不应送达
            transport.emit(&ClientEvent::Turn {
                turn: crate::bridge::Turn {
                    conversation_id: "%9".into(),
                    role: crate::bridge::TurnRole::Agent,
                    text: "other conv".into(),
                    timestamp: 3,
                },
            }).unwrap();
            let mut got_other = false;
            loop {
                let r = tokio::time::timeout(Duration::from_millis(500), stream.next()).await;
                match r {
                    Ok(Some(Ok(Message::Ping(_)))) | Ok(Some(Ok(Message::Pong(_)))) => continue,
                    Ok(Some(Ok(_))) => {
                        got_other = true;
                        break;
                    }
                    _ => break,
                }
            }
            assert!(!got_other, "其他对话的 turn 不应送达");

            drop(transport);
        });
    }
}
