//! v1.14 WebSocket 连接处理：接受循环 + 单连接双向循环 + 指令解析。
//!
//! 与 `transport.rs` 分离的原因：保持每个文件实现部分 ≤400 行。
//! 本模块是纯传输层——握手校验、限流、心跳、帧限制、订阅状态更新，
//! 不涉及对话语义（那是引擎的事）。

use crate::bridge::ClientCommand;
use crate::transport::{
    ct_eq, host_allowed, trusted_client_ip, ConnState, InboundCommand, MAX_FRAME_BYTES,
    MAX_TEXT_BYTES,
};
use crate::transport::{HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT, MAX_FRAMES_PER_SEC, WS_SUBPROTOCOL};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};

const MOBILE_HTML: &str = include_str!("../mobile/index.html");
const MAX_HTTP_REQUEST_BYTES: usize = 8 * 1024;

/// 接受循环：监听 → 握手限速 → 每连接一个任务。
pub(crate) async fn accept_loop(
    listener: TcpListener,
    clients: Arc<Mutex<HashMap<u64, ConnState>>>,
    next_id: Arc<AtomicU64>,
    token: String,
    cmd_tx: mpsc::UnboundedSender<InboundCommand>,
    client_count_tx: mpsc::UnboundedSender<usize>,
) -> Result<(), String> {
    // 握手限速桶：IP → 最近握手时间
    let mut handshakes: HashMap<IpAddr, Vec<Instant>> = HashMap::new();

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(x) => x,
            Err(_) => continue,
        };

        // 服务暴露在所有网卡，但只接受本机/可信 LAN/预留 tailnet 来源。
        if !trusted_client_ip(peer.ip()) {
            continue;
        }

        // 握手限速
        let now = Instant::now();
        let recent = handshakes.entry(peer.ip()).or_default();
        recent.retain(|t| now.duration_since(*t) < crate::transport::HANDSHAKE_WINDOW);
        if recent.len() >= crate::transport::HANDSHAKE_MAX {
            continue; // 静默丢弃，不给探测信息
        }
        recent.push(now);

        let clients = clients.clone();
        let token = token.clone();
        let cmd_tx = cmd_tx.clone();
        let client_count_tx = client_count_tx.clone();
        let conn_id = next_id.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            let _ = route_connection(
                stream,
                peer.ip(),
                conn_id,
                clients,
                token,
                cmd_tx,
                client_count_tx,
            )
            .await;
        });
    }
}

/// 单端口 HTTP/WS 路由。HTTP 只提供 token 保护的单文件移动页。
async fn route_connection(
    mut stream: TcpStream,
    peer_ip: IpAddr,
    conn_id: u64,
    clients: Arc<Mutex<HashMap<u64, ConnState>>>,
    token: String,
    cmd_tx: mpsc::UnboundedSender<InboundCommand>,
    client_count_tx: mpsc::UnboundedSender<usize>,
) -> Result<(), String> {
    let preview = peek_http_head(&stream).await?;
    if is_websocket_upgrade(&preview) {
        return handle_websocket(
            stream,
            peer_ip,
            conn_id,
            clients,
            token,
            cmd_tx,
            client_count_tx,
        )
        .await;
    }
    let head = read_http_head(&mut stream).await?;
    serve_http(&mut stream, &head, &token).await
}

/// 等待完整请求头但不消费字节，确保后续 tungstenite 能读取完整 WS 握手。
async fn peek_http_head(stream: &TcpStream) -> Result<String, String> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut request = [0u8; MAX_HTTP_REQUEST_BYTES];
        loop {
            let read = stream
                .peek(&mut request)
                .await
                .map_err(|e| format!("ERR_HTTP_PEEK|{}", e))?;
            if read == 0 {
                return Err("ERR_HTTP_EOF".to_string());
            }
            if request[..read]
                .windows(4)
                .any(|window| window == b"\r\n\r\n")
            {
                return String::from_utf8(request[..read].to_vec())
                    .map_err(|_| "ERR_HTTP_UTF8".to_string());
            }
            if read == MAX_HTTP_REQUEST_BYTES {
                return Err("ERR_HTTP_HEADER_TOO_LARGE".to_string());
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .map_err(|_| "ERR_HTTP_HEADER_TIMEOUT".to_string())?
}

async fn read_http_head(stream: &mut TcpStream) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 1024];
    while bytes.len() < MAX_HTTP_REQUEST_BYTES {
        let remaining = MAX_HTTP_REQUEST_BYTES - bytes.len();
        let chunk_len = remaining.min(chunk.len());
        let read = stream
            .read(&mut chunk[..chunk_len])
            .await
            .map_err(|e| format!("ERR_HTTP_READ|{}", e))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return String::from_utf8(bytes).map_err(|_| "ERR_HTTP_UTF8".to_string());
        }
    }
    Err("ERR_HTTP_HEADER_TOO_LARGE".to_string())
}

fn is_websocket_upgrade(request: &str) -> bool {
    request.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("upgrade") && value.trim().eq_ignore_ascii_case("websocket")
        })
    })
}

async fn serve_http(stream: &mut TcpStream, request: &str, token: &str) -> Result<(), String> {
    let host_ok = request
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("host"))
        .is_some_and(|(_, value)| host_allowed(value.trim()));
    if !host_ok {
        return write_http(
            stream,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            "forbidden",
            &[],
        )
        .await;
    }
    let first = request.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    if method != "GET" {
        return write_http(
            stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "method not allowed",
            &[],
        )
        .await;
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let token_ok = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("token="))
        .is_some_and(|candidate| ct_eq(candidate, token));
    if !token_ok {
        return write_http(
            stream,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            "forbidden",
            &[],
        )
        .await;
    }
    match path {
        "/v1" => {
            let location = format!("/v1/?{}", query);
            write_http(stream, "302 Found", "text/plain; charset=utf-8", "redirecting", &[("Location", location.as_str())]).await
        }
        "/v1/" => write_http(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            MOBILE_HTML,
            &[("Cache-Control", "no-store"), ("X-Content-Type-Options", "nosniff"), ("Content-Security-Policy", "default-src 'self' 'unsafe-inline'; connect-src ws: wss:; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")],
        ).await,
        _ => write_http(stream, "404 Not Found", "text/plain; charset=utf-8", "not found", &[]).await,
    }
}

async fn write_http(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> Result<(), String> {
    let mut response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        status,
        content_type,
        body.len()
    );
    for (name, value) in headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    response.push_str(body);
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|e| format!("ERR_HTTP_WRITE|{}", e))
}

/// 单 WebSocket 连接：握手校验 → 事件下行 + 指令上行双向循环。
async fn handle_websocket(
    stream: TcpStream,
    peer_ip: IpAddr,
    conn_id: u64,
    clients: Arc<Mutex<HashMap<u64, ConnState>>>,
    token: String,
    cmd_tx: mpsc::UnboundedSender<InboundCommand>,
    client_count_tx: mpsc::UnboundedSender<usize>,
) -> Result<(), String> {
    #[allow(clippy::result_large_err)] // tungstenite 的 ErrorResponse 是 Response 类型，API 强制
    let ws = accept_hdr_async(
        stream,
        |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
         resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
            // 1. 固定路径 + 子协议
            let path_ok = req.uri().path() == "/v1/ws";
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
                .map(|t| crate::transport::ct_eq(t, &token))
                .unwrap_or(false);
            if !(path_ok && proto_ok && host_ok && token_ok) {
                // 统一拒绝，不区分失败原因（无探测信息）
                return Err(
                    tokio_tungstenite::tungstenite::handshake::server::ErrorResponse::new(None),
                );
            }
            let mut resp = resp;
            resp.headers_mut().insert(
                "sec-websocket-protocol",
                tokio_tungstenite::tungstenite::http::HeaderValue::from_static(WS_SUBPROTOCOL),
            );
            Ok(resp)
        },
    )
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
        let _ = client_count_tx.send(guard.len());
    }

    let mut last_activity = Instant::now();
    let mut frames_in_sec: Vec<Instant> = Vec::new();
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::protocol::Message::Text(text))) => {
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
                                // pane 是否存在由引擎校验；校验通过后才改变订阅。
                                let _ = cmd_tx.send(InboundCommand {
                                    conn_id,
                                    peer_ip,
                                    command: cmd,
                                });
                            }
                            Err(_) => break, // 非法指令：断开
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::protocol::Message::Ping(_)))
                    | Some(Ok(tokio_tungstenite::tungstenite::protocol::Message::Pong(_))) => {
                        last_activity = Instant::now();
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::protocol::Message::Binary(_))) => {
                        break; // 只接受文本帧
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::protocol::Message::Close(_)))
                    | Some(Ok(tokio_tungstenite::tungstenite::protocol::Message::Frame(_)))
                    | None => break,
                    Some(Err(_)) => break,
                }
            }
            ev = ev_rx.recv() => {
                match ev {
                    Some(json) => {
                        if ws
                            .send(tokio_tungstenite::tungstenite::protocol::Message::Text(
                                json.into(),
                            ))
                            .await
                            .is_err()
                        {
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
                if ws
                    .send(tokio_tungstenite::tungstenite::protocol::Message::Ping(vec![].into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    // 清理连接
    if let Ok(mut guard) = clients.lock() {
        guard.remove(&conn_id);
        let _ = client_count_tx.send(guard.len());
    }
    Ok(())
}

/// 解析并校验手机指令的传输层硬限制（文本长度、pane 格式留上层）。
pub(crate) fn parse_command(text: &str) -> Result<ClientCommand, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_route_helpers() {
        assert!(is_websocket_upgrade(
            "GET /v1/ws HTTP/1.1\r\nUpgrade: websocket\r\n\r\n"
        ));
        assert!(is_websocket_upgrade(
            "GET / HTTP/1.1\r\nupgrade: WebSocket\r\n\r\n"
        ));
        assert!(!is_websocket_upgrade(
            "GET /v1/ HTTP/1.1\r\nHost: 192.168.1.2\r\n\r\n"
        ));
    }

    #[test]
    fn test_fragmented_websocket_header_still_upgrades() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (stream, peer) = listener.accept().await.unwrap();
                let clients = Arc::new(Mutex::new(HashMap::new()));
                let (cmd_tx, _cmd_rx) = mpsc::unbounded_channel();
                let (count_tx, _count_rx) = mpsc::unbounded_channel();
                route_connection(
                    stream,
                    peer.ip(),
                    1,
                    clients,
                    "secret".to_string(),
                    cmd_tx,
                    count_tx,
                )
                .await
            });

            let mut client = TcpStream::connect(addr).await.unwrap();
            let first = format!(
                "GET /v1/ws?token=secret HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n",
                addr.port()
            );
            client.write_all(first.as_bytes()).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            client
                .write_all(
                    b"Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: tmuxdeck.v1\r\n\r\n",
                )
                .await
                .unwrap();

            let mut response = [0u8; 1024];
            let read = tokio::time::timeout(Duration::from_secs(2), client.read(&mut response))
                .await
                .unwrap()
                .unwrap();
            assert!(std::str::from_utf8(&response[..read])
                .unwrap()
                .starts_with("HTTP/1.1 101"));
            drop(client);
            assert!(server.await.unwrap().is_ok());
        });
    }

    #[test]
    fn test_http_host_and_token_validation() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            async fn request(raw: &str) -> String {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                let request = raw.to_string();
                let server = tokio::spawn(async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let mut buf = [0u8; 2048];
                    let n = stream
                        .readable()
                        .await
                        .and_then(|_| stream.try_read(&mut buf))
                        .unwrap();
                    serve_http(
                        &mut stream,
                        std::str::from_utf8(&buf[..n]).unwrap(),
                        "secret",
                    )
                    .await
                    .unwrap();
                });
                let mut client = TcpStream::connect(addr).await.unwrap();
                client.write_all(request.as_bytes()).await.unwrap();
                let mut response = Vec::new();
                client.read_to_end(&mut response).await.unwrap();
                server.await.unwrap();
                String::from_utf8(response).unwrap()
            }
            assert!(
                request("GET /v1/?token=secret HTTP/1.1\r\nHost: 192.168.1.2\r\n\r\n")
                    .await
                    .starts_with("HTTP/1.1 200")
            );
            assert!(
                request("GET /v1/?token=wrong HTTP/1.1\r\nHost: 192.168.1.2\r\n\r\n")
                    .await
                    .starts_with("HTTP/1.1 403")
            );
            assert!(
                request("GET /v1/?token=secret HTTP/1.1\r\nHost: evil.example.com\r\n\r\n")
                    .await
                    .starts_with("HTTP/1.1 403")
            );
            assert!(
                request("GET /nope?token=secret HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                    .await
                    .starts_with("HTTP/1.1 404")
            );

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                read_http_head(&mut stream).await.unwrap()
            });
            let mut client = TcpStream::connect(addr).await.unwrap();
            client
                .write_all(b"GET /v1/?token=secret HTTP/1.1\r\n")
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            client
                .write_all(b"Host: 192.168.1.2\r\n\r\n")
                .await
                .unwrap();
            let head = server.await.unwrap();
            assert!(head.contains("token=secret"));
            assert!(head.contains("Host: 192.168.1.2"));
        });
    }

    #[test]
    fn test_parse_command_text_limit() {
        let ok = r#"{"type":"say","id":"%3","text":"继续"}"#;
        assert!(parse_command(ok).is_ok());

        let long = format!(
            r#"{{"type":"say","id":"%3","text":"{}"}}"#,
            "x".repeat(9000)
        );
        assert!(parse_command(&long).is_err());

        let bad = r#"{"type":"nope"}"#;
        assert!(parse_command(bad).is_err());

        let binary = "not json at all";
        assert!(parse_command(binary).is_err());
    }

    #[test]
    fn test_subscribe_unsubscribe_parse() {
        let sub = r#"{"type":"subscribe","id":"%3"}"#;
        match parse_command(sub).unwrap() {
            ClientCommand::Subscribe { id } => assert_eq!(id, "%3"),
            _ => panic!("expected Subscribe"),
        }
        let unsub = r#"{"type":"unsubscribe"}"#;
        assert!(matches!(
            parse_command(unsub).unwrap(),
            ClientCommand::Unsubscribe
        ));
    }
}
