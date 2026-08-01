//! # cyrio-tauri
//!
//! Tauri 2.0 命令绑定层：把 [`cyrio_core`] 的 API 包装成 `#[tauri::command]`。
//!
//! ## 架构
//! - [`commands::DeviceState`]：全局共享的设备句柄，用 `smol::Mutex` 包装（因为 cyrio-core 用 smol）
//! - 每个命令 `async fn`，内部 `state.lock().await` 拿设备后调用 cyrio-core API
//! - smol future 可以在 Tauri 的 tokio runtime 上 await（runtime-agnostic）
//! - 但 `smol::Timer` 需要 smol 全局执行器驱动，所以 [`crate::start_smol_executor`]
//!   必须在 Tauri 启动时调用
//!
//! ## 命令位置
//! 所有 `#[tauri::command]` 函数位于 [`commands`] 模块中——Tauri 2.0 的命令宏
//! 在 crate 根模块中不能标记为 `pub`（会触发 E0255 "defined multiple times"），
//! 因此放到子模块后再 `pub use` 重导出。

#![warn(missing_docs)]

use cyrio_core::protocol::rio_file::RioFile;
use cyrio_core::protocol::rio_mem::RioMem;
use serde::Serialize;

pub mod audio_commands;
pub mod commands;
pub mod sync_commands;
pub mod webdav_server;
// AudioState/PlaybackState/start_audio_thread 实际定义在 cyrio_audio::manager，
// 这里 re-export 让 Tauri 应用层用 `cyrio_tauri::AudioState` 即可。
pub use audio_commands::SharedAudioState;
pub use cyrio_audio::manager::{start_audio_thread, AudioState, PlaybackState};
pub use commands::DeviceState;
pub use sync_commands::{SyncResult, SyncRule};
pub use webdav_server::{WebDavState, WebDavStatus};

/// 启动 smol 全局执行器（后台线程）
///
/// cyrio-core 的 `smol::Timer` 需要 smol 全局执行器驱动。
/// Tauri 2.0 默认用 tokio，所以必须在独立线程上跑 `smol::block_on(pending)`。
pub fn start_smol_executor() {
    std::thread::Builder::new()
        .name("smol-executor".into())
        .spawn(|| {
            smol::block_on(smol::future::pending::<()>());
        })
        .expect("spawn smol executor thread");
}

/// 设备信息（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    /// 是否已连接
    pub connected: bool,
    /// 设备型号
    pub model: String,
}

/// USB 设备列表项（用于"强制添加设备"功能）
#[derive(Debug, Clone, Serialize)]
pub struct UsbDeviceListItem {
    /// 厂商 ID（十六进制形式，如 "0x045a"）
    pub vid: String,
    /// 产品 ID（十六进制形式，如 "0x5009"）
    pub pid: String,
    /// 厂商 ID 数值
    pub vid_num: u16,
    /// 产品 ID 数值
    pub pid_num: u16,
    /// 产品名称
    pub name: String,
    /// 厂商名称
    pub manufacturer: String,
    /// 是否为 Diamond 厂商（vid == 0x045a）
    pub is_diamond: bool,
}

/// 存储信息（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct StorageInfo {
    /// 内存单元（0=内置, 1=SD 卡）
    pub mem_unit: u8,
    /// 是否存在
    pub present: bool,
    /// 总容量（字节）
    pub size: u64,
    /// 已用（字节）
    pub used: u64,
    /// 可用（字节）
    pub free: u64,
    /// 人类可读的大小（如 "64 MB"）
    pub size_formatted: String,
}

impl From<RioMem> for StorageInfo {
    fn from(mem: RioMem) -> Self {
        Self {
            mem_unit: 0,
            present: mem.is_present(),
            size: mem.size as u64,
            used: mem.used as u64,
            free: mem.free as u64,
            size_formatted: mem.format_size(),
        }
    }
}

/// 歌曲信息（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct SongInfo {
    /// 文件号
    pub file_no: u32,
    /// 文件大小（字节）
    pub size: u32,
    /// 时长（秒）
    pub time: u32,
    /// 文件名（latin1 已转 UTF-8）
    pub name: String,
    /// 标题
    pub title: String,
    /// 艺术家
    pub artist: String,
    /// 专辑
    pub album: String,
    /// 比特率（单位 kbps << 7，前端显示时 >> 7）
    pub bit_rate: u32,
    /// 所在内存单元（0=内置, 1=SD 卡）
    pub mem_unit: u8,
}

impl From<&RioFile> for SongInfo {
    fn from(f: &RioFile) -> Self {
        Self {
            file_no: f.file_no,
            size: f.size,
            time: f.time,
            name: f.name.clone(),
            title: f.title.clone(),
            artist: f.artist.clone(),
            album: f.album.clone(),
            bit_rate: f.bit_rate,
            mem_unit: 0, // 由调用方覆盖
        }
    }
}

/// MP3 技术参数（来自 parse_mp3_info）
#[derive(Debug, Clone, Serialize)]
pub struct SongTechnical {
    /// 时长（秒）
    pub duration: u32,
    /// 采样率（Hz）
    pub sample_rate: u32,
    /// 比特率（kbps）
    pub bit_rate: u32,
    /// MPEG 层（1/2/3）
    pub layer: u8,
    /// 声道数（1=单声道, 2=立体声）
    pub channels: u8,
}

/// ID3 标签信息
#[derive(Debug, Clone, Serialize)]
pub struct SongId3 {
    /// 标题
    pub title: String,
    /// 艺术家
    pub artist: String,
    /// 专辑
    pub album: String,
    /// 年份
    pub year: String,
    /// 流派
    pub genre: String,
    /// 音轨号
    pub track: String,
    /// 作曲
    pub composer: String,
}

/// 歌曲详细信息（用于"详细信息"弹窗）
#[derive(Debug, Clone, Serialize)]
pub struct SongDetail {
    /// 基本信息
    pub basic: SongInfo,
    /// 技术参数（MP3 帧头解析，可能为 None）
    pub technical: Option<SongTechnical>,
    /// ID3 标签
    pub id3: SongId3,
    /// 专辑封面（设备存储的纯音频无 APIC，通常为 None）
    pub cover_art: Option<Vec<u8>>,
    /// 修改时间（Unix 秒）
    pub mod_date: u32,
}
