//! pi-intercom broker 客户端
//!
//! TmuxDeck 以「人类适配器」的身份接入 pi-intercom 的本地 broker：注册成一个
//! 名为 `me` 的会话，于是任何 agent 都能用 `intercom send to:"me"` 找人，
//! 而手机端的回复也能经由 broker 可靠地投递回任意 agent 会话。
//!
//! 这一层替代了「轮询 capture-pane + 静默启发式猜状态」的全部工作：
//! broker 本身就持有会话注册表与实时状态（idle / thinking / tool:<name>）。
//!
//! 协议（对齐 @dataforxyz/agent-intercom-* protocol v3）：
//!   传输：Unix domain socket，`~/.pi/agent/intercom/broker.sock`
//!         （若设置了 `PI_CODING_AGENT_DIR`，则为 `$PI_CODING_AGENT_DIR/intercom/`）
//!   分帧：4 字节大端长度前缀 + UTF-8 JSON，单帧上限 1 MiB
//!
//! 刻意的设计取舍：入站帧用 `serde_json::Value` 手工分派，而不是内部标记枚举。
//! 协议在演进（dataforxyz 的跨 harness 分支已到 v3，新增了若干帧类型），
//! 手工分派遇到未知类型时忽略即可，不会整条连接反序列化失败。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

const MAX_FRAME_BYTES: u32 = 1024 * 1024;

/// 我们在总线上的身份。agent 用这个名字寻址到「人」。
pub const HUMAN_SESSION_NAME: &str = "me";

// ─────────────────────────────────────────────────────────────────────────────
// 协议数据结构
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub pid: i64,
    #[serde(default)]
    pub started_at: i64,
    #[serde(default)]
    pub last_activity: i64,
    /// `idle` / `thinking` / `tool:<name>`，由各会话自动上报
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_pct: Option<f64>,
}

/// 会话的活动状态。这是四态判定的**事实来源**，不是猜出来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Thinking,
    Tool(String),
    Unknown,
}

impl SessionInfo {
    pub fn agent_status(&self) -> AgentStatus {
        match self.status.as_deref() {
            Some("idle") => AgentStatus::Idle,
            Some("thinking") => AgentStatus::Thinking,
            Some(s) if s.starts_with("tool:") => AgentStatus::Tool(s[5..].to_string()),
            _ => AgentStatus::Unknown,
        }
    }

    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Attachment {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MessageContent {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntercomMessage {
    pub id: String,
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// true 表示对方在 `ask`，正阻塞等待回复——这是最高优先级的「人被需要」信号
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expects_reply: Option<bool>,
    pub content: MessageContent,
}

impl IntercomMessage {
    pub fn expects_reply(&self) -> bool {
        self.expects_reply.unwrap_or(false)
    }
}

/// 从 broker 收到的事件。上层（会话桥 / 手机端传输层）消费这些事件。
#[derive(Debug, Clone)]
pub enum IntercomEvent {
    Registered { session_id: String, features: Vec<String> },
    Sessions { request_id: String, sessions: Vec<SessionInfo> },
    Message {
        from: SessionInfo,
        message: IntercomMessage,
        delivery_id: String,
    },
    PresenceUpdate { session: SessionInfo },
    SessionJoined { session: SessionInfo },
    SessionLeft { session_id: String },
    Delivered { message_id: String },
    DeliveryFailed { message_id: String, reason: String },
    BrokerError { error: String },
    Disconnected,
}

// ─────────────────────────────────────────────────────────────────────────────
// socket 路径
// ─────────────────────────────────────────────────────────────────────────────

pub fn runtime_dir() -> Option<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.is_empty() {
            return Some(std::path::PathBuf::from(dir).join("intercom"));
        }
    }
    dirs::home_dir().map(|h| h.join(".pi").join("agent").join("intercom"))
}

pub fn socket_path() -> Option<std::path::PathBuf> {
    runtime_dir().map(|d| d.join("broker.sock"))
}

/// broker 是否在跑。broker 由第一个 intercom 会话自动拉起，
/// 最后一个会话断开 5 秒后自行退出——所以「不在跑」是常态，不是错误。
pub fn broker_available() -> bool {
    socket_path().map(|p| p.exists()).unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// 分帧
// ─────────────────────────────────────────────────────────────────────────────

fn write_frame<W: Write>(w: &mut W, value: &Value) -> std::io::Result<()> {
    let json = serde_json::to_vec(value)?;
    if json.len() as u32 > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame exceeds 1 MiB",
        ));
    }
    let mut frame = Vec::with_capacity(4 + json.len());
    frame.extend_from_slice(&(json.len() as u32).to_be_bytes());
    frame.extend_from_slice(&json);
    w.write_all(&frame)?;
    w.flush()
}

fn read_frame<R: Read>(r: &mut R) -> std::io::Result<Value> {
    let mut header = [0u8; 4];
    r.read_exact(&mut header)?;
    let len = u32::from_be_bytes(header);
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame length {} exceeds maximum", len),
        ));
    }
    let mut payload = vec![0u8; len as usize];
    r.read_exact(&mut payload)?;
    serde_json::from_slice(&payload)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn new_id() -> String {
    // broker 只要求消息 ID 在发送方内唯一，不要求 UUID 格式。
    // 避免为此引入 uuid 依赖。
    format!("tmuxdeck-{}-{}", std::process::id(), now_ms())
}

fn registration_frame(name: &str, cwd: String, pid: u32, now: i64) -> Value {
    json!({
        "type": "register",
        "protocol": "pi-intercom",
        "version": 3,
        "session": {
            "name": name,
            "cwd": cwd,
            "model": "human",
            "pid": pid,
            "startedAt": now,
            "lastActivity": now,
            "status": "idle",
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 客户端
// ─────────────────────────────────────────────────────────────────────────────

pub struct IntercomClient {
    #[cfg(unix)]
    writer: Arc<Mutex<UnixStream>>,
    session_id: Arc<Mutex<Option<String>>>,
    connected: Arc<AtomicBool>,
}

impl IntercomClient {
    /// 连接 broker 并注册为人类会话。返回客户端与事件流。
    ///
    /// 不会自动拉起 broker——broker 的生命周期归 pi 管，我们只搭便车。
    /// broker 不在时返回 Err，调用方应降级到 send-keys 通道。
    #[cfg(unix)]
    pub fn connect(name: &str) -> Result<(Self, Receiver<IntercomEvent>), String> {
        let path = socket_path().ok_or("ERR_NO_HOME_DIR")?;
        if !path.exists() {
            return Err("ERR_BROKER_NOT_RUNNING".to_string());
        }

        let stream = UnixStream::connect(&path)
            .map_err(|e| format!("ERR_BROKER_CONNECT|{}", e))?;
        let reader_stream = stream
            .try_clone()
            .map_err(|e| format!("ERR_SOCKET_CLONE|{}", e))?;

        let writer = Arc::new(Mutex::new(stream));
        let session_id = Arc::new(Mutex::new(None));
        let connected = Arc::new(AtomicBool::new(true));
        let (tx, rx) = channel();

        // 注册。cwd/model 是展示元数据，broker 不做校验；
        // model 填 "human" 让其他会话在 list 里一眼看出这是人不是 agent。
        let now = now_ms();
        let register = registration_frame(
            name,
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            std::process::id(),
            now,
        );
        {
            let mut w = writer.lock().map_err(|_| "ERR_LOCK_POISONED")?;
            write_frame(&mut *w, &register).map_err(|e| format!("ERR_REGISTER|{}", e))?;
        }

        spawn_reader(reader_stream, tx, session_id.clone(), connected.clone());

        Ok((
            Self { writer, session_id, connected },
            rx,
        ))
    }

    #[cfg(not(unix))]
    pub fn connect(_name: &str) -> Result<(Self, Receiver<IntercomEvent>), String> {
        // Windows 下 broker 用命名管道，需要单独实现。
        Err("ERR_INTERCOM_UNSUPPORTED_PLATFORM".to_string())
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn session_id(&self) -> Option<String> {
        self.session_id.lock().ok().and_then(|g| g.clone())
    }

    #[cfg(unix)]
    fn send_frame(&self, value: Value) -> Result<(), String> {
        if !self.is_connected() {
            return Err("ERR_NOT_CONNECTED".to_string());
        }
        let mut w = self.writer.lock().map_err(|_| "ERR_LOCK_POISONED")?;
        write_frame(&mut *w, &value).map_err(|e| format!("ERR_WRITE|{}", e))
    }

    #[cfg(not(unix))]
    fn send_frame(&self, _value: Value) -> Result<(), String> {
        Err("ERR_INTERCOM_UNSUPPORTED_PLATFORM".to_string())
    }

    /// 请求会话列表。结果异步经 `IntercomEvent::Sessions` 返回。
    pub fn request_list(&self) -> Result<String, String> {
        let request_id = new_id();
        self.send_frame(json!({ "type": "list", "requestId": request_id }))?;
        Ok(request_id)
    }

    /// 向某个会话发消息。`to` 可以是会话名或会话 ID。
    ///
    /// 相比 send-keys 直接往 pane 里塞字符，这条路径由 broker 负责
    /// 「目标忙时排队、空闲时才注入」，不会打断正在思考的 agent。
    pub fn send(&self, to: &str, text: &str) -> Result<String, String> {
        let message_id = new_id();
        self.send_frame(json!({
            "type": "send",
            "to": to,
            "message": {
                "id": message_id,
                "timestamp": now_ms(),
                "content": { "text": text }
            }
        }))?;
        Ok(message_id)
    }

    /// 回复某条入站消息（携带 replyTo，收方会把它匹配到对应的 ask 上）。
    pub fn reply(&self, to: &str, text: &str, reply_to: &str) -> Result<String, String> {
        let message_id = new_id();
        self.send_frame(json!({
            "type": "send",
            "to": to,
            "message": {
                "id": message_id,
                "timestamp": now_ms(),
                "replyTo": reply_to,
                "content": { "text": text }
            }
        }))?;
        Ok(message_id)
    }

    /// v3 回执：确认 broker 分配的 deliveryId 已被我们收到。
    pub fn acknowledge(&self, delivery_id: &str) -> Result<(), String> {
        self.send_frame(json!({
            "type": "message_received",
            "deliveryId": delivery_id,
        }))
    }

    /// 更新我们自己的状态。手机端在线时报 idle，人不在时可报自定义状态，
    /// 让 agent 在 list 里看得到「人现在在不在」。
    pub fn update_presence(&self, status: &str) -> Result<(), String> {
        self.send_frame(json!({ "type": "presence", "status": status }))
    }

    pub fn disconnect(&self) {
        let _ = self.send_frame(json!({ "type": "unregister" }));
        self.connected.store(false, Ordering::Relaxed);
    }
}

impl Drop for IntercomClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 读取线程
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(unix)]
fn spawn_reader(
    stream: UnixStream,
    tx: Sender<IntercomEvent>,
    session_id: Arc<Mutex<Option<String>>>,
    connected: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut stream = stream;
        loop {
            match read_frame(&mut stream) {
                Ok(value) => {
                    if let Some(event) = parse_broker_frame(&value, &session_id) {
                        if tx.send(event).is_err() {
                            break; // 上层已丢弃接收端
                        }
                    }
                }
                Err(_) => break, // EOF 或协议错误：broker 退出/重启
            }
        }
        connected.store(false, Ordering::Relaxed);
        let _ = tx.send(IntercomEvent::Disconnected);
    });
}

/// 手工分派，未知帧类型返回 None 而非报错——协议在演进，容忍未知是必需的。
fn parse_broker_frame(v: &Value, session_id: &Arc<Mutex<Option<String>>>) -> Option<IntercomEvent> {
    let kind = v.get("type")?.as_str()?;
    match kind {
        "registered" => {
            let id = v.get("sessionId")?.as_str()?.to_string();
            if let Ok(mut guard) = session_id.lock() {
                *guard = Some(id.clone());
            }
            let features = v
                .get("features")
                .and_then(|f| f.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            Some(IntercomEvent::Registered { session_id: id, features })
        }
        "sessions" => Some(IntercomEvent::Sessions {
            request_id: v.get("requestId")?.as_str()?.to_string(),
            sessions: serde_json::from_value(v.get("sessions")?.clone()).ok()?,
        }),
        "message" => Some(IntercomEvent::Message {
            from: serde_json::from_value(v.get("from")?.clone()).ok()?,
            message: serde_json::from_value(v.get("message")?.clone()).ok()?,
            delivery_id: v.get("deliveryId")?.as_str()?.to_string(),
        }),
        "presence_update" => Some(IntercomEvent::PresenceUpdate {
            session: serde_json::from_value(v.get("session")?.clone()).ok()?,
        }),
        "session_joined" => Some(IntercomEvent::SessionJoined {
            session: serde_json::from_value(v.get("session")?.clone()).ok()?,
        }),
        "session_left" => Some(IntercomEvent::SessionLeft {
            session_id: v.get("sessionId")?.as_str()?.to_string(),
        }),
        "delivered" => Some(IntercomEvent::Delivered {
            message_id: v.get("messageId")?.as_str()?.to_string(),
        }),
        "delivery_failed" => Some(IntercomEvent::DeliveryFailed {
            message_id: v.get("messageId")?.as_str()?.to_string(),
            reason: v.get("reason").and_then(|r| r.as_str()).unwrap_or("unknown").to_string(),
        }),
        "error" => Some(IntercomEvent::BrokerError {
            error: v.get("error")?.as_str()?.to_string(),
        }),
        // extension_* / message_control 等暂不消费
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_parsing() {
        let mk = |s: Option<&str>| SessionInfo {
            status: s.map(String::from),
            ..Default::default()
        };
        assert_eq!(mk(Some("idle")).agent_status(), AgentStatus::Idle);
        assert_eq!(mk(Some("thinking")).agent_status(), AgentStatus::Thinking);
        assert_eq!(
            mk(Some("tool:bash")).agent_status(),
            AgentStatus::Tool("bash".to_string())
        );
        assert_eq!(mk(None).agent_status(), AgentStatus::Unknown);
        assert_eq!(mk(Some("weird")).agent_status(), AgentStatus::Unknown);
    }

    #[test]
    fn test_registration_uses_protocol_v3() {
        let frame = registration_frame("me", "/tmp".to_string(), 42, 1000);
        assert_eq!(frame["type"], "register");
        assert_eq!(frame["protocol"], "pi-intercom");
        assert_eq!(frame["version"], 3);
        assert_eq!(frame["session"]["name"], "me");
    }

    #[test]
    fn test_frame_roundtrip() {
        let value = json!({ "type": "list", "requestId": "abc" });
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, &value).unwrap();
        // 4 字节大端长度前缀
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        assert_eq!(len, buf.len() - 4);

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_frame(&mut cursor).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn test_parse_unknown_frame_is_ignored() {
        let sid = Arc::new(Mutex::new(None));
        let v = json!({ "type": "some_future_frame", "payload": 1 });
        assert!(parse_broker_frame(&v, &sid).is_none());
    }

    #[test]
    fn test_parse_registered_sets_session_id() {
        let sid = Arc::new(Mutex::new(None));
        let v = json!({ "type": "registered", "sessionId": "s-1", "features": ["extension-bus-v1"] });
        match parse_broker_frame(&v, &sid) {
            Some(IntercomEvent::Registered { session_id, features }) => {
                assert_eq!(session_id, "s-1");
                assert_eq!(features, vec!["extension-bus-v1".to_string()]);
            }
            _ => panic!("expected Registered"),
        }
        assert_eq!(sid.lock().unwrap().clone(), Some("s-1".to_string()));
    }

    #[test]
    fn test_parse_message_expects_reply() {
        let sid = Arc::new(Mutex::new(None));
        let v = json!({
            "type": "message",
            "deliveryId": "delivery-1",
            "from": { "id": "s-2", "name": "planner", "cwd": "/tmp", "model": "sonnet", "pid": 1, "startedAt": 0, "lastActivity": 0 },
            "message": { "id": "m-1", "timestamp": 0, "expectsReply": true, "content": { "text": "需要你确认" } }
        });
        match parse_broker_frame(&v, &sid) {
            Some(IntercomEvent::Message {
                from,
                message,
                delivery_id,
            }) => {
                assert_eq!(from.display_name(), "planner");
                assert!(message.expects_reply());
                assert_eq!(message.content.text, "需要你确认");
                assert_eq!(delivery_id, "delivery-1");
            }
            _ => panic!("expected Message"),
        }

        let mut missing_delivery = v;
        missing_delivery.as_object_mut().unwrap().remove("deliveryId");
        assert!(parse_broker_frame(&missing_delivery, &sid).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_acknowledge_uses_delivery_id_v3_frame() {
        let (stream, mut peer) = UnixStream::pair().unwrap();
        let client = IntercomClient {
            writer: Arc::new(Mutex::new(stream)),
            session_id: Arc::new(Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(true)),
        };

        client.acknowledge("delivery-1").unwrap();
        assert_eq!(
            read_frame(&mut peer).unwrap(),
            json!({ "type": "message_received", "deliveryId": "delivery-1" })
        );
    }
}
