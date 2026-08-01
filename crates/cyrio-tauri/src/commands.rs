//! Tauri 2.0 命令绑定
//!
//! 把 [`cyrio_core`] 的 API 包装成 `#[tauri::command]`。
//!
//! ## 为什么单独放一个模块？
//! Tauri 2.0 的 `#[tauri::command]` 宏会生成 `__cmd__*` macro_rules。
//! 在 crate 根模块（lib.rs）中如果给命令加 `pub`，会触发 E0255
//! "defined multiple times"。放到子模块后命令可以保持 `pub`，
//! 主程序通过 `cyrio_tauri::commands::*` 路径访问。

use std::sync::Arc;
use std::time::{Duration, Instant};

use cyrio_core::api::device::RioDevice;
use cyrio_core::error::CyrioError;
use smol::lock::Mutex;
use tauri::{AppHandle, Emitter};

use crate::{DeviceInfo, SongInfo, StorageInfo, UsbDeviceListItem};

/// songs 缓存条目（按 mem_unit 分开，10 秒 TTL）
struct SongsCacheEntry {
    songs: Vec<SongInfo>,
    fetched_at: Instant,
}

/// songs 缓存 TTL
const SONGS_CACHE_TTL: Duration = Duration::from_secs(10);

/// 全局共享的设备状态
///
/// `Arc<smol::Mutex<Option<RioDevice>>>`：
/// - `Arc` 让 Tauri State 可以共享
/// - `smol::Mutex` 因为 RioDevice 的 API 是 smol async
/// - `Option` 因为设备可能未连接
pub struct DeviceState {
    /// 当前连接的设备（None 表示未连接）
    pub(crate) device: Arc<Mutex<Option<RioDevice>>>,
    /// songs 缓存：[内置, SD]，10 秒 TTL，上传/删除后手动失效
    songs_cache: Arc<Mutex<[Option<SongsCacheEntry>; 2]>>,
}

impl DeviceState {
    /// 创建空的设备状态（未连接）
    pub fn new() -> Self {
        Self {
            device: Arc::new(Mutex::new(None)),
            songs_cache: Arc::new(Mutex::new([None, None])),
        }
    }

    /// 失效指定 mem_unit 的 songs 缓存（上传/删除后调用）
    pub(crate) async fn invalidate_songs_cache(&self, mem_unit: u8) {
        let mut cache = self.songs_cache.lock().await;
        cache[mem_unit as usize] = None;
    }

    /// 清空所有 songs 缓存（断开连接时调用）
    async fn clear_songs_cache(&self) {
        let mut cache = self.songs_cache.lock().await;
        cache[0] = None;
        cache[1] = None;
    }
}

impl Default for DeviceState {
    fn default() -> Self {
        Self::new()
    }
}

/// 把 CyrioError 转成前端可读的字符串
fn err_string(e: CyrioError) -> String {
    e.to_string()
}

/// 上传/下载字节级进度事件 payload（Phase 3b）
///
/// 通过 `app.emit("upload-progress", payload)` / `app.emit("download-progress", payload)`
/// 发送到前端，前端用 `listen` 监听并显示真实进度。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ByteProgressPayload {
    /// 已传输字节数
    pub transferred: u32,
    /// 总字节数
    pub total: u32,
}

// ============================================================================
// Tauri 命令
// ============================================================================

/// 打开 Rio 设备
#[tauri::command]
pub async fn open_device(state: tauri::State<'_, DeviceState>) -> Result<DeviceInfo, String> {
    let transport = cyrio_transport_nusb::NusbTransport::open()
        .await
        .map_err(err_string)?;
    let mut device = RioDevice::new(Box::new(transport));
    device.open().await.map_err(err_string)?;
    let info = DeviceInfo {
        connected: true,
        model: "Rio S-Series".to_string(),
    };
    let mut guard = state.device.lock().await;
    *guard = Some(device);
    Ok(info)
}

/// 关闭设备连接
#[tauri::command]
pub async fn close_device(state: tauri::State<'_, DeviceState>) -> Result<(), String> {
    let mut guard = state.device.lock().await;
    *guard = None;
    drop(guard);
    state.clear_songs_cache().await;
    Ok(())
}

/// 列出所有已连接的 USB 设备
///
/// 用于"强制添加设备"功能。返回系统中所有 USB 设备列表，
/// 前端可让用户选择任意设备当作 Rio 设备进行传输。
#[tauri::command]
pub async fn list_usb_devices() -> Result<Vec<UsbDeviceListItem>, String> {
    let devices = cyrio_transport_nusb::list_all_usb_devices()
        .await
        .map_err(err_string)?;
    Ok(devices
        .into_iter()
        .map(|d| {
            let is_diamond = d.vid == 0x045a;
            UsbDeviceListItem {
                vid: format!("0x{:04x}", d.vid),
                pid: format!("0x{:04x}", d.pid),
                vid_num: d.vid,
                pid_num: d.pid,
                name: d.name,
                manufacturer: d.manufacturer,
                is_diamond,
            }
        })
        .collect())
}

/// 强制以指定 VID/PID 打开任意 USB 设备作为 Rio 设备
///
/// 绕过自动检测，直接尝试以 Rio 协议打开指定 VID/PID 的 USB 设备。
/// 用于"强制添加设备"功能。
#[tauri::command]
pub async fn open_device_force(
    state: tauri::State<'_, DeviceState>,
    vid: u16,
    pid: u16,
) -> Result<DeviceInfo, String> {
    let transport = cyrio_transport_nusb::NusbTransport::open_with_vid_pid(vid, pid)
        .await
        .map_err(err_string)?;
    let mut device = RioDevice::new(Box::new(transport));
    device.open().await.map_err(err_string)?;
    let info = DeviceInfo {
        connected: true,
        model: format!("USB 0x{:04x}:0x{:04x}", vid, pid),
    };
    let mut guard = state.device.lock().await;
    *guard = Some(device);
    Ok(info)
}

/// 检查设备是否已连接
#[tauri::command]
pub async fn is_connected(state: tauri::State<'_, DeviceState>) -> Result<bool, String> {
    let guard = state.device.lock().await;
    Ok(guard.is_some())
}

/// 列出歌曲
///
/// 优先返回 10 秒缓存（标签页切换时秒回）；未命中则走 USB 读取并写入缓存。
/// 上传/删除后会手动失效缓存。
#[tauri::command]
pub async fn list_songs(
    state: tauri::State<'_, DeviceState>,
    mem_unit: u8,
) -> Result<Vec<SongInfo>, String> {
    // 1. 查缓存
    {
        let cache = state.songs_cache.lock().await;
        if let Some(entry) = &cache[mem_unit as usize] {
            if entry.fetched_at.elapsed() < SONGS_CACHE_TTL {
                return Ok(entry.songs.clone());
            }
        }
    }
    // 2. 未命中：走 USB
    let files = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        device
            .list_files(mem_unit, |_| {})
            .await
            .map_err(err_string)?
    };
    let songs: Vec<SongInfo> = files
        .iter()
        .filter(|f| f.file_type == cyrio_core::protocol::constants::TYPE_MP3)
        .map(|f| {
            let mut info: SongInfo = f.into();
            info.mem_unit = mem_unit;
            info
        })
        .collect();
    // 3. 写缓存
    let mut cache = state.songs_cache.lock().await;
    cache[mem_unit as usize] = Some(SongsCacheEntry {
        songs: songs.clone(),
        fetched_at: Instant::now(),
    });
    Ok(songs)
}

/// 列出歌单
#[tauri::command]
pub async fn list_playlists(
    state: tauri::State<'_, DeviceState>,
    mem_unit: u8,
) -> Result<Vec<SongInfo>, String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    let files = device
        .list_files(mem_unit, |_| {})
        .await
        .map_err(err_string)?;
    let playlists: Vec<SongInfo> = files
        .iter()
        .filter(|f| f.file_type == cyrio_core::protocol::constants::TYPE_PLS)
        .map(|f| {
            let mut info: SongInfo = f.into();
            info.mem_unit = mem_unit;
            info
        })
        .collect();
    Ok(playlists)
}

/// 列出歌单内的歌曲（含跨存储引用）
#[tauri::command]
pub async fn list_playlist_songs(
    state: tauri::State<'_, DeviceState>,
    playlist_file_no: u32,
    mem_unit: u8,
) -> Result<Vec<SongInfo>, String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    let songs = cyrio_core::api::playlist::list_playlist_songs(
        device,
        playlist_file_no,
        mem_unit,
    )
    .await
    .map_err(err_string)?;
    Ok(songs
        .iter()
        .map(|ps| SongInfo {
            file_no: ps.song.file_no,
            size: ps.song.size,
            time: ps.song.time,
            name: ps.song.name.clone(),
            title: ps.song.title.clone(),
            artist: ps.song.artist.clone(),
            album: ps.song.album.clone(),
            bit_rate: ps.song.bit_rate,
            mem_unit: ps.mem_unit,
        })
        .collect())
}

/// 获取存储信息
#[tauri::command]
pub async fn get_storage(
    state: tauri::State<'_, DeviceState>,
    mem_unit: u8,
) -> Result<StorageInfo, String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    let mem = device
        .get_memory_info(mem_unit)
        .await
        .map_err(err_string)?;
    let mut info: StorageInfo = mem.into();
    info.mem_unit = mem_unit;
    Ok(info)
}

/// 上传歌曲
///
/// `text_opts`：可选的文本处理选项（slug/strip），与 playlist 编码同步。
/// 前端可传 null/undefined 使用默认（不应用 slug/strip）。
#[tauri::command]
pub async fn upload_song(
    state: tauri::State<'_, DeviceState>,
    app: AppHandle,
    path: String,
    mem_unit: u8,
    text_opts: Option<cyrio_core::api::upload::UploadTextOptions>,
) -> Result<u32, String> {
    let text_opts = text_opts.unwrap_or_default();
    let path = std::path::PathBuf::from(path);
    let file_no = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        let app_handle = app.clone();
        cyrio_core::api::upload::upload_mp3(device, mem_unit, &path, &text_opts, |p| {
            let _ = app_handle.emit(
                "upload-progress",
                ByteProgressPayload {
                    transferred: p.transferred,
                    total: p.total,
                },
            );
        })
        .await
        .map_err(err_string)?
    };
    state.invalidate_songs_cache(mem_unit).await;
    Ok(file_no)
}

/// 批量上传结果（单个文件）
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchUploadResult {
    /// 文件路径
    pub path: String,
    /// 是否成功
    pub success: bool,
    /// 成功时为 file_no，失败时为 -1
    pub file_no: i64,
    /// 错误信息（失败时）
    pub error: String,
}

/// 批量上传多个 MP3 文件
///
/// 用于拖拽上传：用户拖入多个文件/目录，前端展开成路径数组后调用此命令。
/// 逐个上传，返回每个文件的结果。上传期间通过 `upload-progress` 事件发送字节级进度。
///
/// `text_opts`：可选的文本处理选项（slug/strip），与 playlist 编码同步。
/// 前端可传 null/undefined 使用默认（不应用 slug/strip）。
#[tauri::command]
pub async fn upload_song_batch(
    state: tauri::State<'_, DeviceState>,
    app: AppHandle,
    paths: Vec<String>,
    mem_unit: u8,
    text_opts: Option<cyrio_core::api::upload::UploadTextOptions>,
) -> Result<Vec<BatchUploadResult>, String> {
    let text_opts = text_opts.unwrap_or_default();
    let path_bufs: Vec<std::path::PathBuf> =
        paths.into_iter().map(std::path::PathBuf::from).collect();
    let results = {
        let guard = state.device.lock().await;
        let device = match guard.as_ref() {
            Some(d) => d,
            None => return Err("设备未连接".to_string()),
        };
        let app_handle = app.clone();
        cyrio_core::api::upload::upload_mp3_batch(device, mem_unit, path_bufs, &text_opts, |p| {
            let _ = app_handle.emit(
                "upload-progress",
                ByteProgressPayload {
                    transferred: p.transferred,
                    total: p.total,
                },
            );
        })
        .await
    };
    state.invalidate_songs_cache(mem_unit).await;
    Ok(results
        .into_iter()
        .map(|r| BatchUploadResult {
            path: r.path.to_string_lossy().into_owned(),
            success: r.success,
            file_no: r.file_no,
            error: r.error,
        })
        .collect())
}

/// 展开路径数组中的目录，递归收集所有 .mp3 文件
///
/// 用于拖拽上传：用户可能拖入文件或目录，此命令把目录展开成 .mp3 文件列表。
/// 文件直接保留（非 .mp3 文件会被过滤）。
#[tauri::command]
pub async fn expand_paths(paths: Vec<String>) -> Result<Vec<String>, String> {
    let path_bufs: Vec<std::path::PathBuf> =
        paths.into_iter().map(std::path::PathBuf::from).collect();
    let collected = smol::unblock(move || cyrio_core::api::upload::expand_paths(path_bufs)).await;
    Ok(collected
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// 下载歌曲到本地
///
/// 下载期间通过 `download-progress` 事件发送字节级进度。
#[tauri::command]
pub async fn download_song(
    state: tauri::State<'_, DeviceState>,
    app: AppHandle,
    file_no: u32,
    mem_unit: u8,
    save_path: String,
) -> Result<(), String> {
    let result = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        let app_handle = app.clone();
        device
            .download_file(mem_unit, file_no, |p| {
                let _ = app_handle.emit(
                    "download-progress",
                    ByteProgressPayload {
                        transferred: p.transferred,
                        total: p.total,
                    },
                );
            })
            .await
            .map_err(err_string)?
    };
    smol::unblock(move || std::fs::write(&save_path, &result.data))
        .await
        .map_err(|e| format!("写入文件失败: {}", e))?;
    Ok(())
}

/// 删除歌曲
#[tauri::command]
pub async fn delete_song(
    state: tauri::State<'_, DeviceState>,
    file_no: u32,
    mem_unit: u8,
) -> Result<(), String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    device
        .delete_file(mem_unit, file_no)
        .await
        .map_err(err_string)?;
    drop(guard);
    state.invalidate_songs_cache(mem_unit).await;
    Ok(())
}

/// 创建歌单
#[tauri::command]
pub async fn create_playlist(
    state: tauri::State<'_, DeviceState>,
    name: String,
    mem_unit: u8,
) -> Result<u32, String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    let result = cyrio_core::api::playlist::create_playlist(device, &name, mem_unit)
        .await
        .map_err(err_string)?;
    Ok(result.file_no)
}

/// 添加歌曲到歌单（支持跨存储引用）
///
/// `song_mem_unit` 与 `playlist_mem_unit` 不同时，协议层会自动写入
/// `sflags[2]=1` 标记，使歌单可以引用另一个内存单元的歌曲。
#[tauri::command]
pub async fn add_song_to_playlist(
    state: tauri::State<'_, DeviceState>,
    song_file_no: u32,
    song_mem_unit: u8,
    playlist_file_no: u32,
    playlist_mem_unit: u8,
) -> Result<(), String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    cyrio_core::api::playlist::add_to_playlist(
        device,
        song_file_no,
        song_mem_unit,
        playlist_file_no,
        playlist_mem_unit,
    )
    .await
    .map_err(err_string)?;
    Ok(())
}

/// 修复歌单编码（name/title + bits）
///
/// 用于修复旧版本软件创建的歌单：
/// - bit 0=1 导致设备屏幕双重编码乱码
/// - name/title 字段被双重编码字节污染
#[tauri::command]
pub async fn repair_playlist_encoding(
    state: tauri::State<'_, DeviceState>,
    playlist_file_no: u32,
    playlist_mem_unit: u8,
) -> Result<(), String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    cyrio_core::api::playlist::repair_playlist_encoding(
        device,
        playlist_file_no,
        playlist_mem_unit,
    )
    .await
    .map_err(err_string)?;
    Ok(())
}

/// 获取歌曲详细信息（用于"详细信息"弹窗）
///
/// 下载歌曲到内存，解析 MP3 技术参数（duration/sample_rate/bit_rate/layer/channels），
/// 从 rio_file_t header 取 title/artist/album，返回 SongDetail。
///
/// 注意：设备存储的 data 是纯音频字节（上传时已剥离 ID3v2），
/// 因此 year/genre/track/composer/cover_art 无法从设备获取，返回空。
#[tauri::command]
pub async fn get_song_detail(
    state: tauri::State<'_, DeviceState>,
    file_no: u32,
    mem_unit: u8,
) -> Result<crate::SongDetail, String> {
    // 1. 下载文件到内存（释放 device lock 后再解析）
    let result = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        device
            .download_file(mem_unit, file_no, |_| {})
            .await
            .map_err(err_string)?
    };
    // result.header 是 RioFile，result.data 是纯音频字节
    // 2. 解析 MP3 技术参数（data 是纯音频，无 ID3v2 头）
    let technical = cyrio_audio::parse_mp3_info(&result.data).map(|info| crate::SongTechnical {
        duration: info.duration,
        sample_rate: info.sample_rate,
        bit_rate: info.bit_rate,
        layer: info.layer,
        channels: info.channels,
    });
    // 3. ID3 标签（设备 rio_file_t 的 title/artist/album 是 latin1 编码上传时写入的）
    let id3 = crate::SongId3 {
        title: result.header.title.clone(),
        artist: result.header.artist.clone(),
        album: result.header.album.clone(),
        year: String::new(),
        genre: String::new(),
        track: String::new(),
        composer: String::new(),
    };
    // 4. basic info
    let basic = SongInfo {
        file_no: result.header.file_no,
        size: result.header.size,
        time: result.header.time,
        name: result.header.name.clone(),
        title: result.header.title.clone(),
        artist: result.header.artist.clone(),
        album: result.header.album.clone(),
        bit_rate: result.header.bit_rate,
        mem_unit,
    };
    Ok(crate::SongDetail {
        basic,
        technical,
        id3,
        cover_art: None,
        mod_date: result.header.mod_date,
    })
}

// ============================================================================
// Phase 2: 重命名 / 批量文本处理 / 编码修复（对齐 egui message.rs Command）
// ============================================================================

/// 重命名操作输入项（前端传入）
///
/// Tauri 命令参数不支持 tuple，所以用 struct 表达 `(file_no, mem_unit, title)`。
/// `rename_all = "camelCase"`：前端传 `fileNo` / `memUnit`（与 Tauri 命令参数命名一致）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameItemInput {
    /// 文件号
    pub file_no: u32,
    /// 所在内存单元（0=内置, 1=SD）
    pub mem_unit: u8,
    /// 当前标题（用于批量操作时跳过无变化项）
    pub title: String,
}

/// 重命名单个歌曲 title
///
/// 流程：download → 修改 title → serialize（清 bit 0）→ overwrite（重传数据）。
/// 大文件重传较慢，进度通过 `download-progress` / `upload-progress` 事件反馈。
#[tauri::command]
pub async fn rename_song(
    state: tauri::State<'_, DeviceState>,
    file_no: u32,
    mem_unit: u8,
    new_title: String,
) -> Result<(), String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    cyrio_core::api::rename::rename_song_title(device, mem_unit, file_no, &new_title)
        .await
        .map_err(err_string)?;
    state.invalidate_songs_cache(mem_unit).await;
    Ok(())
}

/// 批量转拼音（指定列表）
///
/// 对给定的 `items` 逐个 rename，无中文的标题跳过（不调用 USB）。
/// 通过 `rename-progress` 事件发送进度（current/total/current_title）。
#[tauri::command]
pub async fn batch_slug_songs(
    app: AppHandle,
    state: tauri::State<'_, DeviceState>,
    items: Vec<RenameItemInput>,
) -> Result<Vec<cyrio_core::api::rename::RenameResult>, String> {
    let inner_items: Vec<(u32, u8, String)> = items
        .into_iter()
        .map(|i| (i.file_no, i.mem_unit, i.title))
        .collect();
    let results = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        cyrio_core::api::rename::batch_to_slug(device, inner_items, |cur, total, title| {
            let _ = app.emit("rename-progress", RenameProgressPayload {
                current: cur,
                total,
                current_title: title.to_string(),
                phase: "转拼音",
            });
        })
        .await
    };
    // 失效涉及的 mem_unit 缓存
    for mu in [0u8, 1u8] {
        state.invalidate_songs_cache(mu).await;
    }
    Ok(results)
}

/// 批量去词（指定列表）
///
/// 对给定的 `items` 逐个 rename，应用 strip_noise 去除无关词汇。无需去词的跳过。
/// 通过 `rename-progress` 事件发送进度（current/total/current_title）。
#[tauri::command]
pub async fn batch_strip_songs(
    app: AppHandle,
    state: tauri::State<'_, DeviceState>,
    items: Vec<RenameItemInput>,
    custom_words: Vec<String>,
) -> Result<Vec<cyrio_core::api::rename::RenameResult>, String> {
    let inner_items: Vec<(u32, u8, String)> = items
        .into_iter()
        .map(|i| (i.file_no, i.mem_unit, i.title))
        .collect();
    let results = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        cyrio_core::api::rename::batch_strip_noise(device, inner_items, custom_words, |cur, total, title| {
            let _ = app.emit("rename-progress", RenameProgressPayload {
                current: cur,
                total,
                current_title: title.to_string(),
                phase: "去词",
            });
        })
        .await
    };
    for mu in [0u8, 1u8] {
        state.invalidate_songs_cache(mu).await;
    }
    Ok(results)
}

/// 重命名进度事件 payload
///
/// 通过 `app.emit("rename-progress", payload)` 发送到前端。
/// 前端用 `listen` 监听并显示当前处理进度（第几个/共几个/当前标题）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RenameProgressPayload {
    /// 当前处理到的文件索引（1-based）
    pub current: usize,
    /// 总文件数
    pub total: usize,
    /// 当前文件的标题（或"（跳过）"）
    pub current_title: String,
    /// 操作阶段描述（"转拼音" / "去词"）
    pub phase: &'static str,
}

/// 预览转拼音（不执行实际改名，只计算新标题）
///
/// 纯文本操作，不需要设备连接。用于操作前让用户确认改名效果。
#[tauri::command]
pub async fn preview_slug(
    items: Vec<RenameItemInput>,
) -> Result<Vec<cyrio_core::api::rename::PreviewResult>, String> {
    let inner_items: Vec<(u32, u8, String)> = items
        .into_iter()
        .map(|i| (i.file_no, i.mem_unit, i.title))
        .collect();
    Ok(cyrio_core::api::rename::preview_slug(&inner_items))
}

/// 预览去词（不执行实际改名，只计算新标题）
///
/// 纯文本操作，不需要设备连接。用于操作前让用户确认改名效果。
#[tauri::command]
pub async fn preview_strip(
    items: Vec<RenameItemInput>,
    custom_words: Vec<String>,
) -> Result<Vec<cyrio_core::api::rename::PreviewResult>, String> {
    let inner_items: Vec<(u32, u8, String)> = items
        .into_iter()
        .map(|i| (i.file_no, i.mem_unit, i.title))
        .collect();
    Ok(cyrio_core::api::rename::preview_strip(&inner_items, custom_words))
}

/// 修复单个歌曲编码（重新序列化 header，强制 bit 0=0）
///
/// 用于修复 Phase A 之前上传的歌曲（bit 0=1 导致中文乱码）。
#[tauri::command]
pub async fn repair_song_encoding(
    state: tauri::State<'_, DeviceState>,
    file_no: u32,
    mem_unit: u8,
) -> Result<(), String> {
    let guard = state.device.lock().await;
    let device = guard.as_ref().ok_or("设备未连接")?;
    cyrio_core::api::rename::repair_song_encoding(device, mem_unit, file_no)
        .await
        .map_err(err_string)?;
    state.invalidate_songs_cache(mem_unit).await;
    Ok(())
}

/// 内部辅助：列出指定 mem_unit 的所有 MP3 文件（不走 songs_cache，避免污染）
async fn list_all_mp3_files(
    device: &RioDevice,
    mem_unit: u8,
) -> Result<Vec<(u32, u8, String)>, String> {
    let files = device.list_files(mem_unit, |_| {}).await.map_err(err_string)?;
    Ok(files
        .iter()
        .filter(|f| f.file_type == cyrio_core::protocol::constants::TYPE_MP3)
        .map(|f| {
            // 标题兜底：title 为空时用 name 去路径前缀和扩展名
            let title = if !f.title.is_empty() {
                f.title.clone()
            } else {
                let base = f.name.rsplit(['\\', '/']).next().unwrap_or(&f.name);
                let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
                stem.to_string()
            };
            (f.file_no, mem_unit, title)
        })
        .collect())
}

/// 预览：筛选出所有编码错误的歌曲（bit 0=1）
///
/// 扫描两个 mem_unit 的所有 MP3 文件，返回 `bits & 0x01 == 1` 的歌曲列表。
/// 这些歌曲在设备屏幕上会显示中文乱码（固件双重编码），需要修复。
/// 纯读取操作（list_files），不修改设备数据。用于修复前让用户确认范围。
#[tauri::command]
pub async fn preview_repair_encoding(
    state: tauri::State<'_, DeviceState>,
) -> Result<Vec<cyrio_core::api::rename::PreviewResult>, String> {
    let mut all_previews = Vec::new();
    for mu in [0u8, 1u8] {
        let files: Vec<cyrio_core::protocol::rio_file::RioFile> = {
            let guard = state.device.lock().await;
            let device = match guard.as_ref() {
                Some(d) => d,
                None => return Err("设备未连接".to_string()),
            };
            match device.list_files(mu, |_| {}).await {
                Ok(v) => v,
                Err(e) => {
                    // SD 未插入等错误：跳过该 mem_unit
                    log::warn!("preview_repair_encoding: list mem_unit {} failed: {}", mu, e);
                    continue;
                }
            }
        };
        for f in &files {
            if f.file_type != cyrio_core::protocol::constants::TYPE_MP3 {
                continue;
            }
            if f.bits & 0x01 != 1 {
                continue;
            }
            // title 兜底：title 为空时用 name 去路径前缀和扩展名
            let title = if !f.title.is_empty() {
                f.title.clone()
            } else {
                let base = f.name.rsplit(['\\', '/']).next().unwrap_or(&f.name);
                let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
                stem.to_string()
            };
            all_previews.push(cyrio_core::api::rename::PreviewResult {
                file_no: f.file_no,
                mem_unit: mu,
                original: title.clone(),
                // 修复后 title 内容不变（read_fixed_string 已恢复正确 UTF-8），
                // 变化的是 bits 字段（清 bit 0），设备屏幕显示从乱码→正常
                new_title: title,
                changed: true,
            });
        }
    }
    log::info!(
        "preview_repair_encoding: found {} songs needing repair",
        all_previews.len()
    );
    Ok(all_previews)
}

/// 修复所有歌曲编码（内置 + SD 双存储）
///
/// **智能检测**：只修复 `bits & 0x01 == 1` 的歌曲（bit 0=1 会导致设备屏幕双重编码乱码）。
/// bit 0=0 的歌曲（原版软件上传的）跳过，避免无意义的 download/upload。
///
/// 逐个 download → overwrite + 验证 + delete+upload fallback（repair_song_encoding 内部处理）。
/// 通过 `rename-progress` 事件发送进度（current/total/current_title，phase="修复编码"）。
/// 返回每个文件的结果（包括列出失败的 mem_unit 占位项）。
#[tauri::command]
pub async fn repair_all_songs_encoding(
    app: AppHandle,
    state: tauri::State<'_, DeviceState>,
) -> Result<Vec<cyrio_core::api::rename::RenameResult>, String> {
    let mut all_results = Vec::new();

    // Phase 1：列出两个 mem_unit 的所有 MP3 文件，过滤出需修复的（bit 0=1）
    let mut all_to_repair: Vec<(u32, u8, String)> = Vec::new();
    for mu in [0u8, 1u8] {
        let files: Vec<cyrio_core::protocol::rio_file::RioFile> = {
            let guard = state.device.lock().await;
            let device = match guard.as_ref() {
                Some(d) => d,
                None => return Err("设备未连接".to_string()),
            };
            match device.list_files(mu, |_| {}).await {
                Ok(v) => v,
                Err(e) => {
                    // 某个 mem_unit 失败（如 SD 未插入）不中断整体流程
                    all_results.push(cyrio_core::api::rename::RenameResult {
                        file_no: 0,
                        mem_unit: mu,
                        success: false,
                        original: String::new(),
                        new_title: String::new(),
                        error: format!("列出 mem_unit {} 失败: {}", mu, e),
                    });
                    continue;
                }
            }
        };

        let mut skipped_count = 0u32;
        for f in &files {
            if f.file_type != cyrio_core::protocol::constants::TYPE_MP3 {
                continue;
            }
            let title = if !f.title.is_empty() {
                f.title.clone()
            } else {
                let base = f.name.rsplit(['\\', '/']).next().unwrap_or(&f.name);
                let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
                stem.to_string()
            };
            if f.bits & 0x01 == 1 {
                log::info!(
                    "repair_all_songs_encoding: mem_unit={} file_no={} title={:?} bits=0x{:x} -> needs repair (bit0=1)",
                    mu, f.file_no, title, f.bits
                );
                all_to_repair.push((f.file_no, mu, title));
            } else {
                skipped_count += 1;
            }
        }
        log::info!(
            "repair_all_songs_encoding: mem_unit={} total={} to_repair_in_this_mu skipped={}",
            mu,
            files.iter().filter(|f| f.file_type == cyrio_core::protocol::constants::TYPE_MP3).count(),
            skipped_count
        );
    }

    // Phase 2：逐个修复，发送进度事件（跨两个 mem_unit 累计 current/total）
    let total = all_to_repair.len();
    log::info!(
        "repair_all_songs_encoding: total to repair across both mem_units = {}",
        total
    );
    {
        let guard = state.device.lock().await;
        let device = match guard.as_ref() {
            Some(d) => d,
            None => return Err("设备未连接".to_string()),
        };
        for (i, (file_no, mem_unit, title)) in all_to_repair.iter().enumerate() {
            // 发送进度（1-based current）
            let _ = app.emit("rename-progress", RenameProgressPayload {
                current: i + 1,
                total,
                current_title: title.clone(),
                phase: "修复编码",
            });
            match cyrio_core::api::rename::repair_song_encoding(device, *mem_unit, *file_no).await {
                Ok(()) => all_results.push(cyrio_core::api::rename::RenameResult {
                    file_no: *file_no,
                    mem_unit: *mem_unit,
                    success: true,
                    original: title.clone(),
                    new_title: title.clone(),
                    error: String::new(),
                }),
                Err(e) => all_results.push(cyrio_core::api::rename::RenameResult {
                    file_no: *file_no,
                    mem_unit: *mem_unit,
                    success: false,
                    original: title.clone(),
                    new_title: title.clone(),
                    error: e.to_string(),
                }),
            }
        }
    }

    // Phase 3：失效两个 mem_unit 的 songs 缓存
    for mu in [0u8, 1u8] {
        state.invalidate_songs_cache(mu).await;
    }
    Ok(all_results)
}

/// 修复选中歌曲的编码（指定列表）
///
/// 对给定的 `items` 逐个执行 `repair_song_encoding`（download → overwrite + 验证 + fallback）。
/// 通过 `rename-progress` 事件发送进度（current/total/current_title，phase="修复编码"）。
///
/// 与 `repair_all_songs_encoding` 区别：本命令只处理用户选中的歌曲，不需要扫描全部存储。
#[tauri::command]
pub async fn repair_selected_encoding(
    app: AppHandle,
    state: tauri::State<'_, DeviceState>,
    items: Vec<RenameItemInput>,
) -> Result<Vec<cyrio_core::api::rename::RenameResult>, String> {
    let inner_items: Vec<(u32, u8, String)> = items
        .into_iter()
        .map(|i| (i.file_no, i.mem_unit, i.title))
        .collect();
    let total = inner_items.len();
    let mut all_results = Vec::with_capacity(total);
    log::info!(
        "repair_selected_encoding: start, {} items",
        total
    );
    {
        let guard = state.device.lock().await;
        let device = match guard.as_ref() {
            Some(d) => d,
            None => return Err("设备未连接".to_string()),
        };
        for (i, (file_no, mem_unit, title)) in inner_items.iter().enumerate() {
            // 发送进度（1-based current）
            let _ = app.emit("rename-progress", RenameProgressPayload {
                current: i + 1,
                total,
                current_title: title.clone(),
                phase: "修复编码",
            });
            match cyrio_core::api::rename::repair_song_encoding(device, *mem_unit, *file_no).await {
                Ok(()) => all_results.push(cyrio_core::api::rename::RenameResult {
                    file_no: *file_no,
                    mem_unit: *mem_unit,
                    success: true,
                    original: title.clone(),
                    new_title: title.clone(),
                    error: String::new(),
                }),
                Err(e) => all_results.push(cyrio_core::api::rename::RenameResult {
                    file_no: *file_no,
                    mem_unit: *mem_unit,
                    success: false,
                    original: title.clone(),
                    new_title: title.clone(),
                    error: e.to_string(),
                }),
            }
        }
    }
    // 失效涉及的 mem_unit 缓存
    for mu in [0u8, 1u8] {
        state.invalidate_songs_cache(mu).await;
    }
    Ok(all_results)
}

/// 批量为所有歌曲转拼音（内置 + SD 双存储）
#[tauri::command]
pub async fn batch_slug_all_songs(
    app: AppHandle,
    state: tauri::State<'_, DeviceState>,
) -> Result<Vec<cyrio_core::api::rename::RenameResult>, String> {
    let mut all_items: Vec<(u32, u8, String)> = Vec::new();
    for mu in [0u8, 1u8] {
        let guard = state.device.lock().await;
        let device = match guard.as_ref() {
            Some(d) => d,
            None => return Err("设备未连接".to_string()),
        };
        match list_all_mp3_files(device, mu).await {
            Ok(v) => all_items.extend(v),
            Err(e) => {
                // SD 未插入等错误：跳过该 mem_unit
                log::warn!("列出 mem_unit {} 失败: {}", mu, e);
            }
        }
    }
    let results = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        cyrio_core::api::rename::batch_to_slug(device, all_items, |cur, total, title| {
            let _ = app.emit("rename-progress", RenameProgressPayload {
                current: cur,
                total,
                current_title: title.to_string(),
                phase: "转拼音",
            });
        })
        .await
    };
    for mu in [0u8, 1u8] {
        state.invalidate_songs_cache(mu).await;
    }
    Ok(results)
}

/// 批量为所有歌曲去词（内置 + SD 双存储）
#[tauri::command]
pub async fn batch_strip_all_songs(
    app: AppHandle,
    state: tauri::State<'_, DeviceState>,
    custom_words: Vec<String>,
) -> Result<Vec<cyrio_core::api::rename::RenameResult>, String> {
    let mut all_items: Vec<(u32, u8, String)> = Vec::new();
    for mu in [0u8, 1u8] {
        let guard = state.device.lock().await;
        let device = match guard.as_ref() {
            Some(d) => d,
            None => return Err("设备未连接".to_string()),
        };
        match list_all_mp3_files(device, mu).await {
            Ok(v) => all_items.extend(v),
            Err(e) => {
                log::warn!("列出 mem_unit {} 失败: {}", mu, e);
            }
        }
    }
    let results = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        cyrio_core::api::rename::batch_strip_noise(device, all_items, custom_words, |cur, total, title| {
            let _ = app.emit("rename-progress", RenameProgressPayload {
                current: cur,
                total,
                current_title: title.to_string(),
                phase: "去词",
            });
        })
        .await
    };
    for mu in [0u8, 1u8] {
        state.invalidate_songs_cache(mu).await;
    }
    Ok(results)
}

// ============================================================================
// 系统托盘
// ============================================================================

/// 更新系统托盘 tooltip（显示连接状态和传输进度）
///
/// 前端在连接/断开设备、上传进度变化时调用此命令更新托盘提示。
/// tooltip 格式：
/// - 未连接：`cyrio - 未连接设备`
/// - 已连接：`cyrio - 已连接 Rio S-Series`
/// - 传输中：`cyrio - 正在传输 (3/10)`
#[tauri::command]
pub async fn update_tray_tooltip(
    app: AppHandle,
    connected: bool,
    transferring: Option<(u32, u32)>,
) -> Result<(), String> {
    let tooltip = match (connected, transferring) {
        (_, Some((done, total))) if total > 0 => {
            format!("cyrio - 正在传输 ({}/{})", done, total)
        }
        (true, _) => "cyrio - 已连接 Rio S-Series".to_string(),
        (false, _) => "cyrio - 未连接设备".to_string(),
    };
    // 系统托盘仅在桌面端可用
    #[cfg(desktop)]
    {
        use tauri::Manager;
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_tooltip(Some(&tooltip));
        }
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, tooltip);
    }
    Ok(())
}
