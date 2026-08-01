//! Command + Event 枚举（UI ↔ 后台消息）

use std::path::PathBuf;

use cyrio_core::api::playlist::PlaylistSong;
use cyrio_core::api::rename::RenameResult;
use cyrio_core::api::upload::{UploadResult, UploadTextOptions};
use cyrio_core::error::Result;
use cyrio_core::protocol::rio_file::RioFile;
use cyrio_transport_nusb::UsbDeviceInfo;

/// UI → 后台 的命令
#[derive(Debug)]
pub enum Command {
    /// 打开设备（自动识别 Diamond Rio）
    OpenDevice,
    /// 强制打开指定 VID/PID 的设备
    OpenDeviceForce { vid: u16, pid: u16 },
    /// 关闭设备
    CloseDevice,
    /// 扫描所有 USB 设备
    ScanDevices,
    /// 列出歌曲（参数：内存单元 0=INT, 1=SD）
    ListSongs(u8),
    /// 列出歌单
    ListPlaylists(u8),
    /// 列出歌单内歌曲
    ListPlaylistSongs { playlist_file_no: u32, mem_unit: u8 },
    /// 上传单个 MP3 文件
    UploadSong {
        path: PathBuf,
        mem_unit: u8,
        text_opts: UploadTextOptions,
    },
    /// 批量上传 MP3 文件
    UploadSongBatch {
        paths: Vec<PathBuf>,
        mem_unit: u8,
        text_opts: UploadTextOptions,
    },
    /// 下载歌曲到本地文件
    DownloadSong {
        file_no: u32,
        mem_unit: u8,
        save_path: PathBuf,
    },
    /// 下载歌曲到内存（用于播放试听）
    DownloadSongForPlay { file_no: u32, mem_unit: u8 },
    /// 删除文件
    DeleteSong { file_no: u32, mem_unit: u8 },
    /// 将歌曲加入歌单
    AddToPlaylist {
        song_file_no: u32,
        song_mem_unit: u8,
        playlist_file_no: u32,
        playlist_mem_unit: u8,
    },
    /// 创建空歌单
    CreatePlaylist { name: String, mem_unit: u8 },
    /// 修复歌单编码（清除 bit 0 双重编码污染）
    RepairPlaylistEncoding { file_no: u32, mem_unit: u8 },
    /// 重命名单个歌曲 title
    RenameSong {
        file_no: u32,
        mem_unit: u8,
        new_title: String,
    },
    /// 批量转拼音（指定列表）
    BatchSlugSongs { items: Vec<(u32, u8, String)> },
    /// 批量去词（指定列表）
    BatchStripSongs {
        items: Vec<(u32, u8, String)>,
        custom_words: Vec<String>,
    },
    /// 修复单个歌曲编码
    RepairSongEncoding { file_no: u32, mem_unit: u8 },
    /// 修复所有歌曲编码
    RepairAllSongsEncoding,
    /// 批量为所有歌曲转拼音
    BatchSlugAllSongs,
    /// 批量为所有歌曲去词
    BatchStripAllSongs { custom_words: Vec<String> },
    /// 查询存储状态
    GetStorageStatus,
    /// 退出后台任务
    Quit,
}

/// 后台 → UI 的事件
#[derive(Debug)]
pub enum Event {
    /// 设备已打开
    DeviceOpened(Result<()>),
    /// 设备已关闭
    DeviceClosed,
    /// USB 设备扫描结果
    DevicesScanned(Vec<UsbDeviceInfo>),
    /// 歌曲列表已获取（指定 mem_unit，用于双存储合并）
    SongsListedForMem { songs: Vec<RioFile>, mem_unit: u8 },
    /// 歌单列表已获取（指定 mem_unit，用于双存储合并）
    PlaylistsListedForMem { playlists: Vec<RioFile>, mem_unit: u8 },
    /// 歌单内歌曲已获取
    PlaylistSongsListed(Result<Vec<PlaylistSong>>),
    /// 上传进度
    UploadProgress { sent_bytes: u64, total_bytes: u64 },
    /// 批量上传开始（携带文件名列表，用于初始化传输对话框）
    UploadBatchStarted { names: Vec<String> },
    /// 批量上传中，单个文件开始
    UploadFileStarted { index: usize, name: String },
    /// 批量上传中，单个文件完成
    UploadFileCompleted { index: usize, success: bool },
    /// 单个文件上传完成
    UploadCompleted(Result<u32>),
    /// 批量上传完成
    UploadBatchCompleted(Vec<UploadResult>),
    /// 下载进度
    DownloadProgress {
        received_bytes: u64,
        total_bytes: u64,
    },
    /// 下载到文件完成
    DownloadCompleted(Result<()>),
    /// 下载到内存完成（用于播放）
    SongDownloaded(Result<Vec<u8>>),
    /// 删除完成
    DeleteCompleted(Result<()>),
    /// 加入歌单完成
    AddToPlaylistCompleted(Result<()>),
    /// 创建歌单完成
    CreatePlaylistCompleted(Result<u32>),
    /// 歌单编码修复完成
    PlaylistRepaired(Result<()>),
    /// 单个重命名完成
    RenameCompleted(Result<()>),
    /// 批量操作完成（slug/strip/repair）
    BatchOperationCompleted {
        kind: String,
        results: Vec<RenameResult>,
    },
    /// 存储状态已获取
    StorageStatusGot(Result<StorageStatus>),
    /// 后台日志
    Log(String),
}

/// 存储状态
#[derive(Debug, Clone)]
pub struct StorageStatus {
    /// 内置存储
    pub internal: StorageUnit,
    /// SD 卡
    pub sd_card: StorageUnit,
}

/// 单个内存单元的存储信息
#[derive(Debug, Clone)]
pub struct StorageUnit {
    /// 内存单元编号
    pub mem_unit: u8,
    /// 是否插入
    pub present: bool,
    /// 名称
    pub name: String,
    /// 型号字符串
    pub model: String,
    /// 总容量（字节）
    pub size: u64,
    /// 已用（字节）
    pub used: u64,
    /// 空闲（字节）
    pub free: u64,
}
