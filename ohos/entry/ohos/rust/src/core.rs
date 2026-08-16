/**
 * 核心功能模块
 * 提供与 ArkTS 层交互的核心 API
 */

use crate::rustdesk::{RustDeskConfig, RustDeskConnection, RustDeskVideoStream};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub id_server: Option<String>,
    pub relay_server: Option<String>,
    pub force_relay: bool,
    pub key: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            id_server: None,
            relay_server: None,
            force_relay: false,
            key: None,
        }
    }
}

/// 会话信息
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub connected: bool,
    pub screen_width: u32,
    pub screen_height: u32,
}

/// 核心管理器
pub struct CoreManager {
    connections: Arc<Mutex<HashMap<String, Arc<Mutex<RustDeskConnection>>>>>,
    video_streams: Arc<Mutex<HashMap<String, RustDeskVideoStream>>>,
    server_config: Arc<Mutex<ServerConfig>>,
}

impl CoreManager {
    /// 创建新的核心管理器
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            video_streams: Arc::new(Mutex::new(HashMap::new())),
            server_config: Arc::new(Mutex::new(ServerConfig::default())),
        }
    }

    /// 更新服务器配置
    pub async fn update_server_config(&self, config: ServerConfig) {
        let mut stored = self.server_config.lock().await;
        *stored = config;
    }

    async fn build_config(&self, desk_id: &str, password: &str) -> RustDeskConfig {
        let base = RustDeskConfig::default();
        let settings = self.server_config.lock().await.clone();
        RustDeskConfig {
            desk_id: desk_id.to_string(),
            password: if password.is_empty() {
                None
            } else {
                Some(password.to_string())
            },
            id_server: settings.id_server.unwrap_or(base.id_server),
            relay_server: settings.relay_server.or(base.relay_server),
            force_relay: settings.force_relay,
        }
    }

    /// 连接到远程桌面
    pub async fn connect(&self, desk_id: &str, password: &str) -> Result<SessionInfo, String> {
        log::info!("CoreManager: Connecting to {}", desk_id);

        // 检查是否已存在连接
        {
            let conns = self.connections.lock().await;
            if conns.contains_key(desk_id) {
                return Ok(SessionInfo {
                    id: desk_id.to_string(),
                    connected: true,
                    screen_width: 1920,
                    screen_height: 1080,
                });
            }
        }

        let config = self.build_config(desk_id, password).await;

        // 创建连接
        let mut connection = RustDeskConnection::new(config);
        connection.connect().await?;

        // 存储连接
        let connection = Arc::new(Mutex::new(connection));
        let mut conns = self.connections.lock().await;
        conns.insert(desk_id.to_string(), connection.clone());

        // 启动视频流
        let mut video_stream = RustDeskVideoStream::new(connection);
        video_stream.start().await?;
        let mut streams = self.video_streams.lock().await;
        streams.insert(desk_id.to_string(), video_stream);

        Ok(SessionInfo {
            id: desk_id.to_string(),
            connected: true,
            screen_width: 1920,
            screen_height: 1080,
        })
    }

    /// 断开指定连接
    pub async fn disconnect(&self, desk_id: &str) -> Result<(), String> {
        log::info!("CoreManager: Disconnecting {}", desk_id);

        // 停止视频流
        {
            let mut streams = self.video_streams.lock().await;
            if let Some(mut stream) = streams.remove(desk_id) {
                stream.stop().await?;
            }
        }

        // 断开连接
        let mut conns = self.connections.lock().await;
        if let Some(conn) = conns.remove(desk_id) {
            let mut conn = conn.lock().await;
            conn.disconnect().await?;
        }

        Ok(())
    }

    /// 断开所有连接
    pub async fn disconnect_all(&self) -> Result<(), String> {
        log::info!("CoreManager: Disconnecting all");

        let desk_ids: Vec<String> = {
            let conns = self.connections.lock().await;
            conns.keys().cloned().collect()
        };

        for desk_id in desk_ids {
            self.disconnect(&desk_id).await?;
        }

        Ok(())
    }

    /// 发送键盘事件
    pub async fn send_key(&self, desk_id: &str, key: u32, pressed: bool) -> Result<(), String> {
        let conns = self.connections.lock().await;
        if let Some(conn) = conns.get(desk_id) {
            let conn = conn.lock().await;
            conn.send_key_event(key, pressed).await?;
        }
        Ok(())
    }

    /// 发送鼠标事件
    pub async fn send_mouse_move(&self, desk_id: &str, x: i32, y: i32) -> Result<(), String> {
        let conns = self.connections.lock().await;
        if let Some(conn) = conns.get(desk_id) {
            let conn = conn.lock().await;
            conn.send_mouse_move(x, y).await?;
        }
        Ok(())
    }

    /// 发送鼠标点击
    pub async fn send_mouse_click(
        &self,
        desk_id: &str,
        button: u32,
        pressed: bool,
    ) -> Result<(), String> {
        let conns = self.connections.lock().await;
        if let Some(conn) = conns.get(desk_id) {
            let conn = conn.lock().await;
            conn.send_mouse_click(button, pressed).await?;
        }
        Ok(())
    }

    /// 获取连接列表
    pub async fn get_connections(&self) -> Vec<SessionInfo> {
        let conns = self.connections.lock().await;
        conns
            .keys()
            .map(|id| SessionInfo {
                id: id.clone(),
                connected: true,
                screen_width: 1920,
                screen_height: 1080,
            })
            .collect()
    }
}

impl Default for CoreManager {
    fn default() -> Self {
        Self::new()
    }
}
