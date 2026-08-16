# HarmonyDesk UI 组件文档

本文档详细说明了 HarmonyDesk 的 UI 组件和使用方法。

## 目录

- [页面架构](#页面架构)
- [主页面 (Index)](#主页面-index)
- [远程桌面页面 (RemoteDesktop)](#远程桌面页面-remotedesktop)
- [FFI API](#ffi-api)
- [样式规范](#样式规范)
- [性能优化](#性能优化)

---

## 页面架构

```
HarmonyDesk UI
├── Index.ets              # 主页面（连接界面）
│   ├── 输入框（桌面 ID）
│   ├── 输入框（密码）
│   ├── 连接按钮
│   └── 功能按钮（连接后显示）
│
└── RemoteDesktop.ets      # 远程桌面会话页面
    ├── 视频显示区域
    ├── 顶部状态栏
    └── 底部控制栏
        ├── 缩放控制
        └── 断开按钮
```

---

## 主页面 (Index)

### 功能

- 远程桌面 ID 输入
- 密码输入（可选）
- 连接管理
- 状态显示

### 状态变量

```typescript
@State deskId: string              // 远程桌面 ID
@State password: string            // 密码
@State isConnected: boolean        // 连接状态
@State errorMessage: string         // 错误消息
@State connectionStatus: string     // 连接状态文本
```

### 主要方法

#### `connectToRemoteDesk()`

连接到远程桌面。

```typescript
async connectToRemoteDesk() {
  // 1. 验证输入
  if (!this.deskId) {
    this.errorMessage = '请输入远程桌面 ID';
    return;
  }

  // 2. 调用 Rust 模块连接
  const result = nativeModule.connect(this.deskId, this.password);

  // 3. 处理结果
  if (result === 0) {
    // 连接成功，跳转到远程桌面页面
    router.pushUrl({
      url: 'pages/RemoteDesktop',
      params: {
        deskId: this.deskId,
        password: this.password
      }
    });
  } else {
    this.errorMessage = `连接失败: 错误码 ${result}`;
  }
}
```

#### `disconnect()`

断开连接。

```typescript
disconnect() {
  nativeModule.disconnect();
  this.isConnected = false;
  this.connectionStatus = '未连接';
}
```

### UI 布局

```
┌─────────────────────────────────┐
│                                 │
│         HarmonyDesk             │
│                                 │
│     ● 已连接 / 未连接           │
│                                 │
│  远程桌面 ID                     │
│  ┌───────────────────────────┐  │
│  │ 请输入远程桌面 ID         │  │
│  └───────────────────────────┘  │
│                                 │
│  密码                           │
│  ┌───────────────────────────┐  │
│  │ 请输入密码（可选）        │  │
│  └───────────────────────────┘  │
│                                 │
│  ┌───────────────────────────┐  │
│  │        连接               │  │
│  └───────────────────────────┘  │
│                                 │
│  ┌───────────────────────────┐  │
│  │    打开文件传输           │  │
│  ├───────────────────────────┤  │
│  │    打开会话设置           │  │
│  └───────────────────────────┘  │
│                                 │
│  基于 RustDesk 协议...          │
└─────────────────────────────────┘
```

---

## 远程桌面页面 (RemoteDesktop)

### 功能

- 显示远程桌面画面
- 处理触摸输入
- 缩放和平移
- 性能监控

### 状态变量

```typescript
@State deskId: string                 // 远程桌面 ID
@State password: string               // 密码
@State isConnected: boolean           // 连接状态
@State scale: number                  // 缩放比例
@State offsetX: number                // X 偏移
@State offsetY: number                // Y 偏移
@State videoWidth: number             // 视频宽度
@State videoHeight: number            // 视频高度
@State frameRate: number              // 帧率
@State showControls: boolean          // 控制栏显示状态
```

### 主要方法

#### `connectToDesktop()`

连接到远程桌面。

```typescript
async connectToDesktop() {
  const result = nativeModule.connect(this.deskId, this.password);

  if (result === 0) {
    this.isConnected = true;
    this.startVideoStream();
  } else {
    this.errorMessage = `连接失败: 错误码 ${result}`;
  }
}
```

#### `handleTouch(event: TouchEvent)`

处理触摸事件并转发到远程桌面。

```typescript
handleTouch(event: TouchEvent) {
  const touch = event.touches[0];
  const x = Math.floor(touch.screenX / this.scale);
  const y = Math.floor(touch.screenY / this.scale);

  switch (event.type) {
    case TouchType.Down:
      this.sendMouseEvent(x, y, 1, true);  // 左键按下
      break;
    case TouchType.Move:
      this.sendMouseEvent(x, y, undefined, undefined);  // 鼠标移动
      break;
    case TouchType.Up:
      this.sendMouseEvent(x, y, 1, false);  // 左键释放
      break;
  }
}
```

#### `sendKeyEvent(keyCode: number, pressed: boolean)`

发送键盘事件到远程桌面。

```typescript
sendKeyEvent(keyCode: number, pressed: boolean) {
  if (!this.isConnected) return;
  nativeModule.sendKeyEvent(keyCode, pressed);
}
```

#### `sendMouseEvent(x, y, button, pressed)`

发送鼠标事件到远程桌面。

```typescript
sendMouseEvent(x: number, y: number, button: number, pressed: boolean) {
  if (!this.isConnected) return;

  if (button !== undefined) {
    nativeModule.sendMouseClick(button, pressed);
  } else {
    nativeModule.sendMouseMove(x, y);
  }
}
```

### 手势支持

#### 触摸手势

```typescript
TouchGesture()
  .onAction((event: TouchEvent) => {
    this.handleTouch(event);
  })
```

#### 缩放手势

```typescript
PinchGesture({ fingers: 2 })
  .onActionUpdate((event: GestureEvent) => {
    if (event.scale) {
      this.scale = Math.max(0.5, Math.min(3.0, event.scale));
    }
  })
```

### UI 布局

```
┌─────────────────────────────────┐
│ ← remote-desk-id     30 FPS     │ ← 顶部状态栏
│                                 │
│                                 │
│                                 │
│                                 │
│     ┌───────────────────┐       │
│     │                   │       │
│     │   Remote Screen   │       │
│     │                   │       │
│     └───────────────────┘       │
│                                 │
│  ─  ─  ─             断开       │ ← 底部控制栏
└─────────────────────────────────┘
```

### 控制栏

控制栏在点击画面时显示，3 秒后自动隐藏。

**顶部状态栏**:
- 返回按钮
- 远程桌面 ID
- 性能统计（FPS、延迟）

**底部控制栏**:
- 缩小按钮
- 放大按钮
- 重置视图按钮
- 断开连接按钮

---

## FFI API

### 导出函数

| 函数名 | 参数 | 返回值 | 说明 |
|--------|------|--------|------|
| `init` | 无 | `number` | 初始化模块 |
| `connect` | `deskId: string, password: string` | `number` | 连接到远程桌面 |
| `disconnect` | 无 | `void` | 断开所有连接 |
| `cleanup` | 无 | `void` | 清理资源 |
| `getConnectionStatus` | 无 | `number` | 获取活跃连接数 |
| `sendKeyEvent` | `keyCode: number, pressed: boolean` | `void` | 发送键盘事件 |
| `sendMouseMove` | `x: number, y: number` | `void` | 发送鼠标移动 |
| `sendMouseClick` | `button: number, pressed: boolean` | `void` | 发送鼠标点击 |

### 使用示例

```typescript
import nativeModule from 'libharmonydesk.so';

// 初始化
nativeModule.init();

// 连接
const result = nativeModule.connect('remote-id', 'password');
if (result === 0) {
  console.log('连接成功');
}

// 发送键盘事件
nativeModule.sendKeyEvent(0x1E, true);  // 按下 W 键
nativeModule.sendKeyEvent(0x1E, false); // 释放 W 键

// 发送鼠标事件
nativeModule.sendMouseMove(100, 200);
nativeModule.sendMouseClick(1, true);   // 左键按下
nativeModule.sendMouseClick(1, false);  // 左键释放

// 断开
nativeModule.disconnect();
```

---

## 样式规范

### 颜色方案

```typescript
// 主色调
const PRIMARY_COLOR = '#2196F3';    // 蓝色
const SUCCESS_COLOR = '#4CAF50';    // 绿色
const WARNING_COLOR = '#FF9800';    // 橙色
const ERROR_COLOR = '#F44336';      // 红色

// 中性色
const TEXT_PRIMARY = '#333';        // 主要文本
const TEXT_SECONDARY = '#666';      // 次要文本
const BACKGROUND = '#F5F5F5';       // 背景色
const DIVIDER = '#E0E0E0';          // 分隔线
```

### 字体大小

```typescript
const FONT_SIZE_LARGE = 28;         // 标题
const FONT_SIZE_MEDIUM = 16;        // 正文
const FONT_SIZE_SMALL = 14;         // 说明
const FONT_SIZE_TINY = 12;          // 辅助信息
```

### 间距规范

```typescript
const SPACING_LARGE = 24;           // 大间距
const SPACING_MEDIUM = 16;          // 中间距
const SPACING_SMALL = 8;            // 小间距
const SPACING_TINY = 4;             // 微间距
```

---

## 性能优化

### 1. 视频渲染优化

```typescript
// 使用 GPU 加速
Image(pixelMap)
  .useGPU(true)
  .renderMode(ImageRenderMode.Original)

// 降低渲染频率
@State frameSkip: number = 2;  // 每 2 帧渲染一次
```

### 2. 事件节流

```typescript
// 鼠标移动节流
private mouseMoveTimer: number = -1;

sendMouseMoveThrottled(x: number, y: number) {
  if (this.mouseMoveTimer !== -1) {
    clearTimeout(this.mouseMoveTimer);
  }

  this.mouseMoveTimer = setTimeout(() => {
    this.sendMouseEvent(x, y, undefined, undefined);
  }, 16);  // 60 FPS
}
```

### 3. 内存管理

```typescript
// 释放资源
aboutToDisappear() {
  if (this.controlsTimer !== -1) {
    clearTimeout(this.controlsTimer);
  }

  if (this.mouseMoveTimer !== -1) {
    clearTimeout(this.mouseMoveTimer);
  }
}
```

### 4. 状态优化

```typescript
// 使用 @Observed 裂变嵌套对象
@Observed
class VideoState {
  width: number = 1920;
  height: number = 1080;
  // ...
}

// 使用 @ObjectLink 避免不必要的更新
@ObjectLink videoState: VideoState;
```

---

## 调试

### 查看日志

```typescript
console.info('信息日志');
console.warn('警告日志');
console.error('错误日志');
```

### 性能监控

```typescript
// 帧率监控
@State frameCount: number = 0;
@State fps: number = 0;

private lastFrameTime: number = Date.now();

updateFrame() {
  this.frameCount++;

  const now = Date.now();
  const elapsed = now - this.lastFrameTime;

  if (elapsed >= 1000) {
    this.fps = Math.round(this.frameCount * 1000 / elapsed);
    this.frameCount = 0;
    this.lastFrameTime = now;
  }
}
```

---

## 最佳实践

### 1. 错误处理

```typescript
try {
  const result = nativeModule.connect(this.deskId, this.password);
  // 处理结果
} catch (error) {
  console.error(`操作失败: ${error.message}`);
  this.errorMessage = `操作失败: ${error.message}`;
}
```

### 2. 状态验证

```typescript
// 检查连接状态后再执行操作
sendEventOnlyIfConnected() {
  if (!this.isConnected) {
    console.warn('未连接，无法发送事件');
    return;
  }

  // 执行操作
}
```

### 3. 资源清理

```typescript
aboutToDisappear() {
  // 清理定时器
  clearTimeout(this.timer);

  // 断开连接
  if (this.isConnected) {
    nativeModule.disconnect();
  }
}
```

---

## 相关文档

- [PROTOCOL.md](PROTOCOL.md) - 协议实现文档
- [DEVELOPMENT.md](DEVELOPMENT.md) - 开发指南
- [README.md](README.md) - 项目说明
