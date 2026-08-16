# HarmonyDesk 开发文档

## 开发环境配置

### 1. 安装必要的工具

#### DevEco Studio

```bash
# 下载 DevEco Studio
# 访问: https://developer.huawei.com/consumer/cn/deveco-studio/

# 安装后，需要安装 HarmonyOS SDK
# 在 DevEco Studio 中: Settings > SDK
```

#### HarmonyOS NDK

```bash
# 在 DevEco Studio 的 SDK Manager 中安装 NDK

# 或手动下载并设置环境变量
export HARMONYOS_NDK_PATH=/path/to/ohos-ndk
```

#### Rust 工具链

```bash
# 安装 Rust (如果还没安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 重新加载 PATH
source $HOME/.cargo/env

# 添加鸿蒙目标
rustup target add aarch64-linux-ohos

# 验证安装
rustc --version
cargo --version
```

### 2. 配置 ohos-rs

```bash
# 安装 ohos-rs 工具
cargo install ohos-rs

# 验证安装
ohos-rs --version
```

## 项目结构详解

### ArkTS 层 (harmonyos/entry/src/main/ets/)

```
ets/
├── entryability/
│   └── EntryAbility.ets      # 应用入口点
└── pages/
    ├── Index.ets             # 主页面（连接界面）
    ├── RemoteDesktop.ets     # 远程桌面会话页面（待实现）
    └── Settings.ets          # 设置页面（待实现）
```

### Rust Native 层 (harmonyos/entry/ohos/rust/)

```
rust/
├── src/
│   ├── lib.rs               # FFI 绑定入口，导出给 ArkTS 的函数
│   ├── core.rs              # 核心管理逻辑
│   └── rustdesk/
│       └── mod.rs           # RustDesk 协议集成
├── Cargo.toml               # Rust 项目配置
└── build.rs                 # 构建脚本
```

## FFI 接口定义

### 导出的 Rust 函数

| 函数名 | 参数 | 返回值 | 说明 |
|--------|------|--------|------|
| `init` | 无 | `number` (0=成功, 1=已初始化) | 初始化模块 |
| `connect` | `deskId: string, password: string` | `number` (0=成功, 1=失败) | 连接到远程桌面 |
| `disconnect` | 无 | `void` | 断开所有连接 |
| `cleanup` | 无 | `void` | 清理资源 |
| `getConnectionStatus` | 无 | `number` (连接数) | 获取连接状态 |

### 在 ArkTS 中调用

```typescript
import nativeModule from 'libharmonydesk.so';

// 初始化
const initResult = nativeModule.init();
if (initResult !== 0) {
  console.error('初始化失败');
}

// 连接
const result = nativeModule.connect('desk-id', 'password');
if (result === 0) {
  console.log('连接成功');
}

// 获取状态
const count = nativeModule.getConnectionStatus();
console.log(`活跃连接数: ${count}`);

// 断开
nativeModule.disconnect();

// 清理
nativeModule.cleanup();
```

## 添加新功能

### 1. 添加新的 FFI 函数

#### 在 Rust 中实现

在 `src/lib.rs` 中添加:

```rust
#[ohos_napi::js_function(1)]
fn sendKeyEvent(mut env: Env, info: CallbackInfo) -> Result<JsValue> {
    let key_code: u32 = info.get(0)?.into_inner(&env)?;
    let pressed: bool = info.get(1)?.into_inner(&env)?;

    log::info!("Sending key event: code={}, pressed={}", key_code, pressed);

    // 实现你的逻辑...

    env.create_uint32(0).map(|v| v.into_raw())
}

// 在 exports 函数中导出
fn exports(exports: &mut Exports) -> Result<()> {
    // ... 其他导出
    exports.export("sendKeyEvent", sendKeyEvent)?;
    Ok(())
}
```

#### 在 ArkTS 中调用

```typescript
// 发送键盘事件
nativeModule.sendKeyEvent(65, true);  // 按下 A 键
nativeModule.sendKeyEvent(65, false); // 释放 A 键
```

### 2. 添加新的 UI 页面

1. 创建新的 ArkTS 文件:

```typescript
// pages/Settings.ets
@Entry
@Component
struct Settings {
  build() {
    Column() {
      Text('设置')
      // ... UI 实现
    }
  }
}
```

2. 在 `main_pages.json` 中注册:

```json
{
  "src": [
    "pages/Index",
    "pages/Settings"
  ]
}
```

3. 导航到新页面:

```typescript
router.pushUrl({ url: 'pages/Settings' });
```

## 调试技巧

### 查看 Rust 日志

```bash
# 实时查看 HarmonyDesk 相关日志
hilog | grep HarmonyDesk

# 查看所有级别的日志
hilog -T HarmonyDesk

# 清除日志
hilog -r
```

### 查看 ArkTS 日志

在 DevEco Studio 中:
1. 打开 `View > Tool Windows > HiLog`
2. 在过滤器中输入你的应用包名

### 断点调试

#### ArkTS 断点

在 DevEco Studio 中直接在 ArkTS 代码行号处点击设置断点。

#### Rust 调试（较复杂）

由于 Rust 编译为 .so，调试较为复杂。建议使用日志输出。

```rust
log::info!("信息日志");
log::warn!("警告日志");
log::error!("错误日志");
```

## 常见编译错误

### 错误: `error: linking with cc failed`

**原因**: 链接器配置不正确

**解决**: 检查 `.cargo/config.toml` 中的路径配置

### 错误: `error: unknown target aarch64-linux-ohos`

**原因**: 未添加鸿蒙编译目标

**解决**:
```bash
rustup target add aarch64-linux-ohos
```

### 错误: `cannot find -lohos_ndk`

**原因**: NDK 路径配置错误

**解决**: 检查 `HARMONYOS_NDK_PATH` 环境变量

## 性能优化建议

### Rust 层优化

1. **使用异步运行时**: 所有网络操作使用 Tokio 异步
2. **避免频繁的 FFI 调用**: 批量处理数据
3. **使用零拷贝**: 减少不必要的数据复制

### ArkTS 层优化

1. **使用虚拟列表**: 处理大量数据时使用 `LazyForEach`
2. **避免频繁刷新**: 合理使用状态更新
3. **图片优化**: 使用合适的图片格式和尺寸

## 测试

### Rust 单元测试

```bash
cd harmonyos/entry/ohos/rust

# 运行所有测试
cargo test

# 运行特定测试
cargo test test_connection_config

# 查看测试输出
cargo test -- --nocapture
```

### 集成测试

在 DevEco Studio 中:
1. 创建测试用例
2. 右键运行测试

## 发布流程

### 1. 生成 Release 版本

```bash
# 编译 Rust Release 版本
cd harmonyos/entry/ohos/rust
cargo build --target aarch64-linux-ohos --release
```

### 2. 在 DevEco Studio 中构建

1. 选择 `Build > Build App(s) / Hap(s) > Build Hap(s) > Release`
2. 生成的 HAP 文件在 `entry/build/outputs/` 目录

### 3. 签名

1. 在 `File > Project Structure > Signing Configs` 中配置签名
2. 或使用华为开发者服务自动签名

### 4. 上传到华为应用市场

1. 登录 [华为开发者联盟](https://developer.huawei.com/consumer/cn/)
2. 创建应用并上传 HAP 包
3. 填写应用信息和截图
4. 等待审核

## 资源链接

- [HarmonyOS 开发者文档](https://developer.huawei.com/consumer/cn/doc/)
- [ohos-rs 文档](https://ohos.rs/docs)
- [RustDesk 文档](https://rustdesk.com/docs/)
- [ArkTS 语言指南](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides-V5/arkts-get-started-V5)

## 许可证

本项目的所有代码贡献都将使用与项目相同的许可证 (AGPL-3.0)。
