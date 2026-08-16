/**
 * 日志收集器模块
 *
 * 在 HarmonyOS 真机上，env_logger 无法正常工作。
 * 这个模块提供了一个基于内存的日志收集器，
 * 可以从 ArkTS 层读取 Rust 层的日志和错误信息。
 */

use std::sync::Mutex;
use std::time::SystemTime;
use once_cell::sync::Lazy;

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// 日志条目
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// 全局日志收集器
static LOG_COLLECTOR: Lazy<Mutex<LogCollector>> = Lazy::new(|| {
    Mutex::new(LogCollector::new())
});

/// 日志收集器
pub struct LogCollector {
    entries: Vec<LogEntry>,
    max_entries: usize,
    error_message: Option<String>,
    panic_message: Option<String>,
}

impl LogCollector {
    /// 创建新的日志收集器
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(1000),
            max_entries: 1000,
            error_message: None,
            panic_message: None,
        }
    }

    /// 记录日志
    pub fn log(&mut self, level: LogLevel, message: String, file: Option<String>, line: Option<u32>) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.entries.push(LogEntry {
            timestamp,
            level,
            message,
            file,
            line,
        });

        // 限制日志数量
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// 获取所有日志
    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.entries.clone()
    }

    /// 获取日志字符串（便于在 ArkTS 中显示）
    pub fn get_logs_string(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                let level_str = match entry.level {
                    LogLevel::Error => "ERROR",
                    LogLevel::Warn => "WARN",
                    LogLevel::Info => "INFO",
                    LogLevel::Debug => "DEBUG",
                    LogLevel::Trace => "TRACE",
                };
                format!(
                    "[{}] {}",
                    level_str,
                    entry.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 设置错误信息
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message.clone());
        self.log(LogLevel::Error, message, None, None);
    }

    /// 获取错误信息
    pub fn get_error(&self) -> Option<String> {
        self.error_message.clone()
    }

    /// 设置 Panic 消息
    pub fn set_panic(&mut self, message: String) {
        self.panic_message = Some(message.clone());
        self.log(LogLevel::Error, format!("PANIC: {}", message), None, None);
    }

    /// 获取 Panic 消息
    pub fn get_panic(&self) -> Option<String> {
        self.panic_message.clone()
    }

    /// 清空日志
    pub fn clear(&mut self) {
        self.entries.clear();
        self.error_message = None;
    }
}

/// 获取全局日志收集器
pub fn get_log_collector() -> &'static Mutex<LogCollector> {
    &LOG_COLLECTOR
}

/// 记录错误日志
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        let message = format!($($arg)*);
        let collector = $crate::log_collector::get_log_collector();
        let mut guard = collector.lock().unwrap_or_else(|e| {
            e.into_inner()
        });
        guard.log($crate::log_collector::LogLevel::Error, message, Some(file!().to_string()), Some(line!()));
    };
}

/// 记录警告日志
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        let message = format!($($arg)*);
        let collector = $crate::log_collector::get_log_collector();
        let mut guard = collector.lock().unwrap_or_else(|e| {
            e.into_inner()
        });
        guard.log($crate::log_collector::LogLevel::Warn, message, None, None);
    };
}

/// 记录信息日志
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        let message = format!($($arg)*);
        let collector = $crate::log_collector::get_log_collector();
        let mut guard = collector.lock().unwrap_or_else(|e| {
            e.into_inner()
        });
        guard.log($crate::log_collector::LogLevel::Info, message, None, None);
    };
}

/// 记录调试日志
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        let message = format!($($arg)*);
        let collector = $crate::log_collector::get_log_collector();
        let mut guard = collector.lock().unwrap_or_else(|e| {
            e.into_inner()
        });
        guard.log($crate::log_collector::LogLevel::Debug, message, None, None);
    };
}
