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
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            ws_addr: Mutex::new(None),
            ws_token: Mutex::new(None),
            conversations: Mutex::new(Vec::new()),
            broker_connected: Mutex::new(false),
        }
    }
}

/// 供 Tauri setup 调用：把引擎 spawn 到后台线程。
pub fn spawn_bridge() -> Arc<BridgeState> {
    let state = Arc::new(BridgeState::default());
    let s = state.clone();
    std::thread::spawn(move || crate::engine::BridgeEngine::run(s));
    state
}

/// 桌面端 UI 读取配对信息（WebSocket 地址 + token）。
#[tauri::command]
pub fn bridge_pairing(state: tauri::State<Arc<BridgeState>>) -> serde_json::Value {
    let addr = state.ws_addr.lock().ok().and_then(|g| g.clone());
    let token = state.ws_token.lock().ok().and_then(|g| g.clone());
    let broker = state.broker_connected.lock().map(|b| *b).unwrap_or(false);
    serde_json::json!({
        "addr": addr,
        "token": token,
        "brokerConnected": broker,
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
