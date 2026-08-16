# RustDesk 协议实现文档

本文档详细说明了 HarmonyDesk 中实现的 RustDesk 协议。

## 目录

- [协议概述](#协议概述)
- [连接流程](#连接流程)
- [数据包格式](#数据包格式)
- [NAT 穿透](#nat-穿透)
- [安全握手](#安全握手)
- [视频流](#视频流)
- [输入转发](#输入转发)

---

## 协议概述

HarmonyDesk 实现了兼容 RustDesk 的远程桌面协议，主要组件包括：

### 核心模块

```
src/
├── protocol.rs       # 协议层实现
│   ├── MessageType          # 消息类型定义
│   ├── Packet               # 数据包序列化/反序列化
│   ├── IdServerClient       # ID 服务器通信
│   ├── NatTraversal         # NAT 穿透
│   ├── SecureHandshake      # 安全握手
│   ├── VideoStreamReceiver  # 视频流接收
│   └── InputEventSender     # 输入事件发送
│
├── rustdesk/mod.rs   # RustDesk 连接管理
│   ├── RustDeskConfig       # 连接配置
│   ├── RustDeskConnection   # 连接管理器
│   └── RustDeskVideoStream  # 视频流包装
│
└── core.rs           # 核心管理器
    └── CoreManager          # 顶层 API
```

---

## 连接流程

### 完整连接过程

```
控制端                          ID 服务器                       被控端
  │                                │                              │
  │  1. 连接到 ID 服务器             │                              │
  ├─────────────────────────────>│                              │
  │                                │                              │
  │  2. 请求连接到 remote_id        │                              │
  ├─────────────────────────────>│  3. 转发连接请求              │
  │                                ├──────────────────────────>│
  │                                │                              │
  │                                │  4. 返回对端地址              │
  │  <────────────────────────────┤<───────────────────────────┤
  │  peer_addr                     │                              │
  │                                │                              │
  │  5. NAT 打洞                    │                              │
  ├────────────────────────────────────────────────────────────>│
  │  (punch packets)               │                              │
  │                                │                              │
  │  6. 安全握手                    │                              │
  ├────────────────────────────────────────────────────────────>│
  │  password_hash                  │                              │
  │                                │                              │
  │  7. 握手响应                    │                              │
  │<────────────────────────────────────────────────────────────┤
  │  auth_success                   │                              │
  │                                │                              │
  │  ╔═════════════════════════════════════════════════════════╗│
  │  ║           连接建立成功，开始通信                        ║│
  │  ╚═════════════════════════════════════════════════════════╝│
  │                                │                              │
  │  8. 视频流 <───────────────────────────────────────────────┤
  │  9. 输入事件 ─────────────────────────────────────────────>│
```

### 代码实现

```rust
// rustdesk/mod.rs: RustDeskConnection::connect()

pub async fn connect(&mut self) -> Result<(), String> {
    // 步骤 1: 连接到 ID 服务器
    let mut id_client = IdServerClient::new(...);
    id_client.connect().await?;

    // 步骤 2: 请求对端信息
    let peer_addr = id_client.request_connection(&self.config.desk_id).await?;

    // 步骤 3: NAT 穿透
    let mut nat_traversal = NatTraversal::new();
    nat_traversal.punch_hole(peer_addr).await?;

    // 步骤 4: 安全握手
    let mut handshake = SecureHandshake::new();
    handshake.perform_handshake(&socket, peer_addr, &self.password).await?;

    // 步骤 5: 建立连接
    // 保存连接信息，创建输入/视频通道

    Ok(())
}
```

---

## 数据包格式

### 数据包结构

```
+--------+--------+--------+--------+--------+--------+--------+
|                Message Type (2 bytes)                      |
+--------+--------+--------+--------+--------+--------+--------+
|                Payload Length (4 bytes)                    |
+--------+--------+--------+--------+--------+--------+--------+
|                                                               |
|                Payload Data (N bytes)                        |
|                                                               |
+--------+--------+--------+--------+--------+--------+--------+
```

### 消息类型

| 类型值 | 名称 | 方向 | 说明 |
|--------|------|------|------|
| 0x01 | Handshake | 双向 | 连接握手 |
| 0x02 | HandshakeResponse | 双向 | 握手响应 |
| 0x03 | ConnectionRequest | 控制端→服务器 | 请求连接 |
| 0x04 | ConnectionResponse | 服务器→控制端 | 连接响应 |
| 0x05 | Disconnect | 双向 | 断开连接 |
| 0x10 | VideoFrame | 被控端→控制端 | 视频帧 |
| 0x11 | VideoConfig | 被控端→控制端 | 视频配置 |
| 0x12 | KeepAlive | 双向 | 保活心跳 |
| 0x20 | KeyEvent | 控制端→被控端 | 键盘事件 |
| 0x21 | MouseEvent | 控制端→被控端 | 鼠标事件 |
| 0x22 | ClipboardEvent | 双向 | 剪贴板事件 |
| 0xF0 | Ping | 双向 | Ping |
| 0xF1 | Pong | 双向 | Pong |
| 0xFF | Error | 双向 | 错误消息 |

### 数据包示例

```rust
// protocol.rs: Packet

// 创建数据包
let packet = Packet::new(
    MessageType::KeyEvent,
    vec![0x00, 0x00, 0x00, 0x1E, 0x01]  // key=30, pressed=true
);

// 序列化
let data = packet.serialize();
// [0x00, 0x20, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x1E, 0x01]
//  ^类型^  ^^^^^长度^^^^^          ^^^^^^payload^^^^^^^

// 反序列化
let decoded = Packet::deserialize(&data)?;
```

---

## NAT 穿透

### P2P 打洞原理

```
控制端 NAT                      被控端 NAT
    │                              │
    │  1. 发送打洞包                │
    ├─────────────────────────────>│
    │  (punch_1)                   │
    │                              │
    │  2. 发送打洞包                │
    ├─────────────────────────────>│
    │  (punch_2)                   │
    │                              │
    │  3. 发送打洞包                │
    ├─────────────────────────────>│
    │  (punch_3)                   │
    │                              │
    │  NAT 映射建立                 │  NAT 映射建立
    │  允许来自被控端的数据包        │  允许来自控制端的数据包
    │                              │
    │  ╔══════════════════════════════════════╗
    │  ║     P2P 连接建立成功                ║
    │  ╚══════════════════════════════════════╝
```

### 代码实现

```rust
// protocol.rs: NatTraversal::punch_hole()

pub async fn punch_hole(&mut self, peer_addr: SocketAddr) -> Result<(), ProtocolError> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    // 发送多个打洞包
    for i in 0..5 {
        let packet = Packet::new(
            MessageType::Ping,
            format!("punch_{}", i).into_bytes()
        );
        let data = packet.serialize();

        socket.send_to(&data, peer_addr).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}
```

---

## 安全握手

### 握手流程

```
控制端                          被控端
  │                               │
  │  1. 发送密码哈希               │
  ├────────────────────────────>│
  │  SHA256(password + salt)      │
  │                               │
  │  2. 验证密码                   │
  │                         (验证哈希)
  │                               │
  │  3. 返回握手结果               │
  │<────────────────────────────┤
  │  status=0 (成功)              │
  │                               │
  │  ╔══════════════════════════════════════╗
  │  ║      安全通道建立                   ║
  │  ╚══════════════════════════════════════╝
```

### 密码哈希

```rust
// protocol.rs: SecureHandshake::perform_handshake()

use sha2::{Digest, Sha256};

let mut hasher = Sha256::new();
hasher.update(password.as_bytes());
hasher.update(b"RustDesk");  // 固定 salt
let password_hash = hasher.finalize();

// 发送到对端进行验证
```

---

## 视频流

### 视频帧格式

```
+--------+--------+--------+--------+
|         Width (4 bytes)            |
+--------+--------+--------+--------+
|         Height (4 bytes)           |
+--------+--------+--------+--------+
|         Timestamp (8 bytes)        |
+--------+--------+--------+--------+
|                                       |
|         Frame Data (N bytes)          |
|         (H.264 encoded)               |
|                                       |
+--------+--------+--------+--------+
```

### 接收流程

```rust
// protocol.rs: VideoStreamReceiver

pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,      // H.264 编码数据
    pub timestamp: u64,
}

// 接收视频帧
pub fn handle_packet(&self, packet: &Packet) -> Result<(), ProtocolError> {
    if packet.msg_type == MessageType::VideoFrame {
        let mut data = BytesMut::from(&packet.payload[..]);

        let width = data.get_u32();
        let height = data.get_u32();
        let timestamp = data.get_u64();
        let frame_data = data.to_vec();

        let frame = VideoFrame { width, height, data: frame_data, timestamp };

        // 发送到 UI 层显示
        self.frame_sender.send(frame)?;
    }

    Ok(())
}
```

### 视频编解码

未来将支持 H.264 硬件解码：

```toml
# Cargo.toml
[dependencies]
openh264 = { version = "0.6", optional = true }

[features]
video = ["openh264"]
```

---

## 输入转发

### 键盘事件

```rust
// protocol.rs: InputEventSender::send_key_event()

pub async fn send_key_event(&self, key: u32, pressed: bool) -> Result<(), ProtocolError> {
    let mut payload = BytesMut::new();
    payload.put_u32(key);        // 键码 (scancode)
    payload.put_u8(pressed as u8); // 状态 (0=释放, 1=按下)

    let packet = Packet::new(MessageType::KeyEvent, payload.to_vec());
    let data = packet.serialize();

    self.socket.send_to(&data, self.peer_addr).await?;
    Ok(())
}
```

### 鼠标事件

```rust
// 鼠标移动
pub async fn send_mouse_move(&self, x: i32, y: i32) -> Result<(), ProtocolError> {
    let mut payload = BytesMut::new();
    payload.put_i32(x);
    payload.put_i32(y);

    let packet = Packet::new(MessageType::MouseEvent, payload.to_vec());
    // ...
}

// 鼠标点击
pub async fn send_mouse_click(&self, button: u32, pressed: bool) -> Result<(), ProtocolError> {
    let mut payload = BytesMut::new();
    payload.put_u32(button);  // 1=左键, 2=中键, 3=右键
    payload.put_u8(pressed as u8);

    let packet = Packet::new(MessageType::MouseEvent, payload.to_vec());
    // ...
}
```

---

## ID 服务器配置

### 默认服务器

```
router.rustdesk.com:21116  # 官方 ID 服务器
```

### 自建服务器

```rust
let config = RustDeskConfig {
    id_server: "your-server.com:21116".to_string(),
    ..
};
```

### 端口说明

| 端口 | 协议 | 用途 |
|------|------|------|
| 21115 | TCP | NAT 类型测试 |
| 21116 | UDP | ID 注册和心跳 |
| 21116 | TCP | ID 查询 |
| 21117 | TCP | 中继服务器 |
| 21118 | TCP | WebSocket |
| 21119 | TCP | API 服务 |

---

## 性能优化

### 网络优化

1. **UDP 连接池**: 复用 UDP socket
2. **数据包批量处理**: 减少系统调用
3. **零拷贝**: 使用 `bytes::BytesMut`

### 异步优化

```rust
// 使用 tokio 多线程运行时
tokio = { features = ["rt-multi-thread", ...] }

// 并发处理多个连接
let handles: Vec<_> = connections
    .iter()
    .map(|conn| tokio::spawn(handle_connection(conn)))
    .collect();
```

---

## 调试

### 启用日志

```rust
// main.rs
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
```

### 查看日志

```bash
# 鸿蒙设备
hilog | grep HarmonyDesk

# 查看特定模块
hilog | grep "protocol\|rustdesk"
```

### Wireshark 抓包

```
# 过滤 UDP 端口 21116
udp.port == 21116

# 过滤特定 IP
ip.addr == 192.168.1.100
```

---

## 安全考虑

1. **密码哈希**: 使用 SHA-256 防止明文传输
2. **加密传输**: 未来可使用 AES 加密数据包
3. **认证机制**: 密码验证后才建立连接
4. **超时机制**: 防止资源耗尽攻击

---

## 参考资料

- [RustDesk 官方文档](https://rustdesk.com/docs/)
- [RustDesk 协议分析](https://github.com/rustdesk/rustdesk)
- [NAT 穿透技术](https://en.wikipedia.org/wiki/NAT_traversal)
- [H.264 编码标准](https://en.wikipedia.org/wiki/H.264/MPEG-4_AVC)

---

## Sources

- [Improve NAT Traversal in RustDesk #11979](https://github.com/rustdesk/rustdesk/discussions/11979)
- [RustDesk Server网络穿透方案](https://blog.csdn.net/gitblog_00338/article/details/151387491)
- [RustDesk Self-host Documentation](https://rustdesk.com/docs/en/self-host/)
- [NAT Loopback Issues](https://rustdesk.com/docs/en/self-host/nat-loopback-issues/)
