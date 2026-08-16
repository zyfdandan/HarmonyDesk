# HarmonyOS 真机调试日志获取指南

## 方法一：使用 DevEco Studio HiLog 窗口（推荐）

1. **打开 HiLog 窗口**
   - 在 DevEco Studio 中，点击 `View` > `Tool Windows` > `HiLog`
   - 或者使用快捷键（Mac: `Cmd + Shift + H`，Windows: `Ctrl + Shift + H`）

2. **过滤日志**
   - 在 HiLog 窗口的过滤框中输入应用包名或关键词
   - 例如：`HarmonyDesk` 或 `Session`
   - 可以按日志级别过滤：`Info`、`Warn`、`Error`

3. **查看实时日志**
   - 连接真机后，日志会实时显示
   - 可以清空日志：点击清除按钮

## 方法二：使用 hdc 命令行工具

### 1. 连接设备

```bash
# 查看已连接的设备
hdc list targets

# 如果设备未连接，使用 USB 连接或网络连接
# USB 连接：直接插 USB 线
# 网络连接：
hdc tconn 设备IP:8710
```

### 2. 查看日志

```bash
# 实时查看所有日志
hdc hilog

# 查看 HarmonyDesk 相关日志
hdc hilog | grep -i "HarmonyDesk\|Session"

# 查看特定标签的日志
hdc hilog -T HarmonyDesk

# 查看错误日志
hdc hilog | grep -i "error"

# 清除日志缓冲区
hdc hilog -r
```

### 3. 导出日志到文件

```bash
# 导出所有日志
hdc hilog > log.txt

# 导出并过滤
hdc hilog | grep -i "HarmonyDesk\|Session" > harmony_desk_log.txt
```

## 方法三：使用 hilog 命令（在设备上）

如果设备已 root 或开启了开发者选项：

```bash
# 通过 hdc shell 进入设备
hdc shell

# 在设备 shell 中查看日志
hilog | grep HarmonyDesk

# 查看最近的日志
hilog -T HarmonyDesk -G 100
```

## 方法四：在 UI 上显示调试信息

代码中已经添加了调试信息显示：
- 设备 ID 会显示在界面上（如果为空会显示红色）
- 连接状态会显示
- 连接日志会显示在界面上
- 错误信息会显示在界面上

## 常用日志命令速查

```bash
# 查看所有日志（实时）
hdc hilog

# 只查看应用日志
hdc hilog | grep "com.example.harmonydesk"

# 查看错误和警告
hdc hilog | grep -E "ERROR|WARN"

# 查看特定关键词
hdc hilog | grep "设备 ID\|deskId\|参数"

# 清除日志
hdc hilog -r

# 导出日志到文件
hdc hilog > /path/to/log.txt
```

## 调试技巧

1. **添加更多日志**
   - 在代码中使用 `console.info()`、`console.warn()`、`console.error()`
   - 这些日志会出现在 HiLog 中

2. **使用标签过滤**
   - 代码中使用 `console.info('[Session] ...')` 这样的标签
   - 在 HiLog 中可以用 `Session` 过滤

3. **查看参数传递**
   - 在 `aboutToAppear()` 中添加了参数日志
   - 查看日志中的 `[Session] Received params:` 和 `[Session] Parsed params`

4. **检查连接流程**
   - 查看 `[Session] Starting connection to:` 日志
   - 查看 `[Session] 开始连接到:` 日志
   - 查看连接步骤日志

## 常见问题

### 问题：看不到日志

**解决方案：**
1. 确认设备已连接：`hdc list targets`
2. 确认应用已安装并运行
3. 检查日志级别设置
4. 尝试清除日志后重新操作：`hdc hilog -r`

### 问题：日志太多

**解决方案：**
1. 使用过滤：`hdc hilog | grep "关键词"`
2. 在 DevEco Studio 中使用 HiLog 窗口的过滤功能
3. 只查看错误：`hdc hilog | grep ERROR`

### 问题：日志不实时

**解决方案：**
1. 确保使用 `hdc hilog`（不带参数）查看实时日志
2. 在 DevEco Studio 的 HiLog 窗口中查看（自动实时）

## 示例：查看连接问题

```bash
# 1. 清除旧日志
hdc hilog -r

# 2. 开始实时查看日志
hdc hilog | grep -i "session\|connect\|参数\|deskid"

# 3. 在应用中进行操作（点击连接）

# 4. 观察日志输出，应该能看到：
# [Session] Received params: {...}
# [Session] Parsed params - deskId: xxx
# [Session] Starting connection to: xxx
# [Session] 开始连接到: xxx
```

