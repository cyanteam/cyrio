# cyrio 拯救你的帝盟Rio MP3播放器

Diamond Rio S-Series USB MP3 播放器管理工具（纯 Rust，跨平台）。
无需32位电脑+帝盟古老驱动，现代设备可直接传曲和管理。

## 平台支持

| 平台 | 框架 | 状态 |
|------|------|------|
| macOS / Linux / Windows | Tauri 2 | ✅ 主要版本 |
| Android | 原生Kotlin + JNI | ✅ 主要版本 |
| macOS / Linux / Windows | egui | ✅ 可用，开发中 |
| Web (WASM) | webusb-web | 🟡 正在适配 |
| macOS / Linux / Windows | GPUI | 🟡 开发中 |

## 仓库结构

本仓库为 monorepo，包含核心协议层和所有平台前端实现：

```
cyrio/
├── crates/
│   ├── cyrio-core/                 # Rio S-Series USB 协议核心库
│   ├── cyrio-text/                 # 文本处理（拼音转换、噪音词过滤）
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
└── docs/PROTOCOL.md                # 协议规范
```

## 使用
### **[Release](https://github.com/cyanteam/cyrio/release)**

## 编译

### 前置条件

- Rust 1.75+（推荐 rustup 最新 stable）
- Node.js 18+（Tauri 前端构建）
- Android SDK + NDK 27（Android 构建）

### 桌面版（Tauri）

```bash
cd platforms/tauri-desktop/frontend
npm install
npm run build
cd ../..
cargo run -p cyrio-tauri
```

### 桌面版（egui）

```bash
cargo run -p cyrio-desktop
```

### Android

```bash
cd platforms/android-native
./build-android.sh    # 构建 .so + APK
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
