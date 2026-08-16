/**
 * HarmonyDesk - Rust Native Module
 * 基于 RustDesk 核心的鸿蒙远程桌面控制端
 */

#[macro_use]
extern crate napi_derive_ohos;

use napi_ohos::{CallContext, Env, Error, JsObject, Result};
use napi_ohos::bindgen_prelude::{Null, Object, ToNapiValue, Unknown};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::panic;

mod rustdesk;
mod core;
mod protocol;
mod video;
mod log_collector;

use core::{CoreManager, ServerConfig};
use video::{DecodedFrame, PixelFormat};
use log_collector::get_log_collector;

// 全局核心管理器
static CORE_MANAGER: Mutex<Option<Arc<CoreManager>>> = Mutex::new(None);
static SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);
static SERVER_CONFIG: Mutex<ServerConfig> = Mutex::new(ServerConfig {
    id_server: None,
    relay_server: None,
    force_relay: false,
    key: None,
});

// 设置 Panic Hook
fn init_panic_hook() {
    let previous_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            format!("Panic: {}", s)
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            format!("Panic: {}", s)
        } else if let Some(location) = panic_info.location() {
            format!("Panic at {}:{} - {}",
                location.file(),
                location.line(),
                panic_info.to_string())
        } else {
            format!("Panic: {}", panic_info.to_string())
        };

        // 保存到日志收集器
        let collector = get_log_collector();
        let mut guard = collector.lock().unwrap_or_else(|e| e.into_inner());
        guard.set_panic(message.clone());

        // 调用之前的 hook
        previous_hook(panic_info);
    }));
}

// 初始化模块
#[js_function(0)]
fn init(_ctx: CallContext) -> Result<u32> {
    // 初始化 panic hook
    init_panic_hook();

    log_info!("Initializing HarmonyDesk native module");

    let mut manager = CORE_MANAGER.lock()
        .map_err(|e| {
            log_error!("Lock error: {}", e);
            Error::from_reason("Lock error")
        })?;

    if manager.is_some() {
        log_warn!("Module already initialized");
        return Ok(1);
    }

    *manager = Some(Arc::new(CoreManager::new()));

    log_info!("HarmonyDesk native module initialized successfully");
    Ok(0)
}

// 初始化调试模块
#[js_function(0)]
fn init_debug(_ctx: CallContext) -> Result<u32> {
    init_panic_hook();
    log_info!("Debug mode initialized");
    Ok(0)
}

// 获取所有日志
#[js_function(0)]
fn get_logs(ctx: CallContext) -> Result<Unknown> {
    let collector = get_log_collector();
    let guard = collector.lock().unwrap_or_else(|e| e.into_inner());
    let logs_string = guard.get_logs_string();

    Ok(ctx.env.create_string_from_std(logs_string)?.into_unknown(&*ctx.env)?)
}

// 获取最后一条错误信息
#[js_function(0)]
fn get_last_error(ctx: CallContext) -> Result<Unknown> {
    let collector = get_log_collector();
    let guard = collector.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(error) = guard.get_error() {
        Ok(ctx.env.create_string_from_std(error)?.into_unknown(&*ctx.env)?)
    } else if let Some(panic) = guard.get_panic() {
        Ok(ctx.env.create_string_from_std(panic)?.into_unknown(&*ctx.env)?)
    } else {
        Null.into_unknown(&*ctx.env)
    }
}

// 清空日志
#[js_function(0)]
fn clear_logs(_ctx: CallContext) -> Result<()> {
    let collector = get_log_collector();
    let mut guard = collector.lock().unwrap_or_else(|e| e.into_inner());
    guard.clear();
    Ok(())
}

// 设置服务器配置
#[js_function(4)]
fn set_server_config(ctx: CallContext) -> Result<u32> {
    let id_server: String = ctx.get(0)?;
    let relay_server: String = ctx.get(1)?;
    let force_relay: bool = ctx.get(2)?;
    let key: String = ctx.get(3)?;

    let manager = CORE_MANAGER.lock()
        .map_err(|e| {
            log_error!("Lock error: {}", e);
            Error::from_reason("Failed to acquire lock")
        })?;

    if manager.is_none() {
        log_error!("Module not initialized");
        return Err(Error::from_reason("Module not initialized. Call init() first."));
    }

    let config = ServerConfig {
        id_server: if id_server.is_empty() { None } else { Some(id_server.clone()) },
        relay_server: if relay_server.is_empty() { None } else { Some(relay_server.clone()) },
        force_relay,
        key: if key.is_empty() { None } else { Some(key) },
    };

    if let Ok(mut stored) = SERVER_CONFIG.lock() {
        *stored = config;
    }

    log_info!("Server config set: id_server={}, relay_server={}, force_relay={}",
        if id_server.is_empty() { "none" } else { &id_server },
        if relay_server.is_empty() { "none" } else { &relay_server },
        force_relay);

    Ok(0)
}

// 连接到远程桌面
#[js_function(2)]
fn connect(ctx: CallContext) -> Result<u32> {
    let desk_id: String = ctx.get(0)?;
    let password: String = ctx.get(1)?;

    log_info!("Connecting to remote desk: {}", desk_id);

    let manager = CORE_MANAGER.lock()
        .map_err(|e| {
            log_error!("Failed to acquire lock: {}", e);
            Error::from_reason("Failed to acquire lock")
        })?;

    let manager = manager.as_ref()
        .ok_or_else(|| {
            log_error!("Module not initialized");
            Error::from_reason("Module not initialized. Call init() first.")
        })?;

    let demo = desk_id.starts_with("DEMO") || desk_id.starts_with("TEST");
    if demo {
        SESSION_ACTIVE.store(true, Ordering::SeqCst);
        log_info!("Demo session ready for {}, skip network protocol", desk_id);
        return Ok(0);
    }

    let manager = manager.clone();
    let desk_id_clone = desk_id.clone();
    let password_clone = password.clone();
    SESSION_ACTIVE.store(true, Ordering::SeqCst);

    let spawn_result = std::thread::Builder::new()
        .name("hd-connect".into())
        .spawn(move || {
            log_info!("Background connect thread started for {}", desk_id_clone);
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    log_error!("Failed to create runtime: {}", e);
                    let collector = get_log_collector();
                    let mut guard = collector.lock().unwrap_or_else(|e| e.into_inner());
                    guard.set_error(format!("Failed to create runtime: {}", e));
                    return;
                }
            };
            let result = rt.block_on(async move {
                manager.connect(&desk_id_clone, &password_clone).await
            });
            match result {
                Ok(session) => {
                    log_info!("Background connection ok: {:?}", session);
                }
                Err(e) => {
                    log_error!("Background connection failed: {}", e);
                    let collector = get_log_collector();
                    let mut guard = collector.lock().unwrap_or_else(|e| e.into_inner());
                    guard.set_error(format!("Connection failed: {}", e));
                }
            }
        });

    if let Err(e) = spawn_result {
        log_error!("Failed to spawn connect thread: {}", e);
        SESSION_ACTIVE.store(false, Ordering::SeqCst);
        return Ok(1);
    }

    log_info!("Connect started in background for {}", desk_id);
    Ok(0)
}

// 断开所有连接
#[js_function(0)]
fn disconnect(_ctx: CallContext) -> Result<()> {
    log_info!("Disconnecting all remote desks");
    SESSION_ACTIVE.store(false, Ordering::SeqCst);
    log_info!("Session marked disconnected");
    Ok(())
}

// 清理资源
#[js_function(0)]
fn cleanup(_ctx: CallContext) -> Result<()> {
    log_info!("Cleaning up HarmonyDesk native module");

    let mut manager = CORE_MANAGER.lock()
        .map_err(|e| {
            log_error!("Lock error: {}", e);
            Error::from_reason("Failed to acquire lock")
        })?;

    SESSION_ACTIVE.store(false, Ordering::SeqCst);
    *manager = None;

    log_info!("Cleanup completed");
    Ok(())
}

// 获取连接状态（返回活跃连接数）
#[js_function(0)]
fn get_connection_status(_ctx: CallContext) -> Result<u32> {
    if SESSION_ACTIVE.load(Ordering::SeqCst) {
        Ok(1)
    } else {
        Ok(0)
    }
}

// 发送键盘事件
#[js_function(2)]
fn send_key_event(ctx: CallContext) -> Result<()> {
    let key_code: u32 = ctx.get(0)?;
    let pressed: bool = ctx.get(1)?;

    log_debug!("Sending key event: key={}, pressed={}", key_code, pressed);
    Ok(())
}

// 发送鼠标移动
#[js_function(2)]
fn send_mouse_move(ctx: CallContext) -> Result<()> {
    let x: i32 = ctx.get(0)?;
    let y: i32 = ctx.get(1)?;

    log_debug!("Sending mouse move: x={}, y={}", x, y);
    Ok(())
}

// 发送鼠标点击
#[js_function(2)]
fn send_mouse_click(ctx: CallContext) -> Result<()> {
    let button: u32 = ctx.get(0)?;
    let pressed: bool = ctx.get(1)?;

    log_debug!("Sending mouse click: button={}, pressed={}", button, pressed);
    Ok(())
}

// 获取视频帧数据（返回 RGBA 格式的像素数据）
#[js_function(0)]
fn get_video_frame(ctx: CallContext) -> Result<Unknown> {
    if !SESSION_ACTIVE.load(Ordering::SeqCst) {
        return Null.into_unknown(&*ctx.env);
    }

    let frame = create_test_frame(320, 180);
    let data = frame.data;

    let mut array_buffer = ctx.env.create_arraybuffer(data.len())?;
    array_buffer.as_mut().copy_from_slice(&data);
    let array_buffer = array_buffer.into_raw();

    let mut obj = ctx.env.create_object()?;
    obj.set_named_property("width", frame.width)?;
    obj.set_named_property("height", frame.height)?;
    obj.set_named_property("data", array_buffer)?;
    obj.set_named_property("timestamp", frame.timestamp)?;

    Ok(obj.into_unknown())
}

// 创建测试帧（用于开发调试）
fn create_test_frame(width: u32, height: u32) -> DecodedFrame {
    let mut frame = DecodedFrame::new(width, height, PixelFormat::RGBA);

    // 生成渐变测试图案
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;

            // 创建渐变
            let r = (x * 255 / width) as u8;
            let g = (y * 255 / height) as u8;
            let b = 128;

            // 添加棋盘格效果
            let block_size = 64;
            let is_dark = ((x / block_size) + (y / block_size)) % 2 == 0;

            let multiplier = if is_dark { 0.7 } else { 1.0 };

            frame.data[idx] = (r as f32 * multiplier) as u8;
            frame.data[idx + 1] = (g as f32 * multiplier) as u8;
            frame.data[idx + 2] = (b as f32 * multiplier) as u8;
            frame.data[idx + 3] = 255; // Alpha
        }
    }

    // 在中心添加时间戳区域
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    frame.timestamp = timestamp;

    frame
}

// 导出模块
#[module_exports]
fn init_module(mut exports: JsObject, _env: Env) -> Result<()> {
    exports.create_named_method("init", init)?;
    exports.create_named_method("initDebug", init_debug)?;
    exports.create_named_method("setServerConfig", set_server_config)?;
    exports.create_named_method("connect", connect)?;
    exports.create_named_method("disconnect", disconnect)?;
    exports.create_named_method("cleanup", cleanup)?;
    exports.create_named_method("getConnectionStatus", get_connection_status)?;
    exports.create_named_method("sendKeyEvent", send_key_event)?;
    exports.create_named_method("sendMouseMove", send_mouse_move)?;
    exports.create_named_method("sendMouseClick", send_mouse_click)?;
    exports.create_named_method("getVideoFrame", get_video_frame)?;
    // 调试函数
    exports.create_named_method("getLogs", get_logs)?;
    exports.create_named_method("getLastError", get_last_error)?;
    exports.create_named_method("clearLogs", clear_logs)?;
    Ok(())
}
