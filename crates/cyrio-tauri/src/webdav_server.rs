//! WebDAV 虚拟U盘服务器（Tauri 命令薄包装）
//!
//! 核心实现已抽离到 `cyrio-webdav` crate，这里只做 Tauri 命令绑定。
//!
//! ## 命令
//! - [`start_webdav`]：启动服务器（需设备已连接）
//! - [`stop_webdav`]：停止服务器
//! - [`get_webdav_status`]：查询当前状态
//! - [`mount_webdav`]：自动挂载到系统（macOS Finder / Windows 资源管理器）

use cyrio_webdav::{mount_webdav as do_mount_webdav, WebDavServer};
pub use cyrio_webdav::WebDavStatus;

use crate::commands::DeviceState;

/// WebDAV 服务器全局状态（Tauri managed）
///
/// 持有 [`cyrio_webdav::WebDavServer`] 实例，通过 Tauri State 跨命令共享。
pub struct WebDavState {
    /// 内部 WebDAV 服务器
    server: WebDavServer,
}

impl WebDavState {
    /// 创建初始状态（服务器未运行）
    pub fn new() -> Self {
        Self {
            server: WebDavServer::new(),
        }
    }
}

impl Default for WebDavState {
    fn default() -> Self {
        Self::new()
    }
}

/// 启动 WebDAV 服务器
///
/// 在专用线程中运行 tiny_http 服务器，绑定 127.0.0.1:8765。
/// 设备句柄通过 `Arc<smol::Mutex<Option<RioDevice>>>` 共享给服务器线程。
#[tauri::command]
pub async fn start_webdav(
    device_state: tauri::State<'_, DeviceState>,
    webdav_state: tauri::State<'_, WebDavState>,
) -> Result<String, String> {
    webdav_state.server.start(device_state.device.clone())
}

/// 停止 WebDAV 服务器
///
/// 设置停止标志后等待服务器线程结束（最多 ~1 秒）。
#[tauri::command]
pub async fn stop_webdav(
    webdav_state: tauri::State<'_, WebDavState>,
) -> Result<(), String> {
    // stop() 内部 join 服务器线程（recv_timeout 1s 轮询），直接调用会阻塞当前 async 任务 ~1s。
    // 这是低频操作（用户点击停止），直接调用可接受。
    webdav_state.server.stop()
}

/// 查询 WebDAV 服务器状态
#[tauri::command]
pub async fn get_webdav_status(
    webdav_state: tauri::State<'_, WebDavState>,
) -> Result<WebDavStatus, String> {
    Ok(webdav_state.server.status())
}

/// 自动挂载 WebDAV 网络驱动器
///
/// macOS：`open http://127.0.0.1:8765`（Finder 挂载到 /Volumes/）
/// Windows：`net use Z: http://127.0.0.1:8765 /persistent:no`
#[tauri::command]
pub async fn mount_webdav() -> Result<String, String> {
    smol::unblock(|| do_mount_webdav()).await
}
