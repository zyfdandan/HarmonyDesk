# HarmonyDesk 测试指南

本文档介绍如何测试 HarmonyDesk 应用。

## 前置条件

1. **DevEco Studio**: 安装最新版本的 DevEco Studio (建议 4.0+)
   - 下载地址: https://developer.harmonyos.com/cn/develop/deveco-studio

2. **HarmonyOS SDK**: 在 DevEco Studio 中安装 HarmonyOS SDK
   - API Level 9+ (建议 API 10 或更高)

3. **Rust 工具链**: 需要 Rust 编译器
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

4. **HarmonyOS 目标平台**:
   ```bash
   rustup target add aarch64-linux-ohos
   ```

## 测试环境设置

### 1. 打开项目

在 DevEco Studio 中打开项目:
```bash
# 在 DevEco Studio 中选择 File -> Open
# 选择 harmonyos 目录
```

### 2. 构建 Rust Native 模块

在终端中导航到 Rust 模块目录并构建:

```bash
cd harmonyos/entry/ohos/rust

# 设置交叉编译环境
export PATH=/path/to/ohos-sdk/native/bin:$PATH
export OHOS_NATIVE_HOME=/path/to/ohos-sdk/native

# 构建 aarch64-linux-ohos 目标
cargo build --target aarch64-linux-ohos --release

# 或者使用提供的构建脚本
./build.sh
```

### 3. 配置 DevEco Studio

1. **SDK 配置**:
   - File -> Settings -> SDK
   - 确保 HarmonyOS SDK 已安装

2. **签名配置** (用于真机测试):
   - File -> Project Structure -> Signing Configs
   - 自动签名或手动配置签名证书

### 4. 运行模拟器

1. 在 DevEco Studio 中打开 Device Manager
2. 创建一个新的模拟器:
   - 选择设备类型 (Phone 或 Tablet)
   - 选择 API Level (建议 API 10+)
   - 启动模拟器

## 测试步骤

### 第一阶段: UI 测试

#### 1. 启动应用

- 在 DevEco Studio 中点击 Run 按钮 (或按 Shift+F10)
- 选择目标设备 (模拟器或真机)
- 等待应用安装并启动

#### 2. 测试连接界面

在 Index 页面测试:
- [ ] 应用成功启动并显示连接界面
- [ ] 输入框可以正常输入
- [ ] "连接" 按钮可以点击
- [ ] UI 布局在不同屏幕尺寸下正常显示

#### 3. 测试 FFI 调用

使用测试模式连接 (不依赖实际服务器):
- Desk ID: 输入任意 ID
- Password: 输入任意密码
- 观察控制台输出，检查 FFI 调用是否成功

### 第二阶段: 视频解码测试

#### 1. 进入远程桌面页面

- 使用测试凭证通过连接验证
- 成功导航到 RemoteDesktop 页面

#### 2. 测试视频帧显示

- [ ] 视频显示区域正确渲染
- [ ] 能够看到测试图案 (渐变色 + 棋盘格)
- [ ] PixelMap 正确转换和显示
- [ ] 触摸事件能够正常捕获

#### 3. 测试控制栏功能

- [ ] 断开连接按钮可点击
- [ ] 缩放控制 (+ / -) 正常工作
- [ ] 性能监控显示帧率信息

### 第三阶段: 真实连接测试 (需要 RustDesk 服务器)

#### 1. 设置 ID 服务器

在 `harmonyos/entry/ohos/rust/src/lib.rs` 中配置:
```rust
const ID_SERVER: &str = "your-rustdesk-server.com:21114";
```

#### 2. 连接到真实桌面

- 输入真实的 RustDesk Desk ID
- 输入正确的密码
- 观察连接流程:
  1. ID 服务器连接
  2. NAT 穿透
  3. 安全握手
  4. 视频流接收
  5. H.264 解码
  6. 视频显示

#### 3. 测试输入控制

- [ ] 触摸事件正确映射为鼠标移动
- [ ] 点击事件正确发送
- [ ] 键盘输入 (虚拟键盘) 正常工作

## 调试技巧

### 查看 ArkTS 日志

在 DevEco Studio 的 HiLog 窗口中查看应用日志:
```typescript
console.log('Debug info:', data);
console.error('Error:', error);
```

### 查看 Rust 日志

当前 Rust 日志会通过 FFI 返回到 ArkTS，未来可以添加:
```rust
use log::info;
info!("Connection established");
```

### 常见问题

1. **编译错误**:
   - 检查 Rust 目标平台是否正确安装: `rustup target list`
   - 确认 OHOS SDK 路径配置正确

2. **FFI 调用失败**:
   - 检查 .so 文件是否正确生成: `entry/ohos/rust/target/aarch64-linux-ohos/release/liblib.so`
   - 确认 module.json5 中的 nativeLibraryPath 配置正确

3. **视频显示问题**:
   - 检查 PixelMap 创建参数
   - 确认 RGBA 数据格式正确
   - 查看 HiLog 中的错误信息

4. **应用安装失败**:
   - 检查签名配置
   - 确认设备 API Level 兼容性
   - 查看完整的错误日志

## 性能测试

使用内置的性能监控功能:
- 观察帧率 (FPS) 显示
- 检查解码延迟
- 监控内存使用情况

## 下一步

完成基础测试后，可以继续实现:
1. 真实的 H.264 解码器 (目前是测试模式)
2. 键盘输入支持
3. 多显示器支持
4. 文件传输功能
5. 剪贴板同步

## 相关文档

- [开发指南](DEVELOPMENT.md)
- [协议说明](PROTOCOL.md)
- [UI 组件](UI_COMPONENTS.md)
- [视频解码](VIDEO_DECODER.md)
