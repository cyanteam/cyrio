//! 后台任务循环 — Command/Event 消息传递
//!
//! 对齐 cyrio-app 架构：
//! - UI 线程持 `Sender<Command>` + `Receiver<Event>`
//! - 后台 smol 任务持反向端点，执行 USB/IO 操作
//! - 设备句柄通过 `Arc<smol::Mutex<Option<RioDevice>>>` 共享
//! - 双存储加载：ListSongs(0) + ListSongs(1) 分别请求，SongsListedForMem 合并

use std::path::PathBuf;
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use cyrio_audio::manager::{AudioState, PlaybackState};
use cyrio_core::api::device::RioDevice;
use cyrio_core::api::playlist::{
    add_to_playlist, create_playlist, list_playlist_songs,
};
use cyrio_core::api::rename::{
    batch_strip_noise, batch_to_slug, rename_song_title, repair_song_encoding, RenameResult,
};
use cyrio_core::api::types::{is_mp3_file, is_playlist_file, Song};
use cyrio_core::api::upload::{expand_paths, upload_mp3, UploadResult, UploadTextOptions};
use cyrio_core::error::{CyrioError, Result};
use cyrio_core::protocol::rio_file::RioFile;
use cyrio_core::protocol::rio_mem::RioMem;
use cyrio_transport_nusb::{list_all_usb_devices, NusbTransport};
use smol::lock::Mutex as SmolMutex;

use crate::state::MEM_UNIT_INTERNAL;

/// 内存单元编号：SD 卡
const MEM_UNIT_SD: u8 = 1;

/// UI → 后台 的命令
#[derive(Debug)]
pub enum Command {
    OpenDevice,
    OpenDeviceForce { vid: u16, pid: u16 },
    CloseDevice,
    ScanDevices,
    ListSongs(u8),
    ListPlaylists(u8),
    ListPlaylistSongs { playlist_file_no: u32, mem_unit: u8 },
    UploadSongBatch { paths: Vec<PathBuf>, mem_unit: u8, text_opts: UploadTextOptions },
    DownloadSong { file_no: u32, mem_unit: u8, save_path: PathBuf },
    DownloadSongForPlay { file_no: u32, mem_unit: u8 },
    DeleteSong { file_no: u32, mem_unit: u8 },
    AddToPlaylist { song_file_no: u32, song_mem_unit: u8, playlist_file_no: u32, playlist_mem_unit: u8 },
    CreatePlaylist { name: String, mem_unit: u8 },
    RenameSong { file_no: u32, mem_unit: u8, new_title: String },
    BatchSlugSongs { items: Vec<(u32, u8, String)> },
    BatchStripSongs { items: Vec<(u32, u8, String)>, custom_words: Vec<String> },
    RepairSongEncoding { file_no: u32, mem_unit: u8 },
    RepairAllSongsEncoding,
    BatchSlugAllSongs,
    BatchStripAllSongs { custom_words: Vec<String> },
    GetStorageStatus,
    GetPlaybackState,
    PauseAudio,
    ResumeAudio,
    StopAudio,
    Quit,
}

/// 后台 → UI 的事件
#[derive(Debug)]
pub enum Event {
    DeviceOpened(Result<()>),
    DeviceClosed,
    DevicesScanned(Vec<cyrio_transport_nusb::UsbDeviceInfo>),
    SongsListedForMem { songs: Vec<RioFile>, mem_unit: u8 },
    PlaylistsListedForMem { playlists: Vec<RioFile>, mem_unit: u8 },
    PlaylistSongsListed(Result<Vec<Song>>),
    UploadProgress { sent_bytes: u64, total_bytes: u64 },
    UploadBatchStarted { names: Vec<String> },
    UploadFileStarted { index: usize, name: String },
    UploadFileCompleted { index: usize, success: bool },
    UploadCompleted(Result<u32>),
    UploadBatchCompleted(Vec<UploadResult>),
    DownloadProgress { received_bytes: u64, total_bytes: u64 },
    DownloadCompleted(Result<()>),
    SongDownloaded(Result<Vec<u8>>),
    DeleteCompleted(Result<()>),
    AddToPlaylistCompleted(Result<()>),
    CreatePlaylistCompleted(Result<u32>),
    RenameCompleted(Result<()>),
    BatchOperationCompleted { kind: String, results: Vec<RenameResult> },
    StorageStatusGot(Result<StorageStatus>),
    PlaybackState(PlaybackState),
    Error(String),
    Log(String),
}

/// 存储状态
#[derive(Debug, Clone)]
pub struct StorageStatus {
    pub internal: StorageUnit,
    pub sd_card: StorageUnit,
}

/// 单个内存单元的存储信息
#[derive(Debug, Clone)]
pub struct StorageUnit {
    pub mem_unit: u8,
    pub present: bool,
    pub name: String,
    pub model: String,
    pub size: u64,
    pub used: u64,
    pub free: u64,
}

/// 启动后台任务循环
///
/// 返回 `(command_tx, event_rx)` — UI 线程用 command_tx 发命令，用 event_rx 收事件。
pub fn spawn_task_loop() -> (Sender<Command>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = async_channel::unbounded::<Command>();
    let (event_tx, event_rx) = async_channel::unbounded::<Event>();

    let device: Arc<SmolMutex<Option<RioDevice>>> = Arc::new(SmolMutex::new(None));
    let audio_state = cyrio_audio::manager::start_audio_thread();

    // 后台线程：驱动 smol 全局执行器
    std::thread::Builder::new()
        .name("smol-executor".into())
        .spawn(move || {
            smol::block_on(smol::future::pending::<()>());
        })
        .expect("failed to spawn smol executor thread");

    // 后台任务循环
    let device_clone = device.clone();
    let audio_clone = audio_state;
    smol::spawn(async move {
        task_loop(device_clone, audio_clone, cmd_rx, event_tx).await;
    })
    .detach();

    (cmd_tx, event_rx)
}

/// 核心任务循环
async fn task_loop(
    device: Arc<SmolMutex<Option<RioDevice>>>,
    audio: AudioState,
    cmd_rx: Receiver<Command>,
    event_tx: Sender<Event>,
) {
    loop {
        let cmd = match cmd_rx.recv().await {
            Ok(cmd) => cmd,
            Err(_) => break,
        };

        match cmd {
            Command::Quit => { let _ = audio.quit(); break; }
            Command::OpenDevice => handle_open(&device, &event_tx).await,
            Command::OpenDeviceForce { vid, pid } => handle_open_force(&device, &event_tx, vid, pid).await,
            Command::CloseDevice => handle_close(&device, &event_tx).await,
            Command::ScanDevices => handle_scan(&event_tx).await,
            Command::ListSongs(mem_unit) => handle_list_songs(&device, &event_tx, mem_unit).await,
            Command::ListPlaylists(mem_unit) => handle_list_playlists(&device, &event_tx, mem_unit).await,
            Command::ListPlaylistSongs { playlist_file_no, mem_unit } => {
                handle_list_playlist_songs(&device, &event_tx, playlist_file_no, mem_unit).await
            }
            Command::UploadSongBatch { paths, mem_unit, text_opts } => {
                handle_upload_batch(&device, &event_tx, paths, mem_unit, text_opts).await
            }
            Command::DownloadSong { file_no, mem_unit, save_path } => {
                handle_download(&device, &event_tx, file_no, mem_unit, save_path).await
            }
            Command::DownloadSongForPlay { file_no, mem_unit } => {
                handle_download_for_play(&device, &audio, &event_tx, file_no, mem_unit).await
            }
            Command::DeleteSong { file_no, mem_unit } => {
                handle_delete(&device, &event_tx, file_no, mem_unit).await
            }
            Command::AddToPlaylist { song_file_no, song_mem_unit, playlist_file_no, playlist_mem_unit } => {
                handle_add_to_playlist(&device, &event_tx, song_file_no, song_mem_unit, playlist_file_no, playlist_mem_unit).await
            }
            Command::CreatePlaylist { name, mem_unit } => {
                handle_create_playlist(&device, &event_tx, name, mem_unit).await
            }
            Command::RenameSong { file_no, mem_unit, new_title } => {
                handle_rename(&device, &event_tx, file_no, mem_unit, new_title).await
            }
            Command::BatchSlugSongs { items } => {
                handle_batch_slug(&device, &event_tx, items).await
            }
            Command::BatchStripSongs { items, custom_words } => {
                handle_batch_strip(&device, &event_tx, items, custom_words).await
            }
            Command::RepairSongEncoding { file_no, mem_unit } => {
                handle_repair_song(&device, &event_tx, file_no, mem_unit).await
            }
            Command::RepairAllSongsEncoding => {
                handle_repair_all(&device, &event_tx).await
            }
            Command::BatchSlugAllSongs => {
                handle_batch_slug_all(&device, &event_tx).await
            }
            Command::BatchStripAllSongs { custom_words } => {
                handle_batch_strip_all(&device, &event_tx, custom_words).await
            }
            Command::GetStorageStatus => {
                handle_get_storage(&device, &event_tx).await
            }
            Command::GetPlaybackState => {
                let _ = event_tx.send(Event::PlaybackState(audio.state())).await;
            }
            Command::PauseAudio => { let _ = audio.pause(); }
            Command::ResumeAudio => { let _ = audio.resume(); }
            Command::StopAudio => { let _ = audio.stop(); }
        }
    }
}

// ---- 命令处理函数 ----

async fn handle_open(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>) {
    let transport = match NusbTransport::open().await {
        Ok(t) => t,
        Err(e) => { let _ = event_tx.send(Event::DeviceOpened(Err(e))).await; return; }
    };
    let mut dev = RioDevice::new(Box::new(transport));
    match dev.open().await {
        Ok(()) => {
            *device.lock().await = Some(dev);
            let _ = event_tx.send(Event::DeviceOpened(Ok(()))).await;
        }
        Err(e) => { let _ = event_tx.send(Event::DeviceOpened(Err(e))).await; }
    }
}

async fn handle_open_force(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, vid: u16, pid: u16) {
    let transport = match NusbTransport::open_with_vid_pid(vid, pid).await {
        Ok(t) => t,
        Err(e) => { let _ = event_tx.send(Event::DeviceOpened(Err(e))).await; return; }
    };
    let mut dev = RioDevice::new(Box::new(transport));
    match dev.open().await {
        Ok(()) => {
            *device.lock().await = Some(dev);
            let _ = event_tx.send(Event::DeviceOpened(Ok(()))).await;
        }
        Err(e) => { let _ = event_tx.send(Event::DeviceOpened(Err(e))).await; }
    }
}

async fn handle_close(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>) {
    let mut guard = device.lock().await;
    if let Some(mut dev) = guard.take() { let _ = dev.close().await; }
    drop(guard);
    let _ = event_tx.send(Event::DeviceClosed).await;
}

async fn handle_scan(event_tx: &Sender<Event>) {
    match list_all_usb_devices().await {
        Ok(devices) => { let _ = event_tx.send(Event::DevicesScanned(devices)).await; }
        Err(_) => { let _ = event_tx.send(Event::DevicesScanned(Vec::new())).await; }
    }
}

async fn handle_list_songs(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, mem_unit: u8) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let songs: Vec<RioFile> = match dev.list_files(mem_unit, |_| {}).await {
            Ok(files) => files.into_iter().filter(|f| is_mp3_file(f)).collect(),
            Err(_) => Vec::new(),
        };
        let _ = event_tx.send(Event::SongsListedForMem { songs, mem_unit }).await;
    }
}

async fn handle_list_playlists(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, mem_unit: u8) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let playlists: Vec<RioFile> = match dev.list_files(mem_unit, |_| {}).await {
            Ok(files) => files.into_iter().filter(|f| is_playlist_file(f)).collect(),
            Err(_) => Vec::new(),
        };
        let _ = event_tx.send(Event::PlaylistsListedForMem { playlists, mem_unit }).await;
    }
}

async fn handle_list_playlist_songs(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, file_no: u32, mem_unit: u8) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let result = list_playlist_songs(dev, file_no, mem_unit).await;
        let mapped = result.map(|ps_list| ps_list.into_iter().map(|ps| ps.song).collect::<Vec<Song>>());
        let _ = event_tx.send(Event::PlaylistSongsListed(mapped)).await;
    }
}

async fn handle_upload_batch(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, paths: Vec<PathBuf>, mem_unit: u8, text_opts: UploadTextOptions) {
    let expanded = expand_paths(paths);
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => { let _ = event_tx.send(Event::UploadBatchCompleted(Vec::new())).await; return; }
    };

    let names: Vec<String> = expanded.iter()
        .map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string_lossy().to_string()))
        .collect();
    let _ = event_tx.send(Event::UploadBatchStarted { names: names.clone() }).await;

    let mut results = Vec::with_capacity(expanded.len());
    for (i, path) in expanded.iter().enumerate() {
        let _ = event_tx.send(Event::UploadFileStarted { index: i, name: names[i].clone() }).await;
        let tx_clone = event_tx.clone();
        let result = upload_mp3(dev, mem_unit, path, &text_opts, |p| {
            let _ = tx_clone.try_send(Event::UploadProgress { sent_bytes: p.transferred as u64, total_bytes: p.total as u64 });
        }).await;
        let success = result.is_ok();
        match result {
            Ok(file_no) => results.push(UploadResult { path: path.clone(), success: true, file_no: file_no as i64, error: String::new() }),
            Err(e) => results.push(UploadResult { path: path.clone(), success: false, file_no: -1, error: e.to_string() }),
        }
        let _ = event_tx.send(Event::UploadFileCompleted { index: i, success }).await;
    }
    let _ = event_tx.send(Event::UploadBatchCompleted(results)).await;
}

async fn handle_download(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, file_no: u32, mem_unit: u8, save_path: PathBuf) {
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => { let _ = event_tx.send(Event::DownloadCompleted(Err(CyrioError::Other("设备未连接".into())))).await; return; }
    };
    let tx_clone = event_tx.clone();
    let download = dev.download_file(mem_unit, file_no, |p| {
        let _ = tx_clone.try_send(Event::DownloadProgress { received_bytes: p.transferred as u64, total_bytes: p.total as u64 });
    }).await;
    match download {
        Ok(d) => {
            drop(guard);
            let write_result = smol::unblock(move || std::fs::write(&save_path, &d.data)).await
                .map_err(|e| CyrioError::Other(format!("写入文件失败: {e}")));
            let _ = event_tx.send(Event::DownloadCompleted(write_result)).await;
        }
        Err(e) => { let _ = event_tx.send(Event::DownloadCompleted(Err(e))).await; }
    }
}

async fn handle_download_for_play(device: &Arc<SmolMutex<Option<RioDevice>>>, audio: &AudioState, event_tx: &Sender<Event>, file_no: u32, mem_unit: u8) {
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => { let _ = event_tx.send(Event::SongDownloaded(Err(CyrioError::Other("设备未连接".into())))).await; return; }
    };
    let download = dev.download_file(mem_unit, file_no, |_| {}).await;
    match download {
        Ok(d) => {
            drop(guard);
            if let Err(e) = audio.play(d.data.clone()) {
                let _ = event_tx.send(Event::SongDownloaded(Err(CyrioError::Other(format!("播放失败: {e}"))))).await;
            } else {
                let _ = event_tx.send(Event::SongDownloaded(Ok(d.data))).await;
            }
        }
        Err(e) => { let _ = event_tx.send(Event::SongDownloaded(Err(e))).await; }
    }
}

async fn handle_delete(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, file_no: u32, mem_unit: u8) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let result = dev.delete_file(mem_unit, file_no).await;
        let _ = event_tx.send(Event::DeleteCompleted(result)).await;
    }
}

async fn handle_add_to_playlist(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, song_file_no: u32, song_mem: u8, pl_file_no: u32, pl_mem: u8) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let result = add_to_playlist(dev, song_file_no, song_mem, pl_file_no, pl_mem).await;
        let _ = event_tx.send(Event::AddToPlaylistCompleted(result)).await;
    }
}

async fn handle_create_playlist(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, name: String, mem_unit: u8) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let result = create_playlist(dev, &name, mem_unit).await;
        let _ = event_tx.send(Event::CreatePlaylistCompleted(result.map(|r| r.file_no))).await;
    }
}

async fn handle_rename(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, file_no: u32, mem_unit: u8, new_title: String) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let result = rename_song_title(dev, mem_unit, file_no, &new_title).await;
        let _ = event_tx.send(Event::RenameCompleted(result)).await;
    }
}

async fn handle_batch_slug(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, items: Vec<(u32, u8, String)>) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let results = batch_to_slug(dev, items, |_, _, _| {}).await;
        let _ = event_tx.send(Event::BatchOperationCompleted { kind: "转拼音".into(), results }).await;
    }
}

async fn handle_batch_strip(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, items: Vec<(u32, u8, String)>, custom_words: Vec<String>) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let results = batch_strip_noise(dev, items, custom_words, |_, _, _| {}).await;
        let _ = event_tx.send(Event::BatchOperationCompleted { kind: "去词".into(), results }).await;
    }
}

async fn handle_repair_song(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, file_no: u32, mem_unit: u8) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let result = repair_song_encoding(dev, mem_unit, file_no).await;
        let _ = event_tx.send(Event::RenameCompleted(result)).await;
    }
}

async fn handle_repair_all(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>) {
    let items = collect_all_song_items(device).await;
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let mut results = Vec::with_capacity(items.len());
        for (file_no, mem_unit, title) in items {
            match repair_song_encoding(dev, mem_unit, file_no).await {
                Ok(()) => results.push(RenameResult { file_no, mem_unit, success: true, original: title.clone(), new_title: title, error: String::new() }),
                Err(e) => results.push(RenameResult { file_no, mem_unit, success: false, original: title.clone(), new_title: title, error: e.to_string() }),
            }
        }
        let _ = event_tx.send(Event::BatchOperationCompleted { kind: "修复编码".into(), results }).await;
    }
}

async fn handle_batch_slug_all(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>) {
    let items = collect_all_song_items(device).await;
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let results = batch_to_slug(dev, items, |_, _, _| {}).await;
        let _ = event_tx.send(Event::BatchOperationCompleted { kind: "转拼音".into(), results }).await;
    }
}

async fn handle_batch_strip_all(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>, custom_words: Vec<String>) {
    let items = collect_all_song_items(device).await;
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let results = batch_strip_noise(dev, items, custom_words, |_, _, _| {}).await;
        let _ = event_tx.send(Event::BatchOperationCompleted { kind: "去词".into(), results }).await;
    }
}

async fn handle_get_storage(device: &Arc<SmolMutex<Option<RioDevice>>>, event_tx: &Sender<Event>) {
    let guard = device.lock().await;
    if let Some(dev) = guard.as_ref() {
        let internal = dev.get_memory_info(MEM_UNIT_INTERNAL).await;
        let sd = dev.get_memory_info(MEM_UNIT_SD).await;
        let result: Result<StorageStatus> = (|| {
            let internal = internal?;
            let sd = sd?;
            Ok(StorageStatus {
                internal: convert_mem(&internal, MEM_UNIT_INTERNAL),
                sd_card: convert_mem(&sd, MEM_UNIT_SD),
            })
        })();
        let _ = event_tx.send(Event::StorageStatusGot(result)).await;
    }
}

// ---- 辅助函数 ----

/// 收集所有歌曲 (file_no, mem_unit, title)
async fn collect_all_song_items(device: &Arc<SmolMutex<Option<RioDevice>>>) -> Vec<(u32, u8, String)> {
    let mut items = Vec::new();
    for mem_unit in [MEM_UNIT_INTERNAL, MEM_UNIT_SD] {
        let guard = device.lock().await;
        if let Some(dev) = guard.as_ref() {
            if let Ok(files) = dev.list_files(mem_unit, |_| {}).await {
                for f in files {
                    if is_mp3_file(&f) {
                        let title = if !f.title.is_empty() { f.title.clone() } else { f.name.clone() };
                        items.push((f.file_no, mem_unit, title));
                    }
                }
            }
        }
    }
    items
}

/// 把 RioMem 转为 UI 层的 StorageUnit
fn convert_mem(m: &RioMem, mem_unit: u8) -> StorageUnit {
    StorageUnit {
        mem_unit,
        present: m.is_present(),
        name: m.name.clone(),
        model: m.model.clone(),
        size: m.size as u64,
        used: m.used as u64,
        free: m.free as u64,
    }
}
