//! 后台任务循环
//!
//! UI 线程通过 `Sender<Command>` 发命令，后台 smol 任务通过 `Receiver<Command>` 接收，
//! 执行 USB/文件 IO 后用 `Sender<Event>` 回报，UI 每帧 `try_recv` 检查事件。
//!
//! ## 设备共享
//! 设备句柄通过 `Arc<smol::Mutex<Option<RioDevice>>>` 共享：
//! - TaskContext 持有克隆，用于 USB 操作
//! - AppState 持有克隆，供 WebDAV 服务器启动时使用
//! - WebDAV 服务器线程通过此 Arc 访问设备

use std::path::PathBuf;
use std::sync::Arc;

use async_channel::{Receiver, Sender};
use cyrio_core::api::device::RioDevice;
use cyrio_core::api::playlist::{
    add_to_playlist, create_playlist, list_playlist_songs, repair_playlist_encoding,
};
use cyrio_core::api::rename::{
    batch_strip_noise, batch_to_slug, rename_song_title, repair_song_encoding, RenameResult,
};
use cyrio_core::api::types::{is_mp3_file, is_playlist_file};
use cyrio_core::api::upload::{expand_paths, upload_mp3, UploadResult, UploadTextOptions};
use cyrio_core::error::{CyrioError, Result};
use cyrio_core::protocol::constants::MEM_UNIT_INTERNAL;
use cyrio_core::protocol::rio_file::RioFile;
use cyrio_core::protocol::rio_mem::RioMem;
use cyrio_transport_nusb::{list_all_usb_devices, NusbTransport};
use smol::lock::Mutex as SmolMutex;

use crate::message::{Command, Event};
use crate::state::MEM_UNIT_SD;

/// 后台任务上下文
pub struct TaskContext {
    /// 接收 UI 的命令
    pub cmd_rx: Receiver<Command>,
    /// 向 UI 发事件
    pub event_tx: Sender<Event>,
    /// 共享设备句柄（Arc<smol::Mutex<Option<RioDevice>>>）
    pub device: Arc<SmolMutex<Option<RioDevice>>>,
}

impl TaskContext {
    /// 创建任务上下文，共享设备句柄
    pub fn new(
        cmd_rx: Receiver<Command>,
        event_tx: Sender<Event>,
        device: Arc<SmolMutex<Option<RioDevice>>>,
    ) -> Self {
        Self {
            cmd_rx,
            event_tx,
            device,
        }
    }

    /// 向 UI 发事件
    async fn emit(&self, event: Event) {
        let _ = self.event_tx.send(event).await;
    }

    /// 向 UI 发日志
    async fn log(&self, msg: impl Into<String>) {
        self.emit(Event::Log(msg.into())).await;
    }

    /// 主循环
    pub async fn run(self) {
        self.log("后台任务已启动").await;

        while let Ok(cmd) = self.cmd_rx.recv().await {
            match cmd {
                Command::OpenDevice => self.handle_open().await,
                Command::OpenDeviceForce { vid, pid } => {
                    self.handle_open_force(vid, pid).await
                }
                Command::CloseDevice => self.handle_close().await,
                Command::ScanDevices => self.handle_scan_devices().await,
                Command::ListSongs(mem_unit) => self.handle_list_songs(mem_unit).await,
                Command::ListPlaylists(mem_unit) => self.handle_list_playlists(mem_unit).await,
                Command::ListPlaylistSongs {
                    playlist_file_no,
                    mem_unit,
                } => {
                    self.handle_list_playlist_songs(playlist_file_no, mem_unit)
                        .await
                }
                Command::UploadSong {
                    path,
                    mem_unit,
                    text_opts,
                } => self.handle_upload(path, mem_unit, text_opts).await,
                Command::UploadSongBatch {
                    paths,
                    mem_unit,
                    text_opts,
                } => self.handle_upload_batch(paths, mem_unit, text_opts).await,
                Command::DownloadSong {
                    file_no,
                    mem_unit,
                    save_path,
                } => {
                    self.handle_download(file_no, mem_unit, save_path).await
                }
                Command::DownloadSongForPlay { file_no, mem_unit } => {
                    self.handle_download_for_play(file_no, mem_unit).await
                }
                Command::DeleteSong { file_no, mem_unit } => {
                    self.handle_delete(file_no, mem_unit).await
                }
                Command::AddToPlaylist {
                    song_file_no,
                    song_mem_unit,
                    playlist_file_no,
                    playlist_mem_unit,
                } => {
                    self.handle_add_to_playlist(
                        song_file_no,
                        song_mem_unit,
                        playlist_file_no,
                        playlist_mem_unit,
                    )
                    .await
                }
                Command::CreatePlaylist { name, mem_unit } => {
                    self.handle_create_playlist(name, mem_unit).await
                }
                Command::RepairPlaylistEncoding { file_no, mem_unit } => {
                    self.handle_repair_playlist(file_no, mem_unit).await
                }
                Command::RenameSong {
                    file_no,
                    mem_unit,
                    new_title,
                } => self.handle_rename_song(file_no, mem_unit, new_title).await,
                Command::BatchSlugSongs { items } => self.handle_batch_slug(items).await,
                Command::BatchStripSongs { items, custom_words } => {
                    self.handle_batch_strip(items, custom_words).await
                }
                Command::RepairSongEncoding { file_no, mem_unit } => {
                    self.handle_repair_song(file_no, mem_unit).await
                }
                Command::RepairAllSongsEncoding => self.handle_repair_all_songs().await,
                Command::BatchSlugAllSongs => self.handle_batch_slug_all().await,
                Command::BatchStripAllSongs { custom_words } => {
                    self.handle_batch_strip_all(custom_words).await
                }
                Command::GetStorageStatus => self.handle_get_storage().await,
                Command::Quit => {
                    self.log("后台任务退出").await;
                    break;
                }
            }
        }

        // 清理设备
        let mut guard = self.device.lock().await;
        if let Some(mut dev) = guard.take() {
            let _ = dev.close().await;
        }
    }

    async fn handle_open(&self) {
        self.log("正在连接 Rio 设备…").await;
        let transport = match NusbTransport::open().await {
            Ok(t) => t,
            Err(e) => {
                self.emit(Event::DeviceOpened(Err(e))).await;
                return;
            }
        };
        let mut device = RioDevice::new(Box::new(transport));
        match device.open().await {
            Ok(()) => {
                self.log("Rio 设备已连接").await;
                let mut guard = self.device.lock().await;
                *guard = Some(device);
                drop(guard);
                self.emit(Event::DeviceOpened(Ok(()))).await;
            }
            Err(e) => {
                self.emit(Event::DeviceOpened(Err(e))).await;
            }
        }
    }

    async fn handle_open_force(&self, vid: u16, pid: u16) {
        self.log(format!("正在强制连接设备 vid=0x{vid:04x} pid=0x{pid:04x}…"))
            .await;
        let transport = match NusbTransport::open_with_vid_pid(vid, pid).await {
            Ok(t) => t,
            Err(e) => {
                self.emit(Event::DeviceOpened(Err(e))).await;
                return;
            }
        };
        let mut device = RioDevice::new(Box::new(transport));
        match device.open().await {
            Ok(()) => {
                self.log("设备已连接（强制模式）").await;
                let mut guard = self.device.lock().await;
                *guard = Some(device);
                drop(guard);
                self.emit(Event::DeviceOpened(Ok(()))).await;
            }
            Err(e) => {
                self.emit(Event::DeviceOpened(Err(e))).await;
            }
        }
    }

    async fn handle_close(&self) {
        let mut guard = self.device.lock().await;
        if let Some(mut dev) = guard.take() {
            let _ = dev.close().await;
        }
        drop(guard);
        self.emit(Event::DeviceClosed).await;
    }

    async fn handle_scan_devices(&self) {
        let result = list_all_usb_devices().await;
        match result {
            Ok(devices) => {
                self.emit(Event::DevicesScanned(devices)).await;
            }
            Err(e) => {
                self.log(format!("扫描 USB 设备失败: {e}")).await;
                self.emit(Event::DevicesScanned(Vec::new())).await;
            }
        }
    }

    async fn handle_list_songs(&self, mem_unit: u8) {
        let result = self.list_files_by_type(mem_unit, FileFilter::Songs).await;
        let songs = result.unwrap_or_default();
        self.emit(Event::SongsListedForMem { songs, mem_unit }).await;
    }

    async fn handle_list_playlists(&self, mem_unit: u8) {
        let result = self.list_files_by_type(mem_unit, FileFilter::Playlists).await;
        let playlists = result.unwrap_or_default();
        self.emit(Event::PlaylistsListedForMem { playlists, mem_unit }).await;
    }

    async fn handle_list_playlist_songs(&self, playlist_file_no: u32, mem_unit: u8) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::PlaylistSongsListed(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };
        let result = list_playlist_songs(dev, playlist_file_no, mem_unit).await;
        self.emit(Event::PlaylistSongsListed(result)).await;
    }

    async fn handle_upload(&self, path: PathBuf, mem_unit: u8, text_opts: UploadTextOptions) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::UploadCompleted(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };

        let event_tx = self.event_tx.clone();
        let result = upload_mp3(dev, mem_unit, &path, &text_opts, |p| {
            let _ = event_tx.try_send(Event::UploadProgress {
                sent_bytes: p.transferred as u64,
                total_bytes: p.total as u64,
            });
        })
        .await;

        self.emit(Event::UploadCompleted(result)).await;
    }

    async fn handle_upload_batch(
        &self,
        paths: Vec<PathBuf>,
        mem_unit: u8,
        text_opts: UploadTextOptions,
    ) {
        // 先展开目录（递归找 .mp3）
        let expanded = expand_paths(paths);
        self.log(format!("批量上传：{} 个文件", expanded.len())).await;

        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::UploadBatchCompleted(Vec::new())).await;
                return;
            }
        };

        // 发送批量上传开始事件（携带文件名列表，用于初始化传输对话框）
        let names: Vec<String> = expanded
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.to_string_lossy().to_string())
            })
            .collect();
        self.emit(Event::UploadBatchStarted {
            names: names.clone(),
        })
        .await;

        let mut results = Vec::with_capacity(expanded.len());
        for (i, path) in expanded.iter().enumerate() {
            let name = names[i].clone();
            // 通知 UI：开始上传第 i 个文件
            self.emit(Event::UploadFileStarted {
                index: i,
                name: name.clone(),
            })
            .await;

            let event_tx = self.event_tx.clone();
            let result = upload_mp3(dev, mem_unit, path, &text_opts, |p| {
                let _ = event_tx.try_send(Event::UploadProgress {
                    sent_bytes: p.transferred as u64,
                    total_bytes: p.total as u64,
                });
            })
            .await;

            let success = result.is_ok();
            match result {
                Ok(file_no) => results.push(UploadResult {
                    path: path.clone(),
                    success: true,
                    file_no: file_no as i64,
                    error: String::new(),
                }),
                Err(e) => results.push(UploadResult {
                    path: path.clone(),
                    success: false,
                    file_no: -1,
                    error: e.to_string(),
                }),
            }

            // 通知 UI：第 i 个文件完成
            self.emit(Event::UploadFileCompleted {
                index: i,
                success,
            })
            .await;
        }

        self.emit(Event::UploadBatchCompleted(results)).await;
    }

    async fn handle_download(&self, file_no: u32, mem_unit: u8, save_path: PathBuf) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::DownloadCompleted(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };

        let event_tx = self.event_tx.clone();
        let download = dev
            .download_file(mem_unit, file_no, |p| {
                let _ = event_tx.try_send(Event::DownloadProgress {
                    received_bytes: p.transferred as u64,
                    total_bytes: p.total as u64,
                });
            })
            .await;

        let download = match download {
            Ok(d) => d,
            Err(e) => {
                self.emit(Event::DownloadCompleted(Err(e))).await;
                return;
            }
        };

        let write_result = smol::unblock(move || std::fs::write(&save_path, &download.data))
            .await
            .map_err(|e| CyrioError::Other(format!("写入文件失败: {}", e)));

        self.emit(Event::DownloadCompleted(write_result)).await;
    }

    async fn handle_download_for_play(&self, file_no: u32, mem_unit: u8) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::SongDownloaded(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };

        let event_tx = self.event_tx.clone();
        let download = dev
            .download_file(mem_unit, file_no, |p| {
                let _ = event_tx.try_send(Event::DownloadProgress {
                    received_bytes: p.transferred as u64,
                    total_bytes: p.total as u64,
                });
            })
            .await;

        match download {
            Ok(d) => self.emit(Event::SongDownloaded(Ok(d.data))).await,
            Err(e) => self.emit(Event::SongDownloaded(Err(e))).await,
        }
    }

    async fn handle_delete(&self, file_no: u32, mem_unit: u8) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::DeleteCompleted(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };
        let result = dev.delete_file(mem_unit, file_no).await;
        self.emit(Event::DeleteCompleted(result)).await;
    }

    async fn handle_add_to_playlist(
        &self,
        song_file_no: u32,
        song_mem_unit: u8,
        playlist_file_no: u32,
        playlist_mem_unit: u8,
    ) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::AddToPlaylistCompleted(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };
        let result =
            add_to_playlist(dev, song_file_no, song_mem_unit, playlist_file_no, playlist_mem_unit)
                .await;
        self.emit(Event::AddToPlaylistCompleted(result)).await;
    }

    async fn handle_create_playlist(&self, name: String, mem_unit: u8) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::CreatePlaylistCompleted(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };
        let result = create_playlist(dev, &name, mem_unit).await;
        let mapped = result.map(|r| r.file_no);
        self.emit(Event::CreatePlaylistCompleted(mapped)).await;
    }

    async fn handle_repair_playlist(&self, file_no: u32, mem_unit: u8) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::PlaylistRepaired(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };
        let result = repair_playlist_encoding(dev, file_no, mem_unit).await;
        self.emit(Event::PlaylistRepaired(result)).await;
    }

    async fn handle_rename_song(&self, file_no: u32, mem_unit: u8, new_title: String) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::RenameCompleted(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };
        let result = rename_song_title(dev, mem_unit, file_no, &new_title).await;
        self.emit(Event::RenameCompleted(result)).await;
    }

    async fn handle_batch_slug(&self, items: Vec<(u32, u8, String)>) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::BatchOperationCompleted {
                    kind: "slug".to_string(),
                    results: Vec::new(),
                })
                .await;
                return;
            }
        };
        let results = batch_to_slug(dev, items, |_, _, _| {}).await;
        self.emit(Event::BatchOperationCompleted {
            kind: "slug".to_string(),
            results,
        })
        .await;
    }

    async fn handle_batch_strip(&self, items: Vec<(u32, u8, String)>, custom_words: Vec<String>) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::BatchOperationCompleted {
                    kind: "strip".to_string(),
                    results: Vec::new(),
                })
                .await;
                return;
            }
        };
        let results = batch_strip_noise(dev, items, custom_words, |_, _, _| {}).await;
        self.emit(Event::BatchOperationCompleted {
            kind: "strip".to_string(),
            results,
        })
        .await;
    }

    async fn handle_repair_song(&self, file_no: u32, mem_unit: u8) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::RenameCompleted(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };
        let result = repair_song_encoding(dev, mem_unit, file_no).await;
        self.emit(Event::RenameCompleted(result)).await;
    }

    async fn handle_repair_all_songs(&self) {
        let items = self.collect_all_song_items().await;
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::BatchOperationCompleted {
                    kind: "repair".to_string(),
                    results: Vec::new(),
                })
                .await;
                return;
            }
        };
        let mut results = Vec::with_capacity(items.len());
        for (file_no, mem_unit, title) in items {
            match repair_song_encoding(dev, mem_unit, file_no).await {
                Ok(()) => results.push(RenameResult {
                    file_no,
                    mem_unit,
                    success: true,
                    original: title.clone(),
                    new_title: title,
                    error: String::new(),
                }),
                Err(e) => results.push(RenameResult {
                    file_no,
                    mem_unit,
                    success: false,
                    original: title.clone(),
                    new_title: title,
                    error: e.to_string(),
                }),
            }
        }
        self.emit(Event::BatchOperationCompleted {
            kind: "repair".to_string(),
            results,
        })
        .await;
    }

    async fn handle_batch_slug_all(&self) {
        let items = self.collect_all_song_items().await;
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::BatchOperationCompleted {
                    kind: "slug".to_string(),
                    results: Vec::new(),
                })
                .await;
                return;
            }
        };
        let results = batch_to_slug(dev, items, |_, _, _| {}).await;
        self.emit(Event::BatchOperationCompleted {
            kind: "slug".to_string(),
            results,
        })
        .await;
    }

    async fn handle_batch_strip_all(&self, custom_words: Vec<String>) {
        let items = self.collect_all_song_items().await;
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::BatchOperationCompleted {
                    kind: "strip".to_string(),
                    results: Vec::new(),
                })
                .await;
                return;
            }
        };
        let results = batch_strip_noise(dev, items, custom_words, |_, _, _| {}).await;
        self.emit(Event::BatchOperationCompleted {
            kind: "strip".to_string(),
            results,
        })
        .await;
    }

    /// 收集所有歌曲 (file_no, mem_unit, title)
    ///
    /// 遍历内置存储和 SD 卡，返回所有 MP3 文件的元数据。
    /// 用于"全部转拼音/去词/修复编码"等批量操作。
    async fn collect_all_song_items(&self) -> Vec<(u32, u8, String)> {
        let mut items = Vec::new();
        for mem_unit in [MEM_UNIT_INTERNAL, MEM_UNIT_SD] {
            let guard = self.device.lock().await;
            if let Some(dev) = guard.as_ref() {
                if let Ok(files) = dev.list_files(mem_unit, |_| {}).await {
                    for f in files {
                        if is_mp3_file(&f) {
                            let title = if !f.title.is_empty() {
                                f.title.clone()
                            } else {
                                f.name.clone()
                            };
                            items.push((f.file_no, mem_unit, title));
                        }
                    }
                }
            }
        }
        items
    }

    async fn handle_get_storage(&self) {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => {
                self.emit(Event::StorageStatusGot(Err(CyrioError::Other(
                    "设备未连接".into(),
                ))))
                .await;
                return;
            }
        };

        let internal = dev.get_memory_info(MEM_UNIT_INTERNAL).await;
        let sd = dev.get_memory_info(MEM_UNIT_SD).await;

        let result: Result<crate::message::StorageStatus> = (|| {
            let internal = internal?;
            let sd = sd?;
            Ok(crate::message::StorageStatus {
                internal: convert_mem(&internal, MEM_UNIT_INTERNAL),
                sd_card: convert_mem(&sd, MEM_UNIT_SD),
            })
        })();
        self.emit(Event::StorageStatusGot(result)).await;
    }

    /// 按文件类型列举
    async fn list_files_by_type(
        &self,
        mem_unit: u8,
        filter: FileFilter,
    ) -> Result<Vec<RioFile>> {
        let guard = self.device.lock().await;
        let dev = match guard.as_ref() {
            Some(d) => d,
            None => return Err(CyrioError::Other("设备未连接".into())),
        };
        let all_files = dev.list_files(mem_unit, |_| {}).await?;
        let filtered: Vec<RioFile> = all_files
            .into_iter()
            .filter(|f| match filter {
                FileFilter::Songs => is_mp3_file(f),
                FileFilter::Playlists => is_playlist_file(f),
            })
            .collect();
        Ok(filtered)
    }
}

/// 文件类型过滤
enum FileFilter {
    Songs,
    Playlists,
}

/// 把 `RioMem` 转为 UI 层的 `StorageUnit`
fn convert_mem(m: &RioMem, mem_unit: u8) -> crate::message::StorageUnit {
    crate::message::StorageUnit {
        mem_unit,
        present: m.is_present(),
        name: m.name.clone(),
        model: m.model.clone(),
        size: m.size as u64,
        used: m.used as u64,
        free: m.free as u64,
    }
}

// ============================================================================
// 后台任务启动入口
// ============================================================================

/// 启动后台任务，返回 (`Sender<Command>`, `Receiver<Event>`)
///
/// `device` 参数是共享设备句柄，UI 层和后台任务通过它访问设备。
pub fn spawn_task_loop(
    device: Arc<SmolMutex<Option<RioDevice>>>,
) -> (Sender<Command>, Receiver<Event>) {
    let (cmd_tx, cmd_rx) = async_channel::unbounded::<Command>();
    let (event_tx, event_rx) = async_channel::unbounded::<Event>();
    let ctx = TaskContext::new(cmd_rx, event_tx, device);
    smol::spawn(async move {
        ctx.run().await;
    })
    .detach();
    (cmd_tx, event_rx)
}

/// Arc 包装版本：用于 UI 与后台共享 channel 端点的引用
pub type SharedCommandSender = Arc<Sender<Command>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_mem_sets_mem_unit() {
        let mem = RioMem {
            size: 1024,
            used: 512,
            free: 512,
            ..Default::default()
        };
        let unit = convert_mem(&mem, 1);
        assert_eq!(unit.mem_unit, 1);
        assert_eq!(unit.size, 1024);
        assert_eq!(unit.free, 512);
    }
}
