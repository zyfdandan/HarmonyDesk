# HarmonyDesk（鸿蒙远程桌面控制端）

基于 [RustDesk](https://github.com/rustdesk/rustdesk) 协议的 **HarmonyOS NEXT** 远程桌面**控制端**。  
在鸿蒙手机上侧载安装后，可连接运行 RustDesk 的电脑（或兼容协议的被控端）。

> **说明**：本仓库只提供「控制端」客户端，不包含被控端服务。请仅在你有权管理的设备上使用。

---

## 项目简介

HarmonyDesk 面向纯血鸿蒙（HarmonyOS NEXT）手机，把 RustDesk 控制能力做到可侧载的 HAP 应用里：

- 用手机查看并操作远程电脑桌面
- 支持自建 ID / 中继服务器，也支持局域网 IP 直连
- 画面走系统硬解，输入支持触摸板与屏幕键盘常用快捷键

适合：Mate / Pura 等 NEXT 机型作为移动控制端，连接家里或办公室的 Windows 等被控机。

---

## 主要功能

| 功能 | 说明 |
|------|------|
| 自建服务器 | 配置 hbbs（ID）、hbbr（中继）与 Key |
| 设备列表 | 保存常用设备 ID、名称与密码（仅存本机） |
| ID 连接 | 通过 ID 服务器查询并建立会话 |
| IP 直连 | 直接填写被控端 IP（默认端口 `21118`） |
| 视频解码 | H.264 / H.265 硬解，XComponent 出图 |
| 触控 | 触摸板模式、左右键、可拖拽滚轮 |
| 键盘 | 文本/密码输入，中英切换，Ctrl+Alt+Del、Alt+F4、显示桌面 |
| 稳定性 | 短线自动重连、会话保活 |

---

## 技术架构

```
┌─────────────────────────────────────┐
│  ArkTS 界面（设备列表 / 会话 / 设置） │
└─────────────────┬───────────────────┘
                  │ NAPI
┌─────────────────▼───────────────────┐
│  libharmonydesk.so（C++）             │
│  · 视频硬解（OH AVCodec）             │
│  · 加载并调用 Rust 核心               │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│  libhdcore.so（Rust）                 │
│  · RustDesk 协议 / 打洞与中继         │
│  · 登录、键鼠、画质选项               │
└─────────────────┬───────────────────┘
                  │
         hbbs / hbbr 或 IP:21118
                  │
            对端 RustDesk 被控端
```

技术栈概览：ArkTS UI · Rust 协议核心 · C++ NAPI / 解码 · HarmonyOS NEXT。

---

## 目录结构

```
dandan / HarmonyDesk
├── ohos/                          # 主工程（请用 DevEco 打开此目录）
│   ├── AppScope/                  # 应用级配置与图标
│   ├── entry/
│   │   ├── src/main/ets/          # 页面与业务（Index、Session 等）
│   │   ├── src/main/cpp/          # NAPI、视频解码
│   │   ├── ohos/hdcore/           # Rust 协议核心源码
│   │   └── libs/arm64-v8a/        # 编译出的 .so（不入库，需本地编译）
│   ├── build-profile.json5        # 公共构建配置（不含签名密钥）
│   └── build-profile.json5.example
├── scripts/
│   ├── build-native.ps1           # 交叉编译 libhdcore.so
│   ├── build-hap.bat              # 打包 HAP
│   └── wait-and-install.ps1
└── README.md                      # 本说明
```

仓库里若还有 `harmonyos/` 旧目录，仅为早期骨架，**请以 `ohos/` 为准**。

---

## 环境要求

| 环境 | 建议 |
|------|------|
| 开发工具 | DevEco Studio 5.0+（可自动签名） |
| SDK | HarmonyOS API 15+（鸿蒙 6）或 API 26（鸿蒙 7） |
| Rust | stable，并安装目标 `aarch64-unknown-linux-ohos` |
| NDK | 带 `llvm/bin/clang` 的 OHOS Native SDK |
| 电脑系统 | 当前脚本以 Windows + PowerShell 为主 |
| 手机 | HarmonyOS NEXT，开启开发者模式与 USB 调试 |

---

## 编译与安装

### 1. 配置签名（必做）

公开仓库**不包含**你的调试证书和密码。任选一种方式：

1. DevEco 打开 `ohos/` → **Project Structure → Signing Configs** → 勾选自动签名并登录华为账号。  
2. 或复制示例后手工填写：

```bat
copy ohos\build-profile.json5.example ohos\build-profile.json5
```

再编辑其中的证书路径与口令。

### 2. 编译 Rust 核心库

```powershell
$env:OHOS_NATIVE_HOME = "C:\path\to\ohos-sdk\native"
powershell -ExecutionPolicy Bypass -File .\scripts\build-native.ps1
```

成功后会生成：`ohos/entry/libs/arm64-v8a/libhdcore.so`。

### 3. 打包并安装到手机

```bat
scripts\build-hap.bat
hdc install ohos\entry\build\default\outputs\default\entry-default-signed.hap
```

调试证书必须绑定当前手机的 UDID，否则安装或启动会失败。

---

## 使用说明

1. 打开应用，进入 **设置**，填写自建服务（示例请换成你自己的）：

| 配置项 | 含义 | 示例格式 |
|--------|------|----------|
| ID 服务器 | hbbs | `你的域名或IP:21116` |
| 中继服务器 | hbbr | `你的域名或IP:21117` |
| Key | 与 hbbs 的 `-k` 一致 | 一长串公钥字符串 |

2. 在 **设备** 页新建连接：填对端 **设备 ID** 与 **远程密码**，或填局域网 **IP** 直连。  
3. 进入会话后可用底栏打开键盘，发送 Ctrl+Alt+Del、Alt+F4、显示桌面等。

**请勿把真实服务器地址、Key、远程密码提交到 Git。**

---

## 安全与隐私

本公开仓库已刻意移除：

- 本机签名证书路径与密钥口令  
- 自建服务器 IP、Key、设备密码等本地偏好  
- 调试用的 hilog、崩溃 dump、截图等

本地签名备份若存在 `ohos/build-profile.signing.local.json5`，已被 `.gitignore` 忽略，请勿上传。

---

## 常见问题

**装不上 / 打不开？**  
检查是否已自动签名、UDID 是否在证书里、系统 API 是否匹配。

**能连上但很卡？**  
优先试局域网 IP 直连；公网中继延迟取决于网络与服务器。

**Ctrl+Alt+Del 无效？**  
部分 Windows 被控端需允许软件安全注意序列（SAS）相关策略，与官方 RustDesk 相同。

---

## 相关文档

- [PROTOCOL.md](./PROTOCOL.md) — 协议与端口说明  
- [DEVELOPMENT.md](./DEVELOPMENT.md) — 开发备忘  
- [VIDEO_DECODER.md](./VIDEO_DECODER.md) — 解码相关笔记  

（部分旧文档仍可能写到 `harmonyos/` 路径，以本 README 的 `ohos/` 为准。）

---

## 致谢与声明

- 协议兼容开源 [RustDesk](https://github.com/rustdesk/rustdesk) 生态  
- 在社区 HarmonyDesk 相关工程基础上，面向 HarmonyOS NEXT 控制端继续演进  

使用本软件时，请遵守当地法律法规，并获得被控设备所有者的明确授权。
