# v1.14 Transport + 安全方案设计：手机端怎么连、凭什么信

> PRD v1.12 定义了手机端协议（事件/指令 JSON），但**传输与安全是空白的**。
> 本文补上：WebSocket 服务端选型、监听暴露面、认证、防滥用。
> 原则沿用 DECISIONS-v1.12：**不自己发明安全机制**、能用既有信任边界就不造轮子。

---

## 1. 传输选型

### 1.1 WebSocket 服务端

- **库**：`tokio-tungstenite`（Tauri 2 已带 tokio 运行时，无新增重量依赖）。
- **端口**：动态分配（`127.0.0.1:0` 拿空闲端口），**避免固定端口被占用/被扫描**。
- **监听**：默认只绑 `127.0.0.1`（桌面端本机访问）；手机访问走 §2 的 Tailscale 隧道，
  即服务端**永不直接暴露在局域网明文上**。
- **路径**：`ws://127.0.0.1:<port>/v1/ws`，子协议固定 `tmuxdeck.v1`。
- **生命周期**：Tauri `setup` 里起后台任务；无客户端连接时照常运行（对话表刷新
  仍要消费 intercom 事件），手机断线重连由客户端负责。

### 1.2 手机怎么连（关键决策）

**不绑 `0.0.0.0`、不做局域网明文 HTTP**——DECISIONS-v1.12 第 1 条已经否过
（token 会被同网段嗅探、iOS 自签证书体验差）。取而代之：

```
手机 ── Tailscale（WireGuard，端到端加密）──> Mac 的 tailnet IP:port
        │                                       │
        └── ws://100.x.y.z:<port>/v1/ws?token=… └── 服务端只绑 127.0.0.1
```

- 手机上跑 Tailscale（App Store 免费），进入同一个 tailnet 后
  直接访问 Mac 的 `100.x.y.z`。
- **WireGuard 隧道本身提供机密性**（E2E 加密），这是既有信任边界，
  我们不再发明 TLS/证书方案。
- `ws://` + 明文 HTTP 页面在 tailnet 内是安全的；页面用 HTTPS
  （Tailscale 的 MagicDNS 域名 `mac-name.tailnet-name.ts.net`）则免费拿到
  CA 证书，进一步挡掉 tailnet 内的中间人（同 tailnet 的其他设备）。

**安全模型**：Tailscale 负责「谁在网络上」，token 负责「谁是这个手机客户端」，
两者叠加。第 3 节详述。

### 1.3 手机端 UI 形态

- v1.14 用**静态 SPA**（单 HTML+JS，从同一服务端 /v1/ 托管）：
  - 扫码配对：桌面端显示二维码（含 `ws://…?token=…` 或 HTTPS 版 URL）
  - 多路对话视图：对话列表（`conversations` 事件）+ 每对话的消息流
    （`turn` 事件）+ 输入框（`say`）+ 控制键（`key`）+ 转发（`forward`）
  - 仅当收到 `awaiting-human` 时发 Web 通知/声音（唯一推送信号，PRD §3）
- 不引框架、不建 PWA 安装流程；先把对话体验跑通，原生 App 是后续选项。

---

## 2. 监听暴露面（攻击面清单）

| 面 | 设计 | 理由 |
|---|---|---|
| 端口 | 动态分配，只绑 loopback/Tailscale IP | 无固定端口可扫；不进局域网 |
| HTTP 服务 | 与 WS 同端口同进程 | 一个面好管；仅 tailnet 可达 |
| 手机入口 | Tailscale 隧道，非公网 | 复用既有身份与加密，不自建 |
| DNS rebinding | WS 握手校验 `Host` 头 | 见 §4.2 |
| 明文 | `ws://` 仅在 tailnet 内；`https://` 经 MagicDNS | tailnet 外无监听面 |

---

## 3. 认证：配对 token

### 3.1 token 的生成与传递

- 每次应用启动生成 **32 字节 CSPRNG token**（`OsRng`），**不落盘**。
- 桌面端 UI 显示为二维码 + 可复制文本；手机扫码即可带上 token 连接。
- token 只存在于：桌面内存、二维码/剪贴板、手机端内存。
  应用退出即失效——**不做持久化、不做吊销列表**（没有持久凭证就没有吊销问题）。

### 3.2 握手与校验

- 连接 URL：`ws://host:port/v1/ws?token=<hex>`。
- 服务端握手时：
  1. 校验子协议 = `tmuxdeck.v1`；
  2. 校验 `Host` 头在白名单（`127.0.0.1` / `localhost` / tailnet IP / MagicDNS 域名），
     否则拒绝——**防 DNS rebinding**；
  3. 提取 `token`，**常量时间比较**（`subtle` crate 的
     `ct_eq`，避免时序侧信道）；
  4. 失败记一条日志，断开。**不区分「token 错」与「无 token」**
     ——不给攻击者探测信息。
- 每 IP 每 10 秒最多 5 次握手尝试，超限静默丢弃（`IpAddr` 桶）。

### 3.3 多设备

同一 token 允许多个连接（家人两台手机）；全部复用同一对话表。
无设备管理 UI——v1.14 不做，需要时再引入设备名。

---

## 4. 连接内防滥用

### 4.1 帧级限制（服务端强制的硬上限）

| 项 | 上限 | 处理 |
|---|---|---|
| 单帧 JSON | 64 KiB | 超限断开 |
| `text` 字段 | 8 KiB（对齐 `send_keys` 的 `MAX_SEND_TEXT_BYTES`） | 拒绝该指令 |
| 入站速率 | 100 帧/秒/连接 | 超限断开 |
| 未收到 `pong` | 60 秒 | 断开（心跳 20s） |

### 4.2 指令校验（复用已有白名单）

- `say.id` / `key.id` / `forward.from|to` 必须通过 `validate_pane_id`，
  且**存在于 ConversationRegistry**——不存在的 pane 一律拒绝。
- `key.key` 必须命中 `ALLOWED_KEYS` 白名单（`tmux.rs` 已有），
  **手机端不能发送任意按键序列**。
- 转发时 `from != to`。
- 所有指令落在服务端日志（时间、来源 IP、pane、指令类型、文本摘要），
  便于事后审计——**不做任何「执行任意命令」的接口**。

### 4.3 桌面端联动

- 收到手机连接/断线时，桌面端显示「手机已连接」状态（也便于排查）。
- `ClientCommand::Refresh` 触发一次 registry 重扫 + 全量下发。

---

## 5. 消息流转（衔接阶段 1 的桥）

```
手机 ──say──▶ WS 服务端 ──ClientCommand::Say──▶ deliver() ──▶ intercom broker / send-keys
手机 ◀──turn── WS 服务端 ◀──TranscriptSource 轮询 ◀── registry + transcript 文件
手机 ◀──awaiting-human── WS 服务端 ◀── intercom Message(expectsReply) ── broker
```

- 轮询节奏：`has_clients()` 为真时才跑（手机不在线不空转）；
  **轮询范围收窄到订阅粒度**：只跑 `subscribe` 的对话（`turn` 按订阅推），
  其余对话不轮询——`status-changed` / `awaiting-human` 全量推不受影响（来自 broker 事件，零轮询成本）；
  有 `awaiting-human` 或对话状态为 `thinking` 的 pane 时加密轮询间隔
  （500ms），其余 2s。
- `subscribe` 时：立即推一次该对话 transcript 尾部（游标起始快照）；`unsubscribe` 停止该对话轮询。
- 手机 `say` 的回复送达：经 intercom 有回执（`delivered` 事件）→
  可透传为 `ClientEvent::Delivered`（可选）；send-keys 路径无回执，
  直接显示「已发送」即可。

---

## 6. 验收清单

- [ ] 服务端只监听 `127.0.0.1:<动态端口>`；`lsof` 确认无 `0.0.0.0` 监听
- [ ] 无 token / 错 token / 错 Host 头均被拒，且不区分错误原因
- [ ] 连上后能收到 `conversations` 全量；桌面端显示手机在线
- [ ] 手机发 `say` → pane 内 agent 收到；`key` 白名单外被拒
- [ ] 手机发不存在的 pane id → 拒绝
- [ ] 超 64 KiB 帧 / 超速 / 断心跳 → 断开
- [ ] 手机离线时服务端停止 transcript 轮询（`has_clients` 生效）
- [ ] 应用退出后旧 token 立即失效（重启需重新扫码）

真机验证：

- [ ] iPhone + Tailscale 同一 tailnet，扫码连上，多路对话可并行收发
- [ ] 跨 pane 转发 A→B 生效且带来源标注
- [ ] 一个 agent `ask` 时手机收到 `awaiting-human` 通知（含 reply_to）
