# cyrio

Diamond Rio S-Series USB MP3 播放器管理工具（纯 Rust，跨平台）。

## 平台支持

| 平台 | 框架 | 状态 |
|------|------|------|
| macOS / Linux / Windows | egui | ✅ 可用 |
| macOS / Linux / Windows | Tauri 2 | ✅ 可用 |
| macOS / Linux / Windows | GPUI | 🟡 开发中 |
| Android | Kotlin + JNI | ✅ 可用 |
| Web (WASM) | webusb-web | ⬜ 待开发 |

## 仓库结构

本仓库为 monorepo，包含所有平台前端实现。核心协议层 (`cyrio-core`) 为独立私有仓库：

- **[cyrio-core](https://github.com/cyanteam/cyrio-core)**（私有）— Rio S-Series USB 协议核心库 + 文本处理
- **cyrio**（本仓库）— 各平台 GUI 实现 + 传输层 + 音频

```
cyrio/
├── crates/
│   ├── cyrio-transport-nusb/       # 桌面 USB transport（nusb）
│   ├── cyrio-transport-webusb/     # Web USB transport（WASM）
│   ├── cyrio-audio/                # 音频播放抽象（rodio）
│   ├── cyrio-webdav/               # WebDAV 虚拟U盘
│   ├── cyrio-app/                  # egui 共用应用
│   ├── cyrio-tauri/                # Tauri 2 后端
│   └── cyrio-jni/                  # Android JNI 桥接
├── platforms/
│   ├── desktop/                    # egui 桌面入口
│   ├── tauri-desktop/              # Tauri 2 桌面入口
│   ├── gpui-desktop/               # GPUI 桌面入口（开发中）
│   └── android-native/             # Android 原生（Kotlin）
├── cyrio-java/                     # JavaFX 版本（参考实现）
└── docs/PROTOCOL.md                # 协议规范
```

## 编译

### 前置条件

- Rust 1.75+（推荐 rustup 最新 stable）
- cyrio-core 仓库的访问权限（私有依赖）

### 桌面版（egui）

```bash
cargo run -p cyrio-desktop
```

### 桌面版（Tauri）

```bash
cd platforms/tauri-desktop
npm install
npm run tauri dev
```

### Android

```bash
cd platforms/android-native
./build-android.sh    # 构建 .so + APK
```

## 本地开发（使用本地 cyrio-core 源码）

默认从 GitHub 拉取 `cyrio-core`。如需本地开发，在 `Cargo.toml` 中取消注释 `[patch]` 段：

```toml
[patch."https://github.com/cyanteam/cyrio-core.git"]
cyrio-core = { path = "crates/cyrio-core" }
cyrio-text = { path = "crates/cyrio-text" }
```

## 架构

- **GUI 解耦**：UI ↔ 后台通过 channel 消息传递（`async-channel`）
- **异步运行时**：`smol`（不依赖 tokio）
- **USB 传输**：trait 抽象，桌面用 nusb，Android 用 JNI UsbManager
- **核心协议**：rioutil 兼容算法，自实现 CRC32

## 协议规范

见 [docs/PROTOCOL.md](docs/PROTOCOL.md)。

## License

MIT
