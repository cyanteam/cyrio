//! AppState：全局应用状态
//!
//! 包含：
//! - `page_path`：当前页面路径（与 WASM `window.location.hash` 同步）
//! - `mem_unit`：当前内存单元（0=内置, 1=SD）
//! - 设备连接状态、存储信息、歌曲/歌单列表
//! - 多选集合、进度信息
//! - 音频播放器、WebDAV 服务器
//! - 二次确认对话框、加载遮罩
//! - stdout 日志缓冲 + 调试窗口开关

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use cyrio_audio::manager::{AudioState, PlaybackState};
use cyrio_core::api::playlist::PlaylistSong;
use cyrio_core::api::device::RioDevice;
use cyrio_core::protocol::rio_file::RioFile;
use cyrio_core::protocol::rio_mem::RioMem;
use cyrio_transport_nusb::UsbDeviceInfo;
use cyrio_webdav::{WebDavServer, WebDavStatus};
use smol::lock::Mutex as SmolMutex;

/// 应用设置（持久化到 JSON 文件）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// 外观主题：0=现代, 1=经典（Phase 2）
    pub appearance: u8,
    /// 上传时是否应用 slug（中文→拼音）
    pub upload_apply_slug: bool,
    /// 上传时是否去除无关词汇
    pub upload_apply_strip: bool,
    /// 是否去除括号内容
    pub strip_parentheses: bool,
    /// 是否去除引号内容
    pub strip_quotes: bool,
    /// 是否去除音质/规格停用词
    pub strip_quality_tags: bool,
    /// 自定义停用词（每行一个）
    pub custom_stop_words: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            appearance: 0,
            upload_apply_slug: false,
            upload_apply_strip: false,
            strip_parentheses: true,
            strip_quotes: true,
            strip_quality_tags: true,
            custom_stop_words: String::new(),
        }
    }
}

impl AppSettings {
    /// 配置文件路径（跨平台，不依赖 dirs crate）
    fn config_path() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("Library/Application Support/cyrio/settings.json"))
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
                .map(|d| d.join("cyrio/settings.json"))
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var("APPDATA")
                .ok()
                .map(|d| PathBuf::from(d).join("cyrio/settings.json"))
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            None
        }
    }

    /// 从文件加载（失败返回 Default）
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// 保存到文件（失败静默忽略）
    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// 解析自定义停用词（每行一个）
    pub fn custom_words_vec(&self) -> Vec<String> {
        self.custom_stop_words
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }
}

/// 内存单元编号：内置闪存
pub const MEM_UNIT_INTERNAL: u8 = 0;
/// 内存单元编号：SD 卡
pub const MEM_UNIT_SD: u8 = 1;

/// 歌曲/歌单条目（带内存单元标记，用于双存储合并显示）
///
/// 对齐 Tauri 版 SongInfo + mem_unit：歌曲页同时加载内置+SD 两个 mem_unit
/// 的文件，合并为一个列表，每行用 mem-badge 区分所在存储。
#[derive(Debug, Clone)]
pub struct SongEntry {
    /// 文件元数据
    pub file: RioFile,
    /// 所在内存单元（0=内置, 1=SD）
    pub mem_unit: u8,
}

/// 全局应用状态
///
/// 桌面：UI 线程直接持有 `&mut AppState`
/// Web：用 `Arc<RwLock<>>` 共享给 hashchange 回调
pub struct AppState {
    // ===== 路由与连接 =====
    /// 当前页面路径（如 "songs" / "playlists" / "upload" / "device"）
    pub page_path: String,

    /// 当前内存单元（0=INT, 1=SD）
    pub mem_unit: u8,

    /// 设备是否已连接
    pub connected: bool,

    /// 是否正在连接中（gate USB 扫描，避免 claim_interface 竞态）
    pub connecting: bool,

    /// 设备型号字符串（如 "Rio S50"）
    pub device_model: Option<String>,

    /// 共享设备句柄（Arc<smol::Mutex<Option<RioDevice>>>）
    /// 供 WebDAV 服务器、音频下载等使用
    pub device: Arc<SmolMutex<Option<RioDevice>>>,

    // ===== USB 设备扫描 =====
    /// 扫描到的 USB 设备列表
    pub usb_devices: Vec<UsbDeviceInfo>,

    /// 是否正在扫描
    pub scanning: bool,

    // ===== 数据缓存 =====
    /// 内置存储信息
    pub internal_mem: Option<RioMem>,

    /// SD 卡存储信息（None 表示未插入）
    pub sd_mem: Option<RioMem>,

    /// 当前内存单元的歌曲列表（双存储合并，每项带 mem_unit 标记）
    pub songs: Vec<SongEntry>,

    /// 当前内存单元的歌单列表（双存储合并，每项带 mem_unit 标记）
    pub playlists: Vec<SongEntry>,

    /// 选中的歌曲 file_no 集合（多选）
    pub selected_song_ids: HashSet<u32>,

    /// 上次点击的歌曲索引（Shift 范围选择锚点）
    pub last_clicked_song_index: Option<usize>,

    /// 当前选中的歌单 file_no
    pub selected_playlist_id: Option<u32>,

    /// 选中歌单内的歌曲列表
    pub playlist_songs: Vec<PlaylistSong>,

    /// 是否正在加载歌单内歌曲
    pub loading_playlist_songs: bool,

    // ===== 音频播放 =====
    /// 音频管理器（连接设备时创建）
    pub audio: Option<AudioState>,

    /// 当前播放状态（每帧从 AudioState 读取）
    pub playback_state: PlaybackState,

    /// 当前播放歌曲的 file_no（None 表示未播放）
    pub current_playing_file_no: Option<u32>,

    // ===== WebDAV =====
    /// WebDAV 服务器
    pub webdav: WebDavServer,

    /// WebDAV 当前状态
    pub webdav_status: WebDavStatus,

    // ===== 模态框 =====
    /// 二次确认对话框
    pub confirm_dialog: Option<ConfirmDialog>,

    /// 加载遮罩提示文本（None 表示不显示）
    pub show_loading_modal: Option<String>,

    // ===== 进度 =====
    /// 当前进行中的操作（None 表示空闲）
    pub progress: Option<ProgressInfo>,

    /// 上传传输对话框状态（None 表示不显示）
    pub upload_transfer: Option<UploadTransferState>,

    /// 状态栏消息（最近一条用户可见提示）
    pub status_message: Option<String>,

    // ===== 调试 =====
    /// stdout 输出缓存（调试窗口显示）
    pub logs: Arc<RwLock<Vec<String>>>,

    /// 调试窗口是否打开（Alt+Shift+D 切换）
    pub debug_window_open: bool,

    /// 是否正在加载中（列表加载等）
    pub loading: bool,

    /// 当前页面的搜索查询字符串
    pub search_query: String,

    /// 是否显示新建歌单对话框
    pub show_create_playlist_dialog: bool,

    /// 是否显示"加入歌单"对话框
    pub show_add_to_playlist_dialog: bool,

    /// 待加入歌单的歌曲 file_no（对话框确认后用）
    pub add_to_playlist_song_file_no: Option<u32>,

    /// 新建歌单对话框中的名称输入
    pub new_playlist_name: String,

    /// 待上传文件列表
    pub pending_uploads: Vec<std::path::PathBuf>,

    /// 上传目标内存单元（0=INT, 1=SD）
    pub upload_target_mem: u8,

    /// 格式化目标内存单元
    pub format_target_mem: u8,

    /// 是否显示格式化确认对话框
    pub show_format_confirm: bool,

    /// 分页开关（false=全部，true=分页）
    pub paginate: bool,

    /// 上次 USB 扫描时间（ctx.input time，秒）
    pub last_scan_time: f64,

    /// 是否显示强制添加设备对话框
    pub show_force_add_dialog: bool,

    // ===== notice toast（对齐 Tauri .notice-toast） =====
    /// 通知消息（None 表示不显示）
    pub notice_message: Option<String>,

    /// 通知设置时间（ctx.time，秒），3 秒后自动消失
    pub notice_time: f64,

    // ===== 歌曲页排序/分页 =====
    /// 排序方式（0=名称, 1=大小, 2=时间）
    pub sort_by: u8,

    /// 当前分页页码（从 0 开始）
    pub current_page: usize,

    /// 每页歌曲数
    pub songs_per_page: usize,

    // ===== 双存储加载计数器 =====
    /// 待完成的歌曲加载请求数（连接时设 2，每次事件 -1，归零时 loading=false）
    pub pending_song_loads: u8,

    /// 待完成的歌单加载请求数
    pub pending_playlist_loads: u8,

    // ===== 歌单页独立状态 =====
    /// 当前查看详情的歌单（file_no, mem_unit），None=列表视图
    pub active_playlist: Option<(u32, u8)>,

    /// 歌单多选键集合（file_no, mem_unit）
    pub selected_playlist_keys: HashSet<(u32, u8)>,

    /// 歌单页排序方式（0=名称, 1=大小）
    pub playlist_sort_by: u8,

    /// 歌单页搜索查询
    pub playlist_search_query: String,

    /// 歌单页当前分页页码
    pub playlist_current_page: usize,

    // ===== 设置与重命名 =====
    /// 应用设置（持久化）
    pub settings: AppSettings,

    /// 重命名对话框状态：(file_no, mem_unit, 原始标题)
    pub show_rename_dialog: Option<(u32, u8, String)>,

    /// 重命名输入框文本
    pub rename_input: String,
}

/// 二次确认对话框内容
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    /// 确认动作
    pub action: ConfirmAction,
    /// 提示消息
    pub message: String,
}

/// 需要二次确认的动作
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    /// 删除单个歌曲
    DeleteSong { file_no: u32, mem_unit: u8 },
    /// 批量删除歌曲
    DeleteSongsBatch { file_nos: Vec<u32>, mem_unit: u8 },
    /// 删除歌单
    DeletePlaylist { file_no: u32, mem_unit: u8 },
    /// 格式化内存单元
    Format { mem_unit: u8 },
}

/// 进度信息（上传/下载/删除等）
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    /// 操作类型
    pub kind: ProgressKind,
    /// 已完成字节数
    pub current: u64,
    /// 总字节数
    pub total: u64,
}

/// 上传传输对话框状态（对齐 Tauri UploadTransferDialog）
#[derive(Debug, Clone)]
pub struct UploadTransferState {
    /// 文件列表
    pub files: Vec<UploadFileEntry>,
    /// 当前正在上传的文件索引
    pub current_index: usize,
    /// 完成时间戳（ctx.time，秒）；None 表示尚未完成
    pub done_time: Option<f64>,
}

/// 单个上传文件条目
#[derive(Debug, Clone)]
pub struct UploadFileEntry {
    /// 文件名（不含路径）
    pub name: String,
    /// 已传输字节数
    pub transferred: u64,
    /// 总字节数
    pub total: u64,
    /// 状态
    pub status: UploadFileStatus,
}

/// 上传文件状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadFileStatus {
    /// 等待中
    Pending,
    /// 正在传输
    Uploading,
    /// 已完成
    Done,
    /// 失败
    Failed,
}

impl UploadTransferState {
    /// 已完成文件数
    pub fn done_count(&self) -> usize {
        self.files.iter().filter(|f| f.status == UploadFileStatus::Done).count()
    }

    /// 失败文件数
    pub fn failed_count(&self) -> usize {
        self.files.iter().filter(|f| f.status == UploadFileStatus::Failed).count()
    }

    /// 总字节
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.total).sum()
    }

    /// 已传输字节
    pub fn transferred_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.transferred).sum()
    }

    /// 总进度分数 (0.0..=1.0)
    pub fn total_fraction(&self) -> f32 {
        if self.files.is_empty() {
            0.0
        } else {
            self.done_count() as f32 / self.files.len() as f32
        }
    }

    /// 当前文件
    pub fn current_file(&self) -> Option<&UploadFileEntry> {
        self.files.get(self.current_index).filter(|f| f.status == UploadFileStatus::Uploading)
    }

    /// 是否全部完成
    pub fn all_done(&self) -> bool {
        !self.files.is_empty() && self.files.iter().all(|f| f.status == UploadFileStatus::Done || f.status == UploadFileStatus::Failed)
    }
}

/// 进度操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressKind {
    /// 上传
    Upload,
    /// 下载
    Download,
    /// 删除
    Delete,
    /// 列表加载
    Listing,
}

impl ProgressInfo {
    /// 0.0..=1.0
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }

    /// 人类可读的进度文本
    pub fn label(&self) -> String {
        let kind_str = match self.kind {
            ProgressKind::Upload => "上传",
            ProgressKind::Download => "下载",
            ProgressKind::Delete => "删除",
            ProgressKind::Listing => "加载",
        };
        format!(
            "{}中… {} / {}",
            kind_str,
            format_bytes(self.current),
            format_bytes(self.total)
        )
    }
}

/// 格式化字节数为人类可读字符串
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            page_path: "songs".to_string(),
            mem_unit: MEM_UNIT_INTERNAL,
            connected: false,
            connecting: false,
            device_model: None,
            device: Arc::new(SmolMutex::new(None)),
            usb_devices: Vec::new(),
            scanning: false,
            internal_mem: None,
            sd_mem: None,
            songs: Vec::new(),
            playlists: Vec::new(),
            selected_song_ids: HashSet::new(),
            last_clicked_song_index: None,
            selected_playlist_id: None,
            playlist_songs: Vec::new(),
            loading_playlist_songs: false,
            audio: None,
            playback_state: PlaybackState {
                is_playing: false,
                position: 0.0,
                duration: 0.0,
                is_loading: false,
            },
            current_playing_file_no: None,
            webdav: WebDavServer::new(),
            webdav_status: WebDavStatus::Stopped,
            confirm_dialog: None,
            show_loading_modal: None,
            progress: None,
            upload_transfer: None,
            status_message: None,
            logs: Arc::new(RwLock::new(Vec::new())),
            debug_window_open: false,
            loading: false,
            search_query: String::new(),
            show_create_playlist_dialog: false,
            show_add_to_playlist_dialog: false,
            add_to_playlist_song_file_no: None,
            new_playlist_name: String::new(),
            pending_uploads: Vec::new(),
            upload_target_mem: 0,
            format_target_mem: 0,
            show_format_confirm: false,
            paginate: false,
            last_scan_time: 0.0,
            show_force_add_dialog: false,
            notice_message: None,
            notice_time: 0.0,
            sort_by: 0,
            current_page: 0,
            songs_per_page: 50,
            pending_song_loads: 0,
            pending_playlist_loads: 0,
            active_playlist: None,
            selected_playlist_keys: HashSet::new(),
            playlist_sort_by: 0,
            playlist_search_query: String::new(),
            playlist_current_page: 0,
            settings: AppSettings::load(),
            show_rename_dialog: None,
            rename_input: String::new(),
        }
    }
}

impl AppState {
    /// 添加一条日志
    pub fn log(&self, msg: impl Into<String>) {
        if let Ok(mut logs) = self.logs.write() {
            logs.push(msg.into());
            // 限制最大 1000 条，避免内存爆炸
            let excess = logs.len().saturating_sub(1000);
            if excess > 0 {
                logs.drain(0..excess);
            }
        }
    }

    /// 获取当前内存单元的存储信息
    pub fn current_mem(&self) -> Option<&RioMem> {
        match self.mem_unit {
            MEM_UNIT_INTERNAL => self.internal_mem.as_ref(),
            MEM_UNIT_SD => self.sd_mem.as_ref(),
            _ => None,
        }
    }

    /// 切换内存单元
    pub fn switch_mem_unit(&mut self, unit: u8) {
        self.mem_unit = unit;
        // 双存储合并显示后，切换单元不再清空列表（仅用于上传页目标选择）
        self.selected_song_ids.clear();
        self.last_clicked_song_index = None;
        self.selected_playlist_id = None;
        self.playlist_songs.clear();
    }

    /// 设置状态消息（同时触发 notice toast 显示）
    pub fn set_status(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        self.status_message = Some(m.clone());
        self.notice_message = Some(m);
        // 哨兵值：ui() 首帧检测到后替换为真实 ctx.time
        self.notice_time = f64::MAX;
    }

    /// 设置 notice toast 通知（对齐 Tauri .notice-toast，3 秒后自动消失）
    /// notice_time 由调用方传入 ctx.time
    pub fn set_notice(&mut self, msg: impl Into<String>, now: f64) {
        self.notice_message = Some(msg.into());
        self.notice_time = now;
    }

    /// 清除 notice toast
    pub fn clear_notice(&mut self) {
        self.notice_message = None;
    }

    /// 显示加载遮罩
    pub fn show_loading(&mut self, msg: impl Into<String>) {
        self.show_loading_modal = Some(msg.into());
    }

    /// 隐藏加载遮罩
    pub fn hide_loading(&mut self) {
        self.show_loading_modal = None;
    }

    /// 弹出二次确认对话框
    pub fn confirm(&mut self, action: ConfirmAction, message: impl Into<String>) {
        self.confirm_dialog = Some(ConfirmDialog {
            action,
            message: message.into(),
        });
    }

    /// 更新播放状态（每帧从 AudioState 读取）
    pub fn update_playback_state(&mut self) {
        if let Some(audio) = &self.audio {
            self.playback_state = audio.state();
        }
    }
}
