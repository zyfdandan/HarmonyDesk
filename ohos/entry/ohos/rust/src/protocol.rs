/**
 * RustDesk 协议实现
 *
 * 这个模块实现了 RustDesk 的核心协议，包括：
 * - ID 服务器通信
 * - NAT 穿透（P2P 打洞）
 * - 加密握手
 * - 视频流接收
 * - 输入事件转发
 */

use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::net::SocketAddr;
use std::time::Duration;

/// 协议错误类型
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Handshake failed: {0}")]
    HandshakeFailed(String),

    #[error("Connection timeout")]
    Timeout,

    #[error("Invalid packet format")]
    InvalidPacket,

    #[error("Encryption error")]
    EncryptionError,

    #[error("Peer not found")]
    PeerNotFound,
}

/// RustDesk 协议消息类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    // 连接管理
    Handshake = 0x01,
    HandshakeResponse = 0x02,
    ConnectionRequest = 0x03,
    ConnectionResponse = 0x04,
    Disconnect = 0x05,

    // 视频相关
    VideoFrame = 0x10,
    VideoConfig = 0x11,
    KeepAlive = 0x12,

    // 输入事件
    KeyEvent = 0x20,
    MouseEvent = 0x21,
    ClipboardEvent = 0x22,

    // 其他
    Ping = 0xF0,
    Pong = 0xF1,
    Error = 0xFF,
}

impl TryFrom<u16> for MessageType {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, ProtocolError> {
        match value {
            0x01 => Ok(MessageType::Handshake),
            0x02 => Ok(MessageType::HandshakeResponse),
            0x03 => Ok(MessageType::ConnectionRequest),
            0x04 => Ok(MessageType::ConnectionResponse),
            0x05 => Ok(MessageType::Disconnect),
            0x10 => Ok(MessageType::VideoFrame),
            0x11 => Ok(MessageType::VideoConfig),
            0x12 => Ok(MessageType::KeepAlive),
            0x20 => Ok(MessageType::KeyEvent),
            0x21 => Ok(MessageType::MouseEvent),
            0x22 => Ok(MessageType::ClipboardEvent),
            0xF0 => Ok(MessageType::Ping),
            0xF1 => Ok(MessageType::Pong),
            0xFF => Ok(MessageType::Error),
            _ => Err(ProtocolError::InvalidPacket),
        }
    }
}

/// 协议数据包
#[derive(Debug, Clone)]
pub struct Packet {
    pub msg_type: MessageType,
    pub payload: Vec<u8>,
}

impl Packet {
    /// 数据包头部大小（类型 + 长度）
    const HEADER_SIZE: usize = 6;

    /// 创建新数据包
    pub fn new(msg_type: MessageType, payload: Vec<u8>) -> Self {
        Self { msg_type, payload }
    }

    /// 序列化数据包
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(Self::HEADER_SIZE + self.payload.len());
        buf.put_u16(self.msg_type as u16);
        buf.put_u32(self.payload.len() as u32);
        buf.extend_from_slice(&self.payload);
        buf.to_vec()
    }

    /// 从字节数据反序列化
    pub fn deserialize(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < Self::HEADER_SIZE {
            return Err(ProtocolError::InvalidPacket);
        }

        let mut buf = BytesMut::from(data);
        let type_val = buf.get_u16();
        let msg_type = MessageType::try_from(type_val)?;
        let len = buf.get_u32() as usize;

        if data.len() < Self::HEADER_SIZE + len {
            return Err(ProtocolError::InvalidPacket);
        }

        let payload = data[Self::HEADER_SIZE..Self::HEADER_SIZE + len].to_vec();
        Ok(Self { msg_type, payload })
    }
}

/// ID 服务器通信
pub struct IdServerClient {
    server_addr: String,
    local_id: String,
    socket: Option<UdpSocket>,
}

impl IdServerClient {
    /// 创建 ID 服务器客户端
    pub fn new(server_addr: String, local_id: String) -> Self {
        Self {
            server_addr,
            local_id,
            socket: None,
        }
    }

    /// 连接到 ID 服务器
    pub async fn connect(&mut self) -> Result<(), ProtocolError> {
        log::info!("=== ID 服务器连接开始 ===");
        log::info!("目标服务器: {}", self.server_addr);
        log::info!("本地 ID: {}", self.local_id);

        // 解析服务器地址
        let addr: SocketAddr = match self.server_addr.parse() {
            Ok(a) => {
                log::info!("服务器地址解析成功: {}", a);
                a
            }
            Err(e) => {
                log::error!("❌ 服务器地址解析失败: '{}' - 错误: {}", self.server_addr, e);
                return Err(ProtocolError::HandshakeFailed(format!(
                    "Invalid server address: '{}' - parse error: {}",
                    self.server_addr, e
                )));
            }
        };

        log::info!("服务器 IP: {}:{}", addr.ip(), addr.port());

        // 绑定本地端口
        let socket = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => {
                let local_addr = match s.local_addr() {
                    Ok(a) => a.to_string(),
                    Err(e) => format!("(获取失败: {})", e),
                };
                log::info!("✓ 本地 UDP socket 绑定成功: {}", local_addr);
                s
            }
            Err(e) => {
                log::error!("❌ 绑定本地 UDP socket 失败: {}", e);
                return Err(ProtocolError::HandshakeFailed(format!(
                    "Failed to bind local socket: {}", e
                )));
            }
        };

        // 连接到远程服务器
        log::info!("正在连接到 {}:{}...", addr.ip(), addr.port());
        match socket.connect(addr).await {
            Ok(_) => {
                log::info!("✓ 成功连接到 ID 服务器");
                if let Ok(local_addr) = socket.local_addr() {
                    log::info!("  本地地址: {}", local_addr);
                }
            }
            Err(e) => {
                log::error!("❌ 连接到 ID 服务器失败");
                log::error!("  目标地址: {}:{}", addr.ip(), addr.port());
                log::error!("  错误类型: {}", e);
                log::error!("  错误详情: {:?}", e.kind());

                // 检查是否是网络不可达
                if e.kind() == std::io::ErrorKind::NetworkUnreachable {
                    log::error!("  原因: 网络不可达 - 请检查网络连接");
                } else if e.kind() == std::io::ErrorKind::ConnectionRefused {
                    log::error!("  原因: 连接被拒绝 - 服务器可能不可用");
                } else if e.kind() == std::io::ErrorKind::TimedOut {
                    log::error!("  原因: 连接超时 - 网络延迟过高或防火墙阻止");
                }

                return Err(ProtocolError::HandshakeFailed(format!(
                    "Failed to connect to {}:{} - {}", addr.ip(), addr.port(), e)));
            }
        }

        self.socket = Some(socket);
        log::info!("=== ID 服务器连接成功 ===");

        Ok(())
    }

    /// 注册本地 ID
    pub async fn register_id(&self) -> Result<(), ProtocolError> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| ProtocolError::HandshakeFailed("Not connected".to_string()))?;

        // 构造注册包
        let mut payload = BytesMut::new();
        payload.put_u8(0x01); // 注册命令
        payload.put_u16(self.local_id.len() as u16);
        payload.extend_from_slice(self.local_id.as_bytes());

        let packet = Packet::new(MessageType::Handshake, payload.to_vec());
        let data = packet.serialize();

        socket.send(&data).await?;

        log::info!("Registered ID: {}", self.local_id);
        Ok(())
    }

    /// 请求连接到远程 ID
    pub async fn request_connection(&self, remote_id: &str) -> Result<SocketAddr, ProtocolError> {
        log::info!("=== 请求远程设备信息 ===");
        log::info!("远程设备 ID: {}", remote_id);

        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| {
                log::error!("❌ 未连接到 ID 服务器");
                ProtocolError::HandshakeFailed("Not connected to ID server".to_string())
            })?;

        // 构造连接请求包
        let mut payload = BytesMut::new();
        payload.put_u8(0x02); // 连接请求命令
        payload.put_u16(remote_id.len() as u16);
        payload.extend_from_slice(remote_id.as_bytes());

        let packet = Packet::new(MessageType::ConnectionRequest, payload.to_vec());
        let data = packet.serialize();

        log::info!("发送连接请求到 ID 服务器 ({} 字节)", data.len());

        match socket.send(&data).await {
            Ok(n) => log::info!("✓ 发送成功 ({} 字节)", n),
            Err(e) => {
                log::error!("❌ 发送失败: {}", e);
                return Err(ProtocolError::Io(e));
            }
        }

        // 等待响应
        log::info!("等待 ID 服务器响应 (超时 10 秒)...");
        let mut buf = vec![0u8; 1024];
        let timeout = tokio::time::timeout(Duration::from_secs(10), socket.recv(&mut buf)).await;

        match timeout {
            Ok(Ok(n)) => {
                log::info!("✓ 收到响应 ({} 字节)", n);

                let response = match Packet::deserialize(&buf[..n]) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("❌ 解析响应包失败: {}", e);
                        return Err(ProtocolError::HandshakeFailed(format!(
                            "Failed to parse response: {}", e
                        )));
                    }
                };

                log::info!("响应消息类型: {:?}", response.msg_type);

                if response.msg_type != MessageType::ConnectionResponse {
                    log::error!("❌ 意外的响应类型: {:?}", response.msg_type);
                    return Err(ProtocolError::HandshakeFailed(
                        format!("Unexpected response type: {:?}", response.msg_type),
                    ));
                }

                // 解析对端地址
                let mut data = BytesMut::from(&response.payload[..]);
                let status = data.get_u8();

                if status != 0 {
                    log::error!("❌ 远程设备未找到 (状态码: {})", status);
                    log::error!("  设备 ID: {}", remote_id);
                    log::error!("  可能的原因:");
                    log::error!("    1. 设备 ID 不存在");
                    log::error!("    2. 设备离线");
                    log::error!("    3. 设备未在 RustDesk 网络中注册");
                    return Err(ProtocolError::PeerNotFound);
                }

                log::info!("✓ 远程设备找到");

                // 简化：返回服务器地址，实际会进行 NAT 穿透
                let peer_addr = socket.peer_addr()?;
                log::info!("对端地址: {}", peer_addr);

                Ok(peer_addr)
            }
            Ok(Err(e)) => {
                log::error!("❌ 接收响应时发生 IO 错误: {}", e);
                Err(ProtocolError::Io(e))
            }
            Err(_) => {
                log::error!("❌ 等待响应超时 (10 秒)");
                log::error!("  可能的原因:");
                log::error!("    1. 网络延迟过高");
                log::error!("    2. ID 服务器负载过高");
                log::error!("    3. 防火墙阻止了响应");
                Err(ProtocolError::Timeout)
            }
        }
    }

    /// 心跳保活
    pub async fn send_heartbeat(&self) -> Result<(), ProtocolError> {
        if let Some(socket) = &self.socket {
            let packet = Packet::new(MessageType::KeepAlive, vec![]);
            let data = packet.serialize();
            socket.send(&data).await?;
        }
        Ok(())
    }
}

/// NAT 穿透管理器
pub struct NatTraversal {
    local_socket: Option<UdpSocket>,
    peer_addr: Option<SocketAddr>,
}

impl NatTraversal {
    pub fn new() -> Self {
        Self {
            local_socket: None,
            peer_addr: None,
        }
    }

    /// 执行 P2P 打洞
    pub async fn punch_hole(&mut self, peer_addr: SocketAddr) -> Result<(), ProtocolError> {
        log::info!("Starting NAT hole punching to: {}", peer_addr);

        // 绑定本地 UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let local_addr = socket.local_addr()?;

        log::info!("Local UDP bound to: {}", local_addr);

        // 发送多个打洞包
        for i in 0..5 {
            let packet = Packet::new(MessageType::Ping, format!("punch_{}", i).into_bytes());
            let data = packet.serialize();

            socket.send_to(&data, peer_addr).await?;
            log::debug!("Sent punch packet {} to {}", i + 1, peer_addr);

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        self.local_socket = Some(socket);
        self.peer_addr = Some(peer_addr);

        log::info!("NAT hole punching completed");
        Ok(())
    }

    /// 等待对端连接
    pub async fn wait_for_connection(&self) -> Result<(), ProtocolError> {
        let socket = self
            .local_socket
            .as_ref()
            .ok_or_else(|| ProtocolError::HandshakeFailed("Socket not initialized".to_string()))?;

        let mut buf = vec![0u8; 4096];
        let timeout = Duration::from_secs(30);

        let result = tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await;

        match result {
            Ok(Ok((n, addr))) => {
                log::info!("Received packet from: {}, size: {}", addr, n);
                let packet = Packet::deserialize(&buf[..n])?;

                if packet.msg_type == MessageType::Pong {
                    log::info!("Successfully established P2P connection with {}", addr);
                    Ok(())
                } else {
                    Err(ProtocolError::HandshakeFailed(
                        "Unexpected packet type".to_string(),
                    ))
                }
            }
            Ok(Err(e)) => Err(ProtocolError::Io(e)),
            Err(_) => Err(ProtocolError::Timeout),
        }
    }
}

/// 安全握手（使用简化的加密）
pub struct SecureHandshake {
    shared_secret: Option<Vec<u8>>,
}

impl SecureHandshake {
    pub fn new() -> Self {
        Self {
            shared_secret: None,
        }
    }

    /// 执行握手
    pub async fn perform_handshake(
        &mut self,
        socket: &mut UdpSocket,
        peer_addr: SocketAddr,
        password: &str,
    ) -> Result<(), ProtocolError> {
        log::info!("Starting secure handshake with {}", peer_addr);

        // 简化的握手：发送密码哈希
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(b"RustDesk");
        let password_hash = hasher.finalize();

        // 构造握手包
        let mut payload = BytesMut::new();
        payload.put_u16(password_hash.len() as u16);
        payload.extend_from_slice(&password_hash);

        let packet = Packet::new(MessageType::Handshake, payload.to_vec());
        let data = packet.serialize();

        socket.send_to(&data, peer_addr).await?;

        // 等待握手响应
        let mut buf = vec![0u8; 1024];
        let timeout = Duration::from_secs(10);

        let result = tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await;

        match result {
            Ok(Ok((n, addr))) => {
                let response = Packet::deserialize(&buf[..n])?;

                if response.msg_type == MessageType::HandshakeResponse {
                    // 检查响应状态
                    if !response.payload.is_empty() && response.payload[0] == 0 {
                        log::info!("Handshake successful");

                        // 存储共享密钥（简化：使用密码哈希）
                        self.shared_secret = Some(password_hash.to_vec());
                        Ok(())
                    } else {
                        Err(ProtocolError::HandshakeFailed(
                            "Authentication failed".to_string(),
                        ))
                    }
                } else {
                    Err(ProtocolError::HandshakeFailed(
                        "Invalid handshake response".to_string(),
                    ))
                }
            }
            Ok(Err(e)) => Err(ProtocolError::Io(e)),
            Err(_) => Err(ProtocolError::Timeout),
        }
    }

    /// 加密数据（简化实现）
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        // 简化：实际应使用 AES 等加密算法
        Ok(data.to_vec())
    }

    /// 解密数据（简化实现）
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        // 简化：实际应使用 AES 等解密算法
        Ok(data.to_vec())
    }
}

/// 视频流接收器
pub struct VideoStreamReceiver {
    frame_sender: mpsc::Sender<VideoFrame>,
}

/// 视频帧
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

impl VideoStreamReceiver {
    pub fn new() -> (Self, mpsc::Receiver<VideoFrame>) {
        let (sender, receiver) = mpsc::channel(100);
        (Self { frame_sender: sender }, receiver)
    }

    /// 处理视频数据包
    pub fn handle_packet(&self, packet: &Packet) -> Result<(), ProtocolError> {
        if packet.msg_type == MessageType::VideoFrame {
            // 简化的视频帧解析
            let mut data = BytesMut::from(&packet.payload[..]);

            if data.len() < 12 {
                return Err(ProtocolError::InvalidPacket);
            }

            let width = data.get_u32();
            let height = data.get_u32();
            let timestamp = data.get_u64();
            let frame_data = data.to_vec();

            let frame = VideoFrame {
                width,
                height,
                data: frame_data,
                timestamp,
            };

            // 发送到接收通道
            let _ = self.frame_sender.try_send(frame);
        }

        Ok(())
    }
}

/// 输入事件发送器
pub struct InputEventSender {
    socket: Arc<UdpSocket>,
    peer_addr: SocketAddr,
}

impl InputEventSender {
    pub fn new(socket: Arc<UdpSocket>, peer_addr: SocketAddr) -> Self {
        Self { socket, peer_addr }
    }

    /// 发送键盘事件
    pub async fn send_key_event(&self, key: u32, pressed: bool) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::new();
        payload.put_u32(key);
        payload.put_u8(pressed as u8);

        let packet = Packet::new(MessageType::KeyEvent, payload.to_vec());
        let data = packet.serialize();

        self.socket.send_to(&data, self.peer_addr).await?;
        Ok(())
    }

    /// 发送鼠标移动
    pub async fn send_mouse_move(&self, x: i32, y: i32) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::new();
        payload.put_i32(x);
        payload.put_i32(y);

        let packet = Packet::new(MessageType::MouseEvent, payload.to_vec());
        let data = packet.serialize();

        self.socket.send_to(&data, self.peer_addr).await?;
        Ok(())
    }

    /// 发送鼠标点击
    pub async fn send_mouse_click(&self, button: u32, pressed: bool) -> Result<(), ProtocolError> {
        let mut payload = BytesMut::new();
        payload.put_u32(button);
        payload.put_u8(pressed as u8);

        let packet = Packet::new(MessageType::MouseEvent, payload.to_vec());
        let data = packet.serialize();

        self.socket.send_to(&data, self.peer_addr).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_serialization() {
        let packet = Packet::new(MessageType::Ping, vec![1, 2, 3, 4]);
        let data = packet.serialize();
        let decoded = Packet::deserialize(&data).unwrap();

        assert_eq!(packet.msg_type, decoded.msg_type);
        assert_eq!(packet.payload, decoded.payload);
    }
}
