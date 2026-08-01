//! UI 状态管理 — 1:1 复刻 Tauri 版 CyrioLauncher 状态
//!
//! 核心数据结构 `SongEntry` 对齐 cyrio-app：包装 `RioFile` + `mem_unit`，
//! 支持双存储（内置+SD卡）合并显示。

use crate::task::{self, Event};
use async_channel::{Receiver, Sender};
use cyrio_audio::manager::PlaybackState;
use cyrio_core::api::types::Song;
use cyrio_core::api::upload::UploadTextOptions;
use cyrio_core::protocol::rio_file::RioFile;
use cyrio_core::protocol::rio_mem::RioMem;
use cyrio_transport_nusb::UsbDeviceInfo;
use gpui::*;
use std::collections::HashSet;

/// 内存单元编号：内置闪存
pub const MEM_UNIT_INTERNAL: u8 = 0;
/// 内存单元编号：SD 卡
pub const MEM_UNIT_SD: u8 = 1;

// ---- 导航页面（7 个 tab，与 Tauri 版一致，无传输页——传输用侧栏）----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavPage {
    Songs,
    Playlists,
    Upload,
    Sync,
    Transmission,
    DeviceInfo,
    Settings,
    About,
}

impl NavPage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Songs => "歌曲",
            Self::Playlists => "歌单",
            Self::Upload => "上传",
            Self::Sync => "同步",
            Self::Transmission => "传输",
            Self::DeviceInfo => "设备",
            Self::Settings => "设置",
            Self::About => "关于",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Songs => "♪",
            Self::Playlists => "☰",
            Self::Upload => "↑",
            Self::Sync => "⇅",
            Self::Transmission => "⇄",
            Self::DeviceInfo => "ℹ",
            Self::Settings => "⚙",
            Self::About => "⊙",
        }
    }

    pub fn all() -> &'static [NavPage] {
        &[
            Self::Songs,
            Self::Playlists,
            Self::Upload,
            Self::Sync,
            Self::Transmission,
            Self::DeviceInfo,
            Self::Settings,
            Self::About,
        ]
    }

    pub fn from_path(path: &str) -> Option<Self> {
        match path {
            "songs" => Some(Self::Songs),
            "playlists" => Some(Self::Playlists),
            "upload" => Some(Self::Upload),
            "sync" => Some(Self::Sync),
            "transmission" => Some(Self::Transmission),
            "device" => Some(Self::DeviceInfo),
            "settings" => Some(Self::Settings),
            "about" => Some(Self::About),
            _ => None,
        }
    }

    pub fn path(&self) -> &'static str {
        match self {
            Self::Songs => "songs",
            Self::Playlists => "playlists",
            Self::Upload => "upload",
            Self::Sync => "sync",
            Self::Transmission => "transmission",
            Self::DeviceInfo => "device",
            Self::Settings => "settings",
            Self::About => "about",
        }
    }
}

// ---- 排序字段（Tauri 版：名称/大小/时间）----

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortField {
    Name,
    Size,
    Time,
}

impl Default for SortField {
    fn default() -> Self { Self::Name }
}

// ---- 歌曲/歌单条目（带内存单元标记）----

/// 对齐 cyrio-app SongEntry：包装 RioFile + mem_unit
/// 用于双存储合并显示，每行用 mem-badge 区分所在存储
#[derive(Debug, Clone)]
pub struct SongEntry {
    pub file: RioFile,
    pub mem_unit: u8,
}

// ---- 设置 ----

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub upload_apply_slug: bool,
    pub upload_apply_strip: bool,
    pub strip_parentheses: bool,
    pub strip_quotes: bool,
    pub strip_quality_tags: bool,
    pub custom_stop_words: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
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
    pub fn to_text_opts(&self) -> UploadTextOptions {
        UploadTextOptions {
            apply_slug: self.upload_apply_slug,
            apply_strip: self.upload_apply_strip,
            strip_parentheses: self.strip_parentheses,
            strip_quotes: self.strip_quotes,
            strip_quality_tags: self.strip_quality_tags,
            custom_stop_words: self.custom_stop_words
                .split('\n')
                .map(|w| w.trim().to_string())
                .filter(|w| !w.is_empty())
                .collect(),
        }
    }

    pub fn custom_words_vec(&self) -> Vec<String> {
        self.custom_stop_words
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }
}

// ---- 存储信息 ----

#[derive(Debug, Clone, Default)]
pub struct StorageInfo {
    pub present: bool,
    pub size: u64,
    pub used: u64,
    pub free: u64,
    pub name: String,
    pub model: String,
}

impl StorageInfo {
    pub fn from_rio_mem(mem: &RioMem) -> Self {
        Self {
            present: mem.is_present(),
            size: mem.size as u64,
            used: mem.used as u64,
            free: mem.free as u64,
            name: mem.name.clone(),
            model: mem.model.clone(),
        }
    }

    pub fn used_pct(&self) -> f32 {
        if self.size == 0 { 0.0 } else { (self.used as f32 / self.size as f32) * 100.0 }
    }
}

// ---- 上传文件项（传输侧栏用）----

#[derive(Debug, Clone)]
pub struct UploadFileEntry {
    pub name: String,
    pub transferred: u64,
    pub total: u64,
    pub status: UploadFileStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadFileStatus {
    Pending,
    Uploading,
    Done,
    Failed,
}

/// 上传传输侧栏状态
#[derive(Debug, Clone)]
pub struct UploadTransferState {
    pub files: Vec<UploadFileEntry>,
    pub current_index: usize,
    pub done_time: Option<std::time::Instant>,
}

impl UploadTransferState {
    pub fn done_count(&self) -> usize {
        self.files.iter().filter(|f| f.status == UploadFileStatus::Done).count()
    }

    pub fn failed_count(&self) -> usize {
        self.files.iter().filter(|f| f.status == UploadFileStatus::Failed).count()
    }

    pub fn total_fraction(&self) -> f32 {
        if self.files.is_empty() { 0.0 }
        else { self.done_count() as f32 / self.files.len() as f32 }
    }

    pub fn all_done(&self) -> bool {
        !self.files.is_empty() && self.files.iter().all(|f| f.status == UploadFileStatus::Done || f.status == UploadFileStatus::Failed)
    }
}

// ---- 右键菜单 ----

/// 右键菜单状态（匹配 Tauri ContextMenu）
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
    pub file_no: u32,
    pub mem_unit: u8,
}

// ---- 二次确认对话框 ----

#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    pub action: ConfirmAction,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteSong { file_no: u32, mem_unit: u8 },
    DeleteSongsBatch { items: Vec<(u32, u8)> },
    DeletePlaylist { file_no: u32, mem_unit: u8 },
}

// ---- GPUI 根 View ----

pub struct CyrioApp {
    // 后台通信
    pub cmd_tx: Sender<task::Command>,
    pub event_rx: Receiver<Event>,

    // 连接状态
    pub connected: bool,
    pub connecting: bool,
    pub device_name: String,
    pub usb_devices: Vec<UsbDeviceInfo>,
    pub scanning: bool,
    pub last_scan_time: Option<std::time::Instant>,
    pub force_add_open: bool,

    // 导航
    pub current_page: NavPage,

    // 数据（双存储合并）
    pub songs: Vec<SongEntry>,
    pub playlists: Vec<SongEntry>,
    pub playlist_songs: Vec<Song>,
    pub selected_playlist: Option<(u32, u8)>,
    pub pending_song_loads: u8,
    pub pending_playlist_loads: u8,

    // 存储
    pub internal_mem: Option<RioMem>,
    pub sd_mem: Option<RioMem>,

    // 播放
    pub playback: PlaybackState,
    pub current_playing_file_no: Option<u32>,

    // 搜索/排序/分页
    pub search_query: String,
    pub sort_field: SortField,
    pub paginate: bool,
    pub current_page_num: usize,
    pub page_size: usize,

    // 选中
    pub selected_songs: HashSet<u32>,
    pub last_clicked_idx: Option<usize>,
    pub active_idx: Option<usize>,

    // 上传
    pub upload_target_mem: u8,
    pub upload_transfer: Option<UploadTransferState>,

    // WebDAV
    pub webdav_running: bool,
    pub webdav_addr: String,
    pub webdav_toggling: bool,

    // 设置
    pub settings: AppSettings,

    // UI 状态
    pub loading: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub notice_time: Option<std::time::Instant>,

    // 模态框
    pub confirm_dialog: Option<ConfirmDialog>,
    pub context_menu: Option<ContextMenuState>,
    pub rename_target: Option<(u32, u8, String)>,
    pub rename_input: String,
    pub create_playlist_open: bool,
    pub create_playlist_name: String,
    pub create_playlist_mem: u8,
    pub show_playlist_picker: bool,
    pub picker_song_file_no: Option<u32>,
    pub show_more_menu: bool,
}

impl CyrioApp {
    pub fn new(cmd_tx: Sender<task::Command>, event_rx: Receiver<Event>) -> Self {
        Self {
            cmd_tx,
            event_rx,
            connected: false,
            connecting: false,
            device_name: String::new(),
            usb_devices: Vec::new(),
            scanning: false,
            last_scan_time: None,
            force_add_open: false,
            current_page: NavPage::Songs,
            songs: Vec::new(),
            playlists: Vec::new(),
            playlist_songs: Vec::new(),
            selected_playlist: None,
            pending_song_loads: 0,
            pending_playlist_loads: 0,
            internal_mem: None,
            sd_mem: None,
            playback: PlaybackState { is_playing: false, position: 0.0, duration: 0.0, is_loading: false },
            current_playing_file_no: None,
            search_query: String::new(),
            sort_field: SortField::default(),
            paginate: false,
            current_page_num: 0,
            page_size: 10,
            selected_songs: HashSet::new(),
            last_clicked_idx: None,
            active_idx: None,
            upload_target_mem: 0,
            upload_transfer: None,
            webdav_running: false,
            webdav_addr: String::new(),
            webdav_toggling: false,
            settings: AppSettings::default(),
            loading: false,
            error: None,
            notice: None,
            notice_time: None,
            confirm_dialog: None,
            context_menu: None,
            rename_target: None,
            rename_input: String::new(),
            create_playlist_open: false,
            create_playlist_name: String::new(),
            create_playlist_mem: 0,
            show_playlist_picker: false,
            picker_song_file_no: None,
            show_more_menu: false,
        }
    }

    pub fn send_cmd(&self, cmd: task::Command) {
        let _ = self.cmd_tx.send_blocking(cmd);
    }

    pub fn navigate(&mut self, page: NavPage, cx: &mut Context<Self>) {
        self.current_page = page;
        match page {
            NavPage::Songs => {
                self.loading = true;
                self.pending_song_loads = 2;
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            NavPage::Playlists => {
                self.loading = true;
                self.pending_playlist_loads = 2;
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_SD));
            }
            NavPage::DeviceInfo => {
                self.send_cmd(task::Command::GetStorageStatus);
            }
            _ => {}
        }
        cx.notify();
    }

    pub fn show_notice(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        self.notice = Some(msg.into());
        self.notice_time = Some(std::time::Instant::now());
        cx.notify();
    }

    /// 过滤+排序后的歌曲列表
    pub fn filtered_songs(&self) -> Vec<SongEntry> {
        let mut filtered: Vec<SongEntry> = if self.search_query.trim().is_empty() {
            self.songs.clone()
        } else {
            let q = self.search_query.trim().to_lowercase();
            self.songs.iter()
                .filter(|e| {
                    e.file.title.to_lowercase().contains(&q)
                        || e.file.artist.to_lowercase().contains(&q)
                        || e.file.album.to_lowercase().contains(&q)
                        || e.file.name.to_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        };
        match self.sort_field {
            SortField::Name => filtered.sort_by(|a, b| display_title(&a.file).cmp(&display_title(&b.file))),
            SortField::Size => filtered.sort_by(|a, b| b.file.size.cmp(&a.file.size)),
            SortField::Time => filtered.sort_by(|a, b| b.file.time.cmp(&a.file.time)),
        }
        filtered
    }

    /// 分页后的歌曲
    pub fn paged_songs(&self) -> Vec<SongEntry> {
        let filtered = self.filtered_songs();
        if !self.paginate { return filtered; }
        let start = self.current_page_num * self.page_size;
        filtered.into_iter().skip(start).take(self.page_size).collect()
    }

    pub fn total_pages(&self) -> usize {
        if !self.paginate || self.page_size == 0 { return 1; }
        let total = self.filtered_songs().len();
        total.div_ceil(self.page_size).max(1)
    }

    /// 处理后台事件
    pub fn handle_event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::DeviceOpened(Ok(())) => {
                self.connected = true;
                self.connecting = false;
                self.error = None;
                self.current_page = NavPage::Songs;
                self.show_notice("设备已连接", cx);
                self.send_cmd(task::Command::GetStorageStatus);
                self.pending_song_loads = 2;
                self.pending_playlist_loads = 2;
                self.loading = true;
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_SD));
            }
            Event::DeviceOpened(Err(e)) => {
                self.connected = false;
                self.connecting = false;
                self.show_notice(format!("连接失败：{e}"), cx);
            }
            Event::DeviceClosed => {
                self.connected = false;
                self.songs.clear();
                self.playlists.clear();
                self.playlist_songs.clear();
                self.internal_mem = None;
                self.sd_mem = None;
                self.selected_songs.clear();
                self.current_playing_file_no = None;
                self.show_notice("设备已断开", cx);
            }
            Event::DevicesScanned(devices) => {
                self.usb_devices = devices;
                self.scanning = false;
            }
            Event::Error(msg) => {
                self.error = Some(msg.clone());
                self.loading = false;
                self.connecting = false;
                self.show_notice(msg, cx);
            }
            Event::SongsListedForMem { songs, mem_unit } => {
                // 双存储合并：移除该 mem_unit 的旧项，追加新项
                self.songs.retain(|e| e.mem_unit != mem_unit);
                for file in songs {
                    self.songs.push(SongEntry { file, mem_unit });
                }
                if self.pending_song_loads > 0 { self.pending_song_loads -= 1; }
                if self.pending_song_loads == 0 { self.loading = false; }
            }
            Event::PlaylistsListedForMem { playlists, mem_unit } => {
                self.playlists.retain(|e| e.mem_unit != mem_unit);
                for file in playlists {
                    self.playlists.push(SongEntry { file, mem_unit });
                }
                if self.pending_playlist_loads > 0 { self.pending_playlist_loads -= 1; }
                if self.pending_playlist_loads == 0 { self.loading = false; }
            }
            Event::PlaylistSongsListed(Ok(songs)) => {
                self.playlist_songs = songs;
                self.loading = false;
            }
            Event::PlaylistSongsListed(Err(e)) => {
                self.loading = false;
                self.show_notice(format!("读取歌单失败：{e}"), cx);
            }
            Event::PlaybackState(state) => { self.playback = state; }
            Event::DeleteCompleted(Ok(())) => {
                self.show_notice("删除完成", cx);
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::DeleteCompleted(Err(e)) => {
                self.show_notice(format!("删除失败：{e}"), cx);
            }
            Event::DownloadProgress { .. } => { /* 下载进度由 UI 轮询处理 */ }
            Event::DownloadCompleted(Ok(())) => { self.show_notice("下载完成", cx); }
            Event::DownloadCompleted(Err(e)) => { self.show_notice(format!("下载失败：{e}"), cx); }
            Event::SongDownloaded(Ok(_data)) => {
                // 音频播放由 task.rs 内部处理
                self.show_notice("开始播放", cx);
            }
            Event::SongDownloaded(Err(e)) => {
                self.show_notice(format!("播放下载失败：{e}"), cx);
            }
            Event::UploadProgress { sent_bytes, total_bytes } => {
                if let Some(ut) = self.upload_transfer.as_mut() {
                    if let Some(f) = ut.files.get_mut(ut.current_index) {
                        f.transferred = sent_bytes;
                        f.total = total_bytes;
                    }
                }
            }
            Event::UploadBatchStarted { names } => {
                let files = names.iter().map(|n| UploadFileEntry {
                    name: n.clone(), transferred: 0, total: 0, status: UploadFileStatus::Pending,
                }).collect();
                self.upload_transfer = Some(UploadTransferState {
                    files, current_index: 0, done_time: None,
                });
            }
            Event::UploadFileStarted { index, name: _ } => {
                if let Some(ut) = self.upload_transfer.as_mut() {
                    ut.current_index = index;
                    if let Some(f) = ut.files.get_mut(index) {
                        f.status = UploadFileStatus::Uploading;
                    }
                }
            }
            Event::UploadFileCompleted { index, success } => {
                if let Some(ut) = self.upload_transfer.as_mut() {
                    if let Some(f) = ut.files.get_mut(index) {
                        f.status = if success { UploadFileStatus::Done } else { UploadFileStatus::Failed };
                        if success { f.transferred = f.total; }
                    }
                }
            }
            Event::UploadBatchCompleted(results) => {
                let success = results.iter().filter(|r| r.success).count();
                let fail = results.len() - success;
                if let Some(ut) = self.upload_transfer.as_mut() {
                    ut.done_time = Some(std::time::Instant::now());
                }
                if fail == 0 {
                    self.show_notice(format!("上传完成：成功 {success} 首"), cx);
                } else {
                    self.show_notice(format!("上传完成：成功 {success} 首，失败 {fail} 首"), cx);
                }
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::UploadCompleted(Ok(_)) => {
                self.show_notice("上传完成", cx);
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::UploadCompleted(Err(e)) => {
                self.show_notice(format!("上传失败：{e}"), cx);
            }
            Event::CreatePlaylistCompleted(Ok(_)) => {
                self.create_playlist_open = false;
                self.create_playlist_name.clear();
                self.show_notice("歌单已创建", cx);
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_SD));
            }
            Event::CreatePlaylistCompleted(Err(e)) => {
                self.show_notice(format!("创建歌单失败：{e}"), cx);
            }
            Event::AddToPlaylistCompleted(Ok(())) => {
                self.show_playlist_picker = false;
                self.picker_song_file_no = None;
                self.show_notice("已加入歌单", cx);
            }
            Event::AddToPlaylistCompleted(Err(e)) => {
                self.show_notice(format!("加入歌单失败：{e}"), cx);
            }
            Event::RenameCompleted(Ok(())) => {
                self.rename_target = None;
                self.rename_input.clear();
                self.show_notice("已重命名", cx);
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::RenameCompleted(Err(e)) => {
                self.show_notice(format!("重命名失败：{e}"), cx);
            }
            Event::BatchOperationCompleted { kind, results } => {
                let success = results.iter().filter(|r| r.success).count();
                let fail = results.len() - success;
                if fail == 0 {
                    self.show_notice(format!("{kind}完成：成功 {success} 项"), cx);
                } else {
                    self.show_notice(format!("{kind}完成：成功 {success} 项，失败 {fail} 项"), cx);
                }
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::StorageStatusGot(Ok(status)) => {
                self.internal_mem = Some(RioMem {
                    size: status.internal.size as u32,
                    used: status.internal.used as u32,
                    free: status.internal.free as u32,
                    system: 0,
                    name: status.internal.name,
                    model: status.internal.model,
                });
                self.sd_mem = Some(RioMem {
                    size: status.sd_card.size as u32,
                    used: status.sd_card.used as u32,
                    free: status.sd_card.free as u32,
                    system: 0,
                    name: status.sd_card.name,
                    model: status.sd_card.model,
                });
            }
            Event::StorageStatusGot(Err(e)) => {
                self.show_notice(format!("读取存储信息失败：{e}"), cx);
            }
            Event::Log(_msg) => { /* 日志暂时忽略 */ }
        }
        cx.notify();
    }

    /// 轮询后台事件（静默模式：不调用 cx.notify()，返回是否有状态变化）
    pub fn poll_events_quiet(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.event_rx.try_recv() {
            // 标记有变化，但不调用 cx.notify()
            changed = true;
            // 内联 handle_event 逻辑但不调用 cx.notify()
            self.handle_event_quiet(event);
        }
        // 通知自动消失（3 秒）
        if let Some(t) = self.notice_time {
            if t.elapsed().as_secs() > 3 {
                self.notice = None;
                self.notice_time = None;
                changed = true;
            }
        }
        // 上传传输完成后延迟 1.5 秒清除
        if let Some(ut) = self.upload_transfer.as_ref() {
            if let Some(t) = ut.done_time {
                if t.elapsed().as_millis() > 1500 {
                    self.upload_transfer = None;
                    changed = true;
                }
            }
        }
        changed
    }

    /// 处理事件（静默模式：不调用 cx.notify()）
    fn handle_event_quiet(&mut self, event: Event) {
        match event {
            Event::DeviceOpened(Ok(())) => {
                self.connected = true;
                self.connecting = false;
                self.error = None;
                self.current_page = NavPage::Songs;
                self.notice = Some("设备已连接".into());
                self.notice_time = Some(std::time::Instant::now());
                self.send_cmd(task::Command::GetStorageStatus);
                self.pending_song_loads = 2;
                self.pending_playlist_loads = 2;
                self.loading = true;
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_SD));
            }
            Event::DeviceOpened(Err(e)) => {
                self.connected = false;
                self.connecting = false;
                self.notice = Some(format!("连接失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::DeviceClosed => {
                self.connected = false;
                self.songs.clear();
                self.playlists.clear();
                self.playlist_songs.clear();
                self.internal_mem = None;
                self.sd_mem = None;
                self.selected_songs.clear();
                self.current_playing_file_no = None;
                self.notice = Some("设备已断开".into());
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::DevicesScanned(devices) => {
                self.usb_devices = devices;
                self.scanning = false;
            }
            Event::Error(msg) => {
                self.error = Some(msg.clone());
                self.loading = false;
                self.connecting = false;
                self.notice = Some(msg);
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::SongsListedForMem { songs, mem_unit } => {
                self.songs.retain(|e| e.mem_unit != mem_unit);
                for file in songs {
                    self.songs.push(SongEntry { file, mem_unit });
                }
                if self.pending_song_loads > 0 { self.pending_song_loads -= 1; }
                if self.pending_song_loads == 0 { self.loading = false; }
            }
            Event::PlaylistsListedForMem { playlists, mem_unit } => {
                self.playlists.retain(|e| e.mem_unit != mem_unit);
                for file in playlists {
                    self.playlists.push(SongEntry { file, mem_unit });
                }
                if self.pending_playlist_loads > 0 { self.pending_playlist_loads -= 1; }
                if self.pending_playlist_loads == 0 { self.loading = false; }
            }
            Event::PlaylistSongsListed(Ok(songs)) => {
                self.playlist_songs = songs;
                self.loading = false;
            }
            Event::PlaylistSongsListed(Err(e)) => {
                self.loading = false;
                self.notice = Some(format!("读取歌单失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::PlaybackState(state) => { self.playback = state; }
            Event::DeleteCompleted(Ok(())) => {
                self.notice = Some("删除完成".into());
                self.notice_time = Some(std::time::Instant::now());
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::DeleteCompleted(Err(e)) => {
                self.notice = Some(format!("删除失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::DownloadCompleted(Ok(())) => {
                self.notice = Some("下载完成".into());
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::DownloadCompleted(Err(e)) => {
                self.notice = Some(format!("下载失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::SongDownloaded(Ok(_data)) => {
                self.notice = Some("开始播放".into());
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::SongDownloaded(Err(e)) => {
                self.notice = Some(format!("播放下载失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::UploadProgress { sent_bytes, total_bytes } => {
                if let Some(ut) = self.upload_transfer.as_mut() {
                    if let Some(f) = ut.files.get_mut(ut.current_index) {
                        f.transferred = sent_bytes;
                        f.total = total_bytes;
                    }
                }
            }
            Event::UploadBatchStarted { names } => {
                let files = names.iter().map(|n| UploadFileEntry {
                    name: n.clone(), transferred: 0, total: 0, status: UploadFileStatus::Pending,
                }).collect();
                self.upload_transfer = Some(UploadTransferState {
                    files, current_index: 0, done_time: None,
                });
            }
            Event::UploadFileStarted { index, name: _ } => {
                if let Some(ut) = self.upload_transfer.as_mut() {
                    ut.current_index = index;
                    if let Some(f) = ut.files.get_mut(index) {
                        f.status = UploadFileStatus::Uploading;
                    }
                }
            }
            Event::UploadFileCompleted { index, success } => {
                if let Some(ut) = self.upload_transfer.as_mut() {
                    if let Some(f) = ut.files.get_mut(index) {
                        f.status = if success { UploadFileStatus::Done } else { UploadFileStatus::Failed };
                        if success { f.transferred = f.total; }
                    }
                }
            }
            Event::UploadBatchCompleted(results) => {
                let success = results.iter().filter(|r| r.success).count();
                let fail = results.len() - success;
                if let Some(ut) = self.upload_transfer.as_mut() {
                    ut.done_time = Some(std::time::Instant::now());
                }
                if fail == 0 {
                    self.notice = Some(format!("上传完成：成功 {success} 首"));
                } else {
                    self.notice = Some(format!("上传完成：成功 {success} 首，失败 {fail} 首"));
                }
                self.notice_time = Some(std::time::Instant::now());
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::UploadCompleted(Ok(_)) => {
                self.notice = Some("上传完成".into());
                self.notice_time = Some(std::time::Instant::now());
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::UploadCompleted(Err(e)) => {
                self.notice = Some(format!("上传失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::CreatePlaylistCompleted(Ok(_)) => {
                self.create_playlist_open = false;
                self.create_playlist_name.clear();
                self.notice = Some("歌单已创建".into());
                self.notice_time = Some(std::time::Instant::now());
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListPlaylists(MEM_UNIT_SD));
            }
            Event::CreatePlaylistCompleted(Err(e)) => {
                self.notice = Some(format!("创建歌单失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::AddToPlaylistCompleted(Ok(())) => {
                self.show_playlist_picker = false;
                self.picker_song_file_no = None;
                self.notice = Some("已加入歌单".into());
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::AddToPlaylistCompleted(Err(e)) => {
                self.notice = Some(format!("加入歌单失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::RenameCompleted(Ok(())) => {
                self.rename_target = None;
                self.rename_input.clear();
                self.notice = Some("已重命名".into());
                self.notice_time = Some(std::time::Instant::now());
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::RenameCompleted(Err(e)) => {
                self.notice = Some(format!("重命名失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::BatchOperationCompleted { kind, results } => {
                let success = results.iter().filter(|r| r.success).count();
                let fail = results.len() - success;
                if fail == 0 {
                    self.notice = Some(format!("{kind}完成：成功 {success} 项"));
                } else {
                    self.notice = Some(format!("{kind}完成：成功 {success} 项，失败 {fail} 项"));
                }
                self.notice_time = Some(std::time::Instant::now());
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_INTERNAL));
                self.send_cmd(task::Command::ListSongs(MEM_UNIT_SD));
            }
            Event::StorageStatusGot(Ok(status)) => {
                self.internal_mem = Some(RioMem {
                    size: status.internal.size as u32,
                    used: status.internal.used as u32,
                    free: status.internal.free as u32,
                    system: 0,
                    name: status.internal.name,
                    model: status.internal.model,
                });
                self.sd_mem = Some(RioMem {
                    size: status.sd_card.size as u32,
                    used: status.sd_card.used as u32,
                    free: status.sd_card.free as u32,
                    system: 0,
                    name: status.sd_card.name,
                    model: status.sd_card.model,
                });
            }
            Event::StorageStatusGot(Err(e)) => {
                self.notice = Some(format!("读取存储信息失败：{e}"));
                self.notice_time = Some(std::time::Instant::now());
            }
            Event::DownloadProgress { .. } => { /* 下载进度由 UI 轮询处理 */ }
            Event::Log(_msg) => { /* 日志暂时忽略 */ }
        }
    }

    /// 轮询后台事件（兼容旧接口，调用 cx.notify()）
    pub fn poll_events(&mut self, cx: &mut Context<Self>) {
        let changed = self.poll_events_quiet();
        if changed {
            cx.notify();
        }
    }
}

// ---- 格式化函数（与 Tauri 版一致）----

pub fn format_time(sec: u32) -> String {
    if sec == 0 { return "0:00".into(); }
    format!("{}:{:02}", sec / 60, sec % 60)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB { format!("{:.2} GB", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.1} MB", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.1} KB", bytes as f64 / KB as f64) }
    else { format!("{} B", bytes) }
}

/// 歌曲标题显示：title 为空时用 name 字段兜底
pub fn display_title(file: &RioFile) -> String {
    if !file.title.is_empty() {
        return file.title.clone();
    }
    let name = &file.name;
    if name.is_empty() {
        return "(无标题)".to_string();
    }
    let base = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let without_ext = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    if without_ext.is_empty() { "(无标题)".to_string() }
    else { without_ext.to_string() }
}
