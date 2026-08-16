# HarmonyDesk 快速开始

## 5 分钟快速测试

### 前置要求检查

```bash
# 1. 检查 Rust 安装
rustc --version

# 2. 检查 DevEco Studio 安装
# 确保已安装 DevEco Studio 4.0+

# 3. 检查 HarmonyOS SDK
# 在 DevEco Studio -> Settings -> SDK 中确认
```

### 快速启动

#### 步骤 1: 构建 Rust 模块

```bash
cd harmonyos/entry/ohos/rust

# Linux/macOS
./build.sh

# Windows (使用 PowerShell)
# .\build.ps1
```

#### 步骤 2: 在 DevEco Studio 中打开项目

1. 启动 DevEco Studio
2. File -> Open
3. 选择 `harmonyos` 目录
4. 等待项目索引完成

#### 步骤 3: 运行应用

1. 启动模拟器 (Device Manager)
2. 点击 Run 按钮 (▶) 或按 Shift+F10
3. 应用将在模拟器中启动

### 测试连接界面

1. 应用启动后，你会看到连接界面
2. 在 "Desk ID" 输入框中输入任意 ID (例如: `123456789`)
3. 在 "Password" 输入框中输入任意密码 (例如: `test123`)
4. 点击 "连接" 按钮

**注意**: 当前版本使用测试模式，不会连接真实服务器，但会测试:
- UI 交互
- FFI 调用
- 页面导航
- 视频帧生成和显示

### 进入远程桌面页面

如果连接成功，应用将导航到远程桌面页面，你会看到:
- 视频显示区域 (显示测试图案)
- 控制栏 (断开连接、缩放控制)
- 性能监控信息 (FPS)

### 常见问题

#### 构建失败: "target not found"

```bash
rustup target add aarch64-linux-ohos
```

#### OHOS SDK 未找到

```bash
# 设置 OHOS_NATIVE_HOME 环境变量
export OHOS_NATIVE_HOME=/path/to/ohos-sdk/native

# macOS 示例
export OHOS_NATIVE_HOME=~/HarmonyOS/Sdk/ohos-sdk/native
```

#### 应用无法安装

1. 检查签名配置 (Project Structure -> Signing Configs)
2. 确认模拟器 API Level 与项目配置一致
3. 查看 DevEco Studio 的完整错误信息

### 下一步

完成基础测试后，查看 [测试指南](TESTING.md) 了解:
- 完整的测试流程
- 调试技巧
- 性能测试方法
- 真实连接配置

### 项目结构

```
HarmonyDesk/
├── harmonyos/              # HarmonyOS 项目
│   ├── entry/              # 应用模块
│   │   ├── ohos/rust/     # Rust Native 模块
│   │   │   ├── src/       # Rust 源代码
│   │   │   ├── build.sh   # 构建脚本
│   │   │   └── Cargo.toml # Rust 依赖配置
│   │   └── src/main/ets/  # ArkTS 源代码
│   │       ├── pages/     # 页面组件
│   │       └── services/  # 服务层
│   └── AppScope/          # 应用全局配置
├── docs/                  # 文档目录
├── TESTING.md            # 测试指南
├── DEVELOPMENT.md        # 开发指南
└── README.md             # 项目说明
```

### 技术栈

- **UI**: ArkTS (TypeScript 扩展)
- **核心逻辑**: Rust
- **FFI**: ohos-napi
- **视频解码**: H.264 (软件解码)
- **渲染**: PixelMap
- **异步运行时**: Tokio

### 参与贡献

欢迎提交 Issue 和 Pull Request!

### 许可证

MIT License
