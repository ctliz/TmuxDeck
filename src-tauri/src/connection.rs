//! v1.14 WebSocket 连接处理：接受循环 + 单连接双向循环 + 指令解析。
//!
//! 与 `transport.rs` 分离的原因：保持每个文件实现部分 ≤400 行。
//! 本模块是纯传输层——握手校验、限流、心跳、帧限制、订阅状态更新，
//! 不涉及对话语义（那是引擎的事）。

use crate::bridge::{ClientCommand, ClientCommand::Subscribe, ClientCommand::Unsubscribe};
use crate::transport::{host_allowed, ConnState, WsTransport, MAX_FRAME_BYTES, MAX_TEXT_BYTES};
use crate::transport::{HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT, MAX_FRAMES_PER_SEC, WS_SUBPROTOCOL};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};

/// 接受循环：监听 → 握手限速 → 每连接一个任务。
pub(crate) async fn accept_loop(
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
        recent.retain(|t| now.duration_since(*t) < crate::transport::HANDSHAKE_WINDOW);
        if recent.len() >= crate::transport::HANDSHAKE_MAX {
            continue; // 静默丢弃，不给探测信息
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
    let ws = accept_hdr_async(
        stream,
        |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
         resp: tokio_tungstenite::tungstenite::handshake::server::Response| {
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
                .map(|t| crate::transport::ct_eq(t, &token))
                .unwrap_or(false);
            if !(proto_ok && host_ok && token_ok) {
                // 统一拒绝，不区分失败原因（无探测信息）
                return Err(tokio_tungstenite::tungstenite::handshake::server::ErrorResponse::new(
                    None,
                ));
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
                                // 订阅状态就地更新（emit 过滤用），指令仍转发引擎（推快照+收窄轮询）
                                match &cmd {
                                    Subscribe { id } => {
                                        WsTransport::set_subscription(
                                            conn_id,
                                            Some(id.clone()),
                                            &clients,
                                        );
                                    }
                                    Unsubscribe => {
                                        WsTransport::set_subscription(conn_id, None, &clients);
                                    }
                                    _ => {}
                                }
                                let _ = cmd_tx.send(cmd);
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
    fn test_subscribe_unsubscribe_parse() {
        let sub = r#"{"type":"subscribe","id":"%3"}"#;
        match parse_command(sub).unwrap() {
            ClientCommand::Subscribe { id } => assert_eq!(id, "%3"),
            _ => panic!("expected Subscribe"),
        }
        let unsub = r#"{"type":"unsubscribe"}"#;
        assert!(matches!(parse_command(unsub).unwrap(), ClientCommand::Unsubscribe));
    }
}
