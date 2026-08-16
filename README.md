# HarmonyDesk

基于 [RustDesk](https://github.com/rustdesk/rustdesk) 协议的 **HarmonyOS NEXT** 远程桌面**控制端**（手机连电脑）。

> 本仓库只包含控制端客户端，不含被控端。在鸿蒙手机上侧载 HAP 后，可连接自建或兼容 RustDesk 协议的被控设备。

## 功能概览

- 自建 hbbs / hbbr 服务器（ID + 中继 + Key）
- 设备 ID 连接，或 **IP 直连**（默认端口 `21118`）
- H.264 / H.265 硬解画面（Native Window / XComponent）
- 触摸板、滚轮、屏幕键盘（含 Ctrl+Alt+Del、Alt+F4、显示桌面 Win+D）
- 短线自动重连、会话保活
- 统一画质档（custom q=100 / 60fps）

## 仓库结构

```
HarmonyDesk/
├── ohos/                         # 主工程（DevEco 打开此目录）
│   ├── AppScope/
│   ├── entry/
│   │   ├── src/main/
│   │   │   ├── ets/              # ArkTS UI / 业务
│   │   │   ├── cpp/              # NAPI + 视频解码
│   │   │   └── resources/
│   │   ├── ohos/
│   │   │   ├── hdcore/           # Rust 协议核心 (libhdcore.so)
│   │   │   └── rust/             # 辅助 Rust 模块
│   │   └── libs/arm64-v8a/       # 编译产物 .so（不入库）
│   ├── build-profile.json5       # 无签名密钥的公共配置
│   └── build-profile.json5.example
├── scripts/
│   ├── build-native.ps1          # 交叉编译 libhdcore.so
│   ├── build-hap.bat             # hvigor 打 HAP
│   └── wait-and-install.ps1
└── README.md
```

历史目录 `harmonyos/` 为早期骨架，**请以 `ohos/` 为准**。

## 环境要求

| 组件 | 说明 |
|------|------|
| DevEco Studio | 建议 5.0+，能签 HarmonyOS 调试包 |
| HarmonyOS SDK | API 15+（HarmonyOS 6）或 API 26（HarmonyOS 7） |
| Rust | `stable` + target `aarch64-unknown-linux-ohos` |
| OHOS NDK | 含 `llvm/bin/clang`，供 Rust 交叉编译 |
| Windows | 本仓库脚本以 Windows / PowerShell 为主 |

## 快速开始

### 1. 配置签名（必做）

仓库**不包含**调试证书与密码。任选其一：

1. 用 DevEco 打开 `ohos/` → **File → Project Structure → Signing Configs** → 勾选自动签名（华为账号）。
2. 或复制示例后自行填写路径：

```bat
copy ohos\build-profile.json5.example ohos\build-profile.json5
```

然后编辑 `ohos/build-profile.json5` 中的 `certpath` / `storeFile` / 密码字段。

### 2. 编译 Rust 核心库

设置 NDK 路径后执行：

```powershell
$env:OHOS_NATIVE_HOME = "C:\path\to\ohos-sdk\native"
powershell -ExecutionPolicy Bypass -File .\scripts\build-native.ps1
```

产物：`ohos/entry/libs/arm64-v8a/libhdcore.so`。

### 3. 编译并安装 HAP

```bat
scripts\build-hap.bat
hdc install ohos\entry\build\default\outputs\default\entry-default-signed.hap
```

设备需开启「开发者模式 / USB 调试」，且调试证书已绑定该机 UDID。

### 4. 配置服务器

应用内 **设置** 填写自建 RustDesk 服务：

| 项 | 示例（请换成你自己的） |
|----|------------------------|
| ID 服务器 hbbs | `your.domain.com:21116` |
| 中继 hbbr | `your.domain.com:21117` |
| Key | 与 hbbs 启动参数 `-k` 一致 |

局域网也可在「新建连接」直接填被控端 IP（默认 `21118`）。

**请勿把真实服务器地址、Key、远程密码提交到 Git。**

## 安全说明

已从公开仓库中剥离：

- 本机调试签名证书路径与密钥口令
- 自建服务器 IP / Key / 设备密码等偏好导出
- 设备 hilog、崩溃 dump、截图等调试产物

本地若仍有签名备份文件 `ohos/build-profile.signing.local.json5`，该文件已被 `.gitignore` 忽略，请勿上传。

## 架构简图

```
ArkTS (Index / Session / Store)
        │  NAPI
        ▼
libharmonydesk.so  ──►  video_decoder (OH AVCodec)
        │  dlopen
        ▼
libhdcore.so (Rust) ──► hbbs/hbbr / IP:21118 ──► 对端 RustDesk
```

## 相关文档

仓库内另有协议与开发笔记（部分路径可能仍写旧的 `harmonyos/`，以本文为准）：

- [PROTOCOL.md](./PROTOCOL.md) — 协议与端口
- [DEVELOPMENT.md](./DEVELOPMENT.md) — 开发备忘
- [VIDEO_DECODER.md](./VIDEO_DECODER.md) — 解码相关

## 许可证与致谢

- 协议兼容 [RustDesk](https://github.com/rustdesk/rustdesk) 开源生态
- 本项目衍生自社区 HarmonyDesk 工程，面向 HarmonyOS NEXT 控制端场景继续演进

使用前请遵守当地法律与被控设备所有者的授权要求。
