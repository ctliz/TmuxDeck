//! v1.14 共享状态与 Tauri 命令：桌面端 UI 与引擎之间的桥梁。
//!
//! 引擎（`engine.rs`）单线程独占 registry 与 transport；本模块持有
//! 引擎周期性刷新的只读快照，供桌面端 UI 经 `invoke` 读取配对信息
//! 与对话表。与 `engine.rs` 分离是 ≤400 行/文件约束下的结构拆分。

use std::sync::{Arc, Mutex};

/// 暴露给桌面端 UI 的共享状态（engine 更新，UI 读取）。
pub struct BridgeState {
    /// WebSocket 监听地址与 token（配对用，启动后填充）
    pub ws_addr: Mutex<Option<String>>,
    pub ws_token: Mutex<Option<String>>,
    /// 最近一次对话表快照（engine 周期更新）
    pub conversations: Mutex<Vec<crate::bridge::Conversation>>,
    /// broker 是否已接入
    pub broker_connected: Mutex<bool>,
    /// HTTP/WS 单端口与可用于局域网配对的主机地址
    pub port: Mutex<Option<u16>>,
    pub lan_hosts: Mutex<Vec<String>>,
    /// 当前已认证 WebSocket 手机连接数
    pub connected_clients: Mutex<usize>,
    /// Desktop asked to mint a new pairing token.
    pub rotate_token: Mutex<bool>,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            ws_addr: Mutex::new(None),
            ws_token: Mutex::new(None),
            conversations: Mutex::new(Vec::new()),
            broker_connected: Mutex::new(false),
            port: Mutex::new(None),
            lan_hosts: Mutex::new(Vec::new()),
            connected_clients: Mutex::new(0),
            rotate_token: Mutex::new(false),
        }
    }
}

/// 供 Tauri setup 调用：把引擎 spawn 到后台线程。
pub fn spawn_bridge(app_handle: tauri::AppHandle) -> Arc<BridgeState> {
    let state = Arc::new(BridgeState::default());
    let s = state.clone();
    std::thread::spawn(move || crate::engine::BridgeEngine::run(s, Some(app_handle)));
    state
}

/// 桌面端 UI 读取配对信息（WebSocket 地址 + token）。
#[tauri::command]
pub fn bridge_pairing(state: tauri::State<Arc<BridgeState>>) -> serde_json::Value {
    pairing_json(&state)
}

#[tauri::command]
pub fn refresh_bridge_pairing(state: tauri::State<Arc<BridgeState>>) -> serde_json::Value {
    if let Ok(mut flag) = state.rotate_token.lock() {
        *flag = true;
    }
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        if state.rotate_token.lock().map(|flag| !*flag).unwrap_or(true) {
            break;
        }
    }
    pairing_json(&state)
}

fn pairing_json(state: &BridgeState) -> serde_json::Value {
    let token = state.ws_token.lock().ok().and_then(|g| g.clone());
    let port = state.port.lock().ok().and_then(|g| *g);
    let hosts = state
        .lan_hosts
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let clients = state.connected_clients.lock().map(|g| *g).unwrap_or(0);
    let broker = state.broker_connected.lock().map(|b| *b).unwrap_or(false);
    let http_urls: Vec<String> = match (port, token.as_deref()) {
        (Some(port), Some(token)) => hosts
            .iter()
            .map(|host| format!("http://{}:{}/v1/?token={}", host, port, token))
            .collect(),
        _ => Vec::new(),
    };
    let ws_urls: Vec<String> = match (port, token.as_deref()) {
        (Some(port), Some(token)) => hosts
            .iter()
            .map(|host| format!("ws://{}:{}/v1/ws?token={}", host, port, token))
            .collect(),
        _ => Vec::new(),
    };
    let desktop_ws_url = match (port, token.as_deref()) {
        (Some(port), Some(token)) => Some(format!(
            "ws://127.0.0.1:{}/v1/ws?token={}&client=desktop",
            port, token
        )),
        _ => None,
    };
    serde_json::json!({
        "enabled": port.is_some() && token.is_some(),
        "port": port,
        "httpUrls": http_urls,
        "lanUrls": http_urls,
        "wsUrls": ws_urls,
        "desktopWsUrl": desktop_ws_url,
        "token": token,
        "connectedClients": clients,
        "brokerConnected": broker,
        "trustedLanOnly": true,
    })
}

/// 桌面端 UI 读取当前对话表快照（与手机端 `conversations` 事件同构）。
#[tauri::command]
pub fn bridge_conversations(
    state: tauri::State<Arc<BridgeState>>,
) -> Vec<crate::bridge::Conversation> {
    state
        .conversations
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_json_includes_desktop_ws_url_when_port_and_token_ready() {
        let state = BridgeState::default();
        *state.port.lock().unwrap() = Some(8765);
        *state.ws_token.lock().unwrap() = Some("secret_token_123".to_string());
        *state.lan_hosts.lock().unwrap() = vec!["192.168.1.50".to_string()];

        let value = pairing_json(&state);
        assert_eq!(value["enabled"], true);
        assert_eq!(value["port"], 8765);
        assert_eq!(
            value["desktopWsUrl"],
            "ws://127.0.0.1:8765/v1/ws?token=secret_token_123&client=desktop"
        );
        assert_eq!(
            value["wsUrls"][0],
            "ws://192.168.1.50:8765/v1/ws?token=secret_token_123"
        );
    }

    #[test]
    fn test_pairing_json_desktop_ws_url_is_null_when_offline() {
        let state = BridgeState::default();
        let value = pairing_json(&state);
        assert_eq!(value["enabled"], false);
        assert!(value["desktopWsUrl"].is_null());
    }
}
