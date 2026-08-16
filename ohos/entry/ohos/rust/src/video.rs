/**
 * H.264 视频解码器
 *
 * 负责解码 H.264 编码的视频流
 * 支持软件解码（使用 openh264）和硬件加速
 */

use bytes::BytesMut;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 解码错误类型
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Decoder not initialized")]
    NotInitialized,

    #[error("Invalid frame data: {0}")]
    InvalidFrame(String),

    #[error("Decode failed: {0}")]
    DecodeFailed(String),

    #[error("Buffer overflow")]
    BufferOverflow,
}

/// 视频帧信息
#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
}

/// 像素格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// RGBA 32-bit
    RGBA,
    /// RGB 24-bit
    RGB,
    /// YUV420P
    YUV420P,
}

/// 解码后的视频帧
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub format: PixelFormat,
    pub timestamp: u64,
}

impl DecodedFrame {
    /// 创建新的解码帧
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let data_size = match format {
            PixelFormat::RGBA => width * height * 4,
            PixelFormat::RGB => width * height * 3,
            PixelFormat::YUV420P => (width * height * 3) / 2,
        };

        Self {
            width,
            height,
            data: vec![0u8; data_size as usize],
            format,
            timestamp: 0,
        }
    }

    /// 获取帧大小（字节）
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// 获取 RGBA 数据（如果格式不是 RGBA，会转换）
    pub fn to_rgba(&self) -> Result<Vec<u8>, DecodeError> {
        match self.format {
            PixelFormat::RGBA => Ok(self.data.clone()),
            PixelFormat::RGB => {
                // RGB -> RGBA 转换
                let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
                for chunk in self.data.chunks_exact(3) {
                    rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                }
                Ok(rgba)
            }
            PixelFormat::YUV420P => {
                // YUV420P -> RGBA 转换（简化实现）
                // 实际应使用更高效的转换算法
                self.yuv420p_to_rgba()
            }
        }
    }

    /// YUV420P 转 RGBA（简化实现）
    fn yuv420p_to_rgba(&self) -> Result<Vec<u8>, DecodeError> {
        let y_size = (self.width * self.height) as usize;
        let uv_size = y_size / 4;

        if self.data.len() < y_size + uv_size * 2 {
            return Err(DecodeError::InvalidFrame("Invalid YUV420P data".to_string()));
        }

        let y_plane = &self.data[0..y_size];
        let u_plane = &self.data[y_size..y_size + uv_size];
        let v_plane = &self.data[y_size + uv_size..y_size + uv_size * 2];

        let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);

        for i in 0..self.height {
            for j in 0..self.width {
                let y_idx = (i * self.width + j) as usize;
                let uv_idx = (i / 2 * self.width / 2 + j / 2) as usize;

                let y = y_plane[y_idx] as f32;
                let u = u_plane[uv_idx] as f32 - 128.0;
                let v = v_plane[uv_idx] as f32 - 128.0;

                // YUV 到 RGB 转换
                let r = (y + 1.402 * v).round().clamp(0.0, 255.0) as u8;
                let g = (y - 0.344136 * u - 0.714136 * v).round().clamp(0.0, 255.0) as u8;
                let b = (y + 1.772 * u).round().clamp(0.0, 255.0) as u8;

                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }

        Ok(rgba)
    }
}

/// H.264 解码器配置
#[derive(Debug, Clone)]
pub struct DecoderConfig {
    pub width: u32,
    pub height: u32,
    pub enable_hardware_acceleration: bool,
    pub thread_count: usize,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            enable_hardware_acceleration: false,
            thread_count: 4,
        }
    }
}

/// H.264 解码器
pub struct H264Decoder {
    config: DecoderConfig,
    initialized: bool,
    frame_count: u64,
    // software_decoder: Option<openh264::Decoder>, // 后续启用
}

impl H264Decoder {
    /// 创建新的解码器
    pub fn new(config: DecoderConfig) -> Self {
        Self {
            config,
            initialized: false,
            frame_count: 0,
        }
    }

    /// 初始化解码器
    pub fn initialize(&mut self) -> Result<(), DecodeError> {
        log::info!("Initializing H.264 decoder: {}x{}", self.config.width, self.config.height);

        // TODO: 初始化实际的 openh264 解码器
        // self.software_decoder = Some(openh264::Decoder::new()?);

        self.initialized = true;
        log::info!("H.264 decoder initialized successfully");
        Ok(())
    }

    /// 解码 H.264 NAL 单元
    pub fn decode_nal(&mut self, nal_data: &[u8]) -> Result<Option<DecodedFrame>, DecodeError> {
        if !self.initialized {
            return Err(DecodeError::NotInitialized);
        }

        // TODO: 使用实际的 H.264 解码器
        // 当前返回模拟帧用于测试

        // 简化实现：检测关键帧（帧类型 0x67 或 0x65）
        let is_key_frame = nal_data.len() > 4 &&
            (nal_data[4] == 0x67 || nal_data[4] == 0x65);

        if is_key_frame {
            log::trace!("Detected key frame, size: {}", nal_data.len());

            // 创建模拟帧
            let mut frame = DecodedFrame::new(self.config.width, self.config.height, PixelFormat::RGBA);

            // 生成测试图案（棋盘格）
            self.generate_test_pattern(&mut frame.data, self.config.width, self.config.height);

            frame.timestamp = self.frame_count;
            self.frame_count += 1;

            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    /// 解码完整的视频帧
    pub fn decode_frame(&mut self, frame_data: &[u8]) -> Result<DecodedFrame, DecodeError> {
        if !self.initialized {
            return Err(DecodeError::NotInitialized);
        }

        log::trace!("Decoding frame: {} bytes", frame_data.len());

        // TODO: 实际的 H.264 解码
        // 当前返回模拟帧
        let mut frame = DecodedFrame::new(self.config.width, self.config.height, PixelFormat::RGBA);
        self.generate_test_pattern(&mut frame.data, self.config.width, self.config.height);
        frame.timestamp = self.frame_count;
        self.frame_count += 1;

        Ok(frame)
    }

    /// 刷新解码器缓冲区
    pub fn flush(&mut self) -> Result<Option<DecodedFrame>, DecodeError> {
        if !self.initialized {
            return Err(DecodeError::NotInitialized);
        }

        // TODO: 刷新解码器缓冲区
        Ok(None)
    }

    /// 获取解码器信息
    pub fn get_info(&self) -> FrameInfo {
        FrameInfo {
            width: self.config.width,
            height: self.config.height,
            stride: self.config.width,
            format: PixelFormat::RGBA,
        }
    }

    /// 生成测试图案（用于开发调试）
    fn generate_test_pattern(&self, data: &mut [u8], width: u32, height: u32) {
        let block_size = 64;
        let mut color_index = 0;

        // 测试图案颜色
        let colors = [
            [0x1E, 0x88, 0xE5, 0xFF], // 蓝色
            [0x43, 0xA0, 0x47, 0xFF], // 绿色
            [0xFF, 0x98, 0x00, 0xFF], // 橙色
            [0xE9, 0x1E, 0x63, 0xFF], // 红色
        ];

        for y in 0..height {
            for x in 0..width {
                let block_x = (x / block_size) as usize % colors.len();
                let block_y = (y / block_size) as usize % colors.len();
                color_index = (block_x + block_y) % colors.len();

                let idx = ((y * width + x) * 4) as usize;
                if idx + 4 <= data.len() {
                    data[idx..idx + 4].copy_from_slice(&colors[color_index]);
                }
            }
        }

        // 在中心显示 "HarmonyDesk" 文字（简化为白色矩形）
        let center_x = width / 2 - 100;
        let center_y = height / 2 - 20;
        for y in center_y..center_y + 40 {
            for x in center_x..center_x + 200 {
                let idx = (y * width + x) as usize * 4;
                if idx + 4 <= data.len() {
                    data[idx..idx + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }

    /// 重置解码器
    pub fn reset(&mut self) -> Result<(), DecodeError> {
        log::info!("Resetting decoder");
        self.initialized = false;
        self.frame_count = 0;
        Ok(())
    }
}

/// 视频帧缓冲区
pub struct FrameBuffer {
    frames: Vec<DecodedFrame>,
    max_size: usize,
}

impl FrameBuffer {
    /// 创建新的帧缓冲区
    pub fn new(max_size: usize) -> Self {
        Self {
            frames: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// 添加帧到缓冲区
    pub fn push(&mut self, frame: DecodedFrame) {
        if self.frames.len() >= self.max_size {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }

    /// 获取最新帧
    pub fn get_latest(&self) -> Option<&DecodedFrame> {
        self.frames.last()
    }

    /// 获取指定索引的帧
    pub fn get(&self, index: usize) -> Option<&DecodedFrame> {
        self.frames.get(index)
    }

    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// 获取帧数量
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_creation() {
        let config = DecoderConfig::default();
        let mut decoder = H264Decoder::new(config);

        decoder.initialize().unwrap();
        assert!(decoder.initialized);
    }

    #[test]
    fn test_frame_creation() {
        let frame = DecodedFrame::new(1920, 1080, PixelFormat::RGBA);
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.size(), 1920 * 1080 * 4);
    }

    #[test]
    fn test_frame_buffer() {
        let mut buffer = FrameBuffer::new(3);

        let frame1 = DecodedFrame::new(800, 600, PixelFormat::RGBA);
        let frame2 = DecodedFrame::new(800, 600, PixelFormat::RGBA);

        buffer.push(frame1);
        assert_eq!(buffer.len(), 1);

        buffer.push(frame2);
        assert_eq!(buffer.len(), 2);

        assert!(buffer.get_latest().is_some());
    }
}
