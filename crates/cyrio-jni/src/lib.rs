//! # cyrio-jni
//!
//! JNI 桥接层：让 Java/JavaFX 调用 Rust 实现的 cyrio-core。
//!
//! ## 架构
//! ```text
//! Java (UI层)  →  JNI  →  cyrio-jni (本crate)  →  cyrio-core + cyrio-text + cyrio-transport-nusb
//! ```
//!
//! ## 设计
//! - 设备句柄以 `jlong` 传递（`Box<RioDevice>` 的裸指针）
//! - 复杂数据用 JSON 字符串交换（Java 端用 Jackson 解析）
//! - 异步操作用 `smol::block_on` 同步执行
//! - 所有 JNI 函数遵循 `Java_c_cyrio_android_jni_CyrioNative_<method>` 命名

#![warn(clippy::all)]
#![allow(clippy::missing_safety_doc)]

use std::ptr;
use std::sync::Once;

use cyrio_core::api::device::RioDevice;
use cyrio_core::api::types::{rio_file_to_song, rio_file_to_playlist, Song, Playlist};
use cyrio_core::api::upload::{process_title, UploadTextOptions};
use cyrio_core::api::rename::{rename_song_title, repair_song_encoding};
use cyrio_core::api::playlist;
use cyrio_core::protocol::rio_mem::RioMem;
use cyrio_transport_nusb::{NusbTransport, UsbDeviceInfo};
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use serde::Serialize;

// ============================================================================
// smol 全局执行器（cyrio-core 的 Timer 需要）
// ============================================================================

static SMOL_EXECUTOR_INIT: Once = Once::new();

/// 启动 smol 全局执行器（后台线程）
///
/// cyrio-core 的 `smol::Timer` 需要 smol 全局执行器驱动。
/// JNI 端在首次打开设备时自动启动。
fn start_smol_executor() {
    SMOL_EXECUTOR_INIT.call_once(|| {
        std::thread::Builder::new()
            .name("smol-executor".into())
            .spawn(|| {
                smol::block_on(smol::future::pending::<()>());
            })
            .expect("spawn smol executor thread");
    });
}

// ============================================================================
// JSON 序列化辅助类型
// ============================================================================

#[derive(Serialize)]
struct SongJson {
    #[serde(rename = "fileNo")]
    file_no: u32,
    size: u32,
    time: u32,
    #[serde(rename = "bitRate")]
    bit_rate: u32,
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
    name: String,
    title: String,
    artist: String,
    album: String,
    #[serde(rename = "memUnit")]
    mem_unit: u8,
}

impl From<&Song> for SongJson {
    fn from(s: &Song) -> Self {
        Self {
            file_no: s.file_no,
            size: s.size,
            time: s.time,
            bit_rate: s.bit_rate,
            sample_rate: s.sample_rate,
            name: s.name.clone(),
            title: s.title.clone(),
            artist: s.artist.clone(),
            album: s.album.clone(),
            mem_unit: 0,
        }
    }
}

#[derive(Serialize)]
struct PlaylistJson {
    #[serde(rename = "fileNo")]
    file_no: u32,
    size: u32,
    name: String,
    title: String,
}

impl From<&Playlist> for PlaylistJson {
    fn from(p: &Playlist) -> Self {
        Self {
            file_no: p.file_no,
            size: p.size,
            name: p.name.clone(),
            title: p.title.clone(),
        }
    }
}

#[derive(Serialize)]
struct StorageJson {
    #[serde(rename = "totalSize")]
    total_size: u32,
    #[serde(rename = "usedSize")]
    used_size: u32,
    #[serde(rename = "freeSize")]
    free_size: u32,
    #[serde(rename = "systemSize")]
    system_size: u32,
    name: String,
    model: String,
    #[serde(rename = "isPresent")]
    is_present: bool,
}

impl From<&RioMem> for StorageJson {
    fn from(m: &RioMem) -> Self {
        Self {
            total_size: m.size,
            used_size: m.used,
            free_size: m.free,
            system_size: m.system,
            name: m.name.clone(),
            model: m.model.clone(),
            is_present: m.is_present(),
        }
    }
}

#[derive(Serialize)]
struct UsbDeviceJson {
    vid: u16,
    pid: u16,
    name: String,
    manufacturer: String,
    serial: String,
}

impl From<&UsbDeviceInfo> for UsbDeviceJson {
    fn from(d: &UsbDeviceInfo) -> Self {
        Self {
            vid: d.vid,
            pid: d.pid,
            name: d.name.clone(),
            manufacturer: d.manufacturer.clone(),
            serial: d.serial.clone(),
        }
    }
}

// ============================================================================
// JNI 辅助函数
// ============================================================================

/// 将 Rust String 转为 JNI jstring
fn rust_str_to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s).map(|j| j.into_raw()).unwrap_or(ptr::null_mut())
}

/// 将 JNI jstring 转为 Rust String
fn jstring_to_rust(env: &mut JNIEnv, s: JString) -> String {
    env.get_string(&s).map(|j| j.to_str().unwrap_or("").to_string()).unwrap_or_default()
}

/// 将错误信息转为 jstring（返回 null 表示成功，非 null 表示错误消息）
fn error_to_jstring(env: &mut JNIEnv, e: &dyn std::fmt::Display) -> jstring {
    rust_str_to_jstring(env, &format!("{}", e))
}

/// 从 jlong 句柄获取 RioDevice 引用
///
/// # Safety
/// handle 必须是之前 `open_device` 返回的有效指针
unsafe fn device_from_handle(handle: jlong) -> &'static mut RioDevice {
    &mut *(handle as *mut RioDevice)
}

/// 运行异步 future（smol block_on）
fn run_async<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    smol::block_on(f)
}

// ============================================================================
// 设备管理
// ============================================================================

/// 打开 Rio S-Series 设备
///
/// 自动扫描 VID=0x045a 的设备，完成 USB 协议握手。
///
/// @return 设备句柄 (jlong)，0 表示失败
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_openDevice(
    mut env: JNIEnv,
    _class: JClass,
) -> jlong {
    let result = run_async(async {
        let transport = NusbTransport::open().await?;
        let mut device = RioDevice::new(Box::new(transport));
        device.open().await?;
        // 启动 smol 执行器（cyrio-core 的 Timer 需要）
        Ok::<_, cyrio_core::error::CyrioError>(device) as Result<_, cyrio_core::error::CyrioError>
    });

    match result {
        Ok(device) => {
            // 启动后台 smol 执行器线程
            start_smol_executor();
            let boxed = Box::new(device);
            Box::into_raw(boxed) as jlong
        }
        Err(e) => {
            log::error!("openDevice failed: {}", e);
            0
        }
    }
}

/// 以指定 VID/PID 强制打开设备
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_openDeviceWithVidPid(
    mut env: JNIEnv,
    _class: JClass,
    vid: jint,
    pid: jint,
) -> jlong {
    let result = run_async(async {
        let transport = NusbTransport::open_with_vid_pid(vid as u16, pid as u16).await?;
        let mut device = RioDevice::new(Box::new(transport));
        device.open().await?;
        Ok::<_, cyrio_core::error::CyrioError>(device)
    });

    match result {
        Ok(device) => {
            start_smol_executor();
            Box::into_raw(Box::new(device)) as jlong
        }
        Err(e) => {
            log::error!("openDeviceWithVidPid failed: {}", e);
            0
        }
    }
}

/// 关闭设备并释放资源
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_closeDevice(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            let _ = Box::from_raw(handle as *mut RioDevice);
        }
    }
}

/// 检查设备是否已连接
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_isConnected(
    _env: JNIEnv,
    _class: JClass,
    _handle: jlong,
) -> jboolean {
    // 设备句柄存在即视为已连接
    if _handle != 0 {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

// ============================================================================
// 设备操作
// ============================================================================

/// 列出内存单元中的所有歌曲
///
/// @param handle 设备句柄
/// @param memUnit 内存单元 (0=内置, 1=SD卡)
/// @return JSON 字符串: [{fileNo, size, time, bitRate, sampleRate, name, title, artist, album, memUnit}]
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_listSongs(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mem_unit: jint,
) -> jstring {
    if handle == 0 {
        return rust_str_to_jstring(&mut env, "[]");
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        let files = device.list_files(mem_unit as u8, |_| {}).await?;
        let songs: Vec<SongJson> = files
            .iter()
            .filter(|f| f.file_type == cyrio_core::protocol::constants::TYPE_MP3)
            .map(|f| {
                let mut s: SongJson = (&rio_file_to_song(f)).into();
                s.mem_unit = mem_unit as u8;
                s
            })
            .collect();
        serde_json::to_string(&songs).map_err(|e| {
            cyrio_core::error::CyrioError::Other(format!("JSON serialize: {}", e))
        })
    });

    match result {
        Ok(json) => rust_str_to_jstring(&mut env, &json),
        Err(e) => {
            log::error!("listSongs failed: {}", e);
            rust_str_to_jstring(&mut env, "[]")
        }
    }
}

/// 获取存储信息
///
/// @return JSON: {totalSize, usedSize, freeSize, systemSize, name, model, isPresent}
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_getStorage(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mem_unit: jint,
) -> jstring {
    if handle == 0 {
        return rust_str_to_jstring(&mut env, "{}");
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        let mem = device.get_memory_info(mem_unit as u8).await?;
        let json: StorageJson = (&mem).into();
        serde_json::to_string(&json).map_err(|e| {
            cyrio_core::error::CyrioError::Other(format!("JSON serialize: {}", e))
        })
    });

    match result {
        Ok(json) => rust_str_to_jstring(&mut env, &json),
        Err(e) => {
            log::error!("getStorage failed: {}", e);
            rust_str_to_jstring(&mut env, "{}")
        }
    }
}

/// 上传 MP3 文件到设备
///
/// @param handle 设备句柄
/// @param memUnit 目标内存单元
/// @param filePath 本地文件路径
/// @param applySlug 是否应用拼音转换
/// @param applyStrip 是否应用去词
/// @return 文件号 (>0 成功, -1 失败)
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_uploadFile(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mem_unit: jint,
    file_path: JString,
    apply_slug: jboolean,
    apply_strip: jboolean,
) -> jint {
    if handle == 0 {
        return -1;
    }

    let path = jstring_to_rust(&mut env, file_path);
    let device = unsafe { device_from_handle(handle) };
    let text_opts = UploadTextOptions {
        apply_slug: apply_slug == JNI_TRUE,
        apply_strip: apply_strip == JNI_TRUE,
        ..Default::default()
    };

    let result = run_async(async {
        cyrio_core::api::upload::upload_mp3(
            device,
            mem_unit as u8,
            std::path::Path::new(&path),
            &text_opts,
            |_| {}, // 无进度回调
        )
        .await
    });

    match result {
        Ok(file_no) => file_no as jint,
        Err(e) => {
            log::error!("uploadFile error: {}", e);
            -1
        }
    }
}

/// 下载文件到本地路径
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_downloadFile(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mem_unit: jint,
    file_no: jint,
    output_path: JString,
) -> jboolean {
    if handle == 0 {
        return JNI_FALSE;
    }

    let path = jstring_to_rust(&mut env, output_path);
    let device = unsafe { device_from_handle(handle) };

    let result = run_async(async {
        let dl = device.download_file(mem_unit as u8, file_no as u32, |_| {}).await?;
        std::fs::write(&path, &dl.data).map_err(|e| {
            cyrio_core::error::CyrioError::Other(format!("write file: {}", e))
        })
    });

    match result {
        Ok(()) => JNI_TRUE,
        Err(e) => {
            log::error!("downloadFile failed: {}", e);
            JNI_FALSE
        }
    }
}

/// 删除设备上的文件
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_deleteFile(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mem_unit: jint,
    file_no: jint,
) -> jboolean {
    if handle == 0 {
        return JNI_FALSE;
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        device.delete_file(mem_unit as u8, file_no as u32).await
    });

    match result {
        Ok(()) => JNI_TRUE,
        Err(e) => {
            log::error!("deleteFile failed: {}", e);
            JNI_FALSE
        }
    }
}

// ============================================================================
// 歌单操作
// ============================================================================

/// 列出所有歌单
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_listPlaylists(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    mem_unit: jint,
) -> jstring {
    if handle == 0 {
        return rust_str_to_jstring(&mut env, "[]");
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        let files = device.list_files(mem_unit as u8, |_| {}).await?;
        let playlists: Vec<PlaylistJson> = files
            .iter()
            .filter(|f| f.file_type == cyrio_core::protocol::constants::TYPE_PLS)
            .map(|f| (&rio_file_to_playlist(f)).into())
            .collect();
        serde_json::to_string(&playlists).map_err(|e| {
            cyrio_core::error::CyrioError::Other(format!("JSON serialize: {}", e))
        })
    });

    match result {
        Ok(json) => rust_str_to_jstring(&mut env, &json),
        Err(e) => {
            log::error!("listPlaylists failed: {}", e);
            rust_str_to_jstring(&mut env, "[]")
        }
    }
}

/// 创建新歌单
///
/// @return 文件号 (>0 成功, -1 失败)
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_createPlaylist(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    name: JString,
    mem_unit: jint,
) -> jint {
    if handle == 0 {
        return -1;
    }

    let name = jstring_to_rust(&mut env, name);
    let device = unsafe { device_from_handle(handle) };

    let result = run_async(async {
        playlist::create_playlist(device, &name, mem_unit as u8).await
    });

    match result {
        Ok(created) => created.file_no as jint,
        Err(e) => {
            log::error!("createPlaylist failed: {}", e);
            -1
        }
    }
}

/// 添加歌曲到歌单
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_addToPlaylist(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    song_file_no: jint,
    song_mem_unit: jint,
    playlist_file_no: jint,
    playlist_mem_unit: jint,
) -> jboolean {
    if handle == 0 {
        return JNI_FALSE;
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        playlist::add_to_playlist(
            device,
            song_file_no as u32,
            song_mem_unit as u8,
            playlist_file_no as u32,
            playlist_mem_unit as u8,
        )
        .await
    });

    match result {
        Ok(()) => JNI_TRUE,
        Err(e) => {
            log::error!("addToPlaylist failed: {}", e);
            JNI_FALSE
        }
    }
}

/// 列出歌单内的歌曲
///
/// @return JSON: [{fileNo, size, time, bitRate, sampleRate, name, title, artist, album, memUnit, index}]
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_listPlaylistSongs(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    playlist_file_no: jint,
    mem_unit: jint,
) -> jstring {
    if handle == 0 {
        return rust_str_to_jstring(&mut env, "[]");
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        let songs = playlist::list_playlist_songs(
            device,
            playlist_file_no as u32,
            mem_unit as u8,
        )
        .await?;

        #[derive(Serialize)]
        struct PlaylistSongJson {
            #[serde(rename = "fileNo")]
            file_no: u32,
            size: u32,
            time: u32,
            #[serde(rename = "bitRate")]
            bit_rate: u32,
            #[serde(rename = "sampleRate")]
            sample_rate: u32,
            name: String,
            title: String,
            artist: String,
            album: String,
            #[serde(rename = "memUnit")]
            mem_unit: u8,
            index: usize,
        }

        let json_songs: Vec<PlaylistSongJson> = songs
            .iter()
            .map(|ps| {
                let s = &ps.song;
                PlaylistSongJson {
                    file_no: s.file_no,
                    size: s.size,
                    time: s.time,
                    bit_rate: s.bit_rate,
                    sample_rate: s.sample_rate,
                    name: s.name.clone(),
                    title: s.title.clone(),
                    artist: s.artist.clone(),
                    album: s.album.clone(),
                    mem_unit: ps.mem_unit,
                    index: ps.index,
                }
            })
            .collect();

        serde_json::to_string(&json_songs).map_err(|e| {
            cyrio_core::error::CyrioError::Other(format!("JSON serialize: {}", e))
        })
    });

    match result {
        Ok(json) => rust_str_to_jstring(&mut env, &json),
        Err(e) => {
            log::error!("listPlaylistSongs failed: {}", e);
            rust_str_to_jstring(&mut env, "[]")
        }
    }
}

/// 从歌单中移除指定位置的歌曲
///
/// @param handle          设备句柄
/// @param playlistFileNo  歌单文件号
/// @param memUnit         歌单所在内存单元
/// @param index           条目索引 (0-based，来自 listPlaylistSongs 返回的 index)
/// @return true 成功
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_removeFromPlaylist(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    playlist_file_no: jint,
    mem_unit: jint,
    index: jint,
) -> jboolean {
    if handle == 0 {
        return JNI_FALSE;
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        playlist::remove_from_playlist(
            device,
            playlist_file_no as u32,
            mem_unit as u8,
            index as usize,
        )
        .await
    });

    match result {
        Ok(()) => JNI_TRUE,
        Err(e) => {
            log::error!("removeFromPlaylist failed: {}", e);
            JNI_FALSE
        }
    }
}

// ============================================================================
// 重命名 / 编码修复
// ============================================================================

/// 重命名歌曲（修改 name 和 title 字段）
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_renameSong(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    file_no: jint,
    mem_unit: jint,
    new_name: JString,
) -> jboolean {
    if handle == 0 {
        return JNI_FALSE;
    }

    let name = jstring_to_rust(&mut env, new_name);
    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        rename_song_title(device, mem_unit as u8, file_no as u32, &name).await
    });

    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            log::error!("renameSong failed: {}", e);
            JNI_FALSE
        }
    }
}

/// 修复歌曲编码（双重编码 → 正确 UTF-8）
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_repairEncoding(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    file_no: jint,
    mem_unit: jint,
) -> jboolean {
    if handle == 0 {
        return JNI_FALSE;
    }

    let device = unsafe { device_from_handle(handle) };
    let result = run_async(async {
        repair_song_encoding(device, mem_unit as u8, file_no as u32).await
    });

    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            log::error!("repairEncoding failed: {}", e);
            JNI_FALSE
        }
    }
}

// ============================================================================
// 文本处理
// ============================================================================

/// Slug 转换（中文→拼音，日文→罗马字）
///
/// @param text 输入文本
/// @param separator 分隔符 (如 "-")
/// @param capitalize 是否首字母大写
/// @param keepPunctuation 是否保留标点
/// @return 转换后的字符串
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_toSlug(
    mut env: JNIEnv,
    _class: JClass,
    text: JString,
    separator: JString,
    capitalize: jboolean,
    keep_punctuation: jboolean,
) -> jstring {
    let input = jstring_to_rust(&mut env, text);
    let sep = jstring_to_rust(&mut env, separator);
    let sep_char = sep.chars().next().unwrap_or('-');

    let opts = cyrio_text::SlugOptions {
        keep_punctuation: keep_punctuation == JNI_TRUE,
        separator: sep_char,
        capitalize: capitalize == JNI_TRUE,
    };

    let result = cyrio_text::to_slug(&input, &opts);
    rust_str_to_jstring(&mut env, &result)
}

/// 去除标题噪音词
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_stripNoise(
    mut env: JNIEnv,
    _class: JClass,
    text: JString,
) -> jstring {
    let input = jstring_to_rust(&mut env, text);
    let result = cyrio_text::strip_noise(&input, &cyrio_text::StripOptions::default());
    rust_str_to_jstring(&mut env, &result.cleaned)
}

/// 处理标题（先 strip 再 slug）
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_processTitle(
    mut env: JNIEnv,
    _class: JClass,
    title: JString,
    apply_slug: jboolean,
    apply_strip: jboolean,
) -> jstring {
    let input = jstring_to_rust(&mut env, title);
    let opts = UploadTextOptions {
        apply_slug: apply_slug == JNI_TRUE,
        apply_strip: apply_strip == JNI_TRUE,
        ..Default::default()
    };
    let result = process_title(&input, &opts);
    rust_str_to_jstring(&mut env, &result)
}

// ============================================================================
// USB 设备扫描
// ============================================================================

/// 列出系统中所有 USB 设备
///
/// @return JSON: [{vid, pid, name, manufacturer, serial}]
#[no_mangle]
pub extern "system" fn Java_c_cyrio_android_jni_CyrioNative_listUsbDevices(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let result = run_async(async {
        let devices = cyrio_transport_nusb::list_all_usb_devices().await?;
        let json_devices: Vec<UsbDeviceJson> = devices.iter().map(|d| d.into()).collect();
        serde_json::to_string(&json_devices).map_err(|e| {
            cyrio_core::error::CyrioError::Other(format!("JSON serialize: {}", e))
        })
    });

    match result {
        Ok(json) => rust_str_to_jstring(&mut env, &json),
        Err(e) => {
            log::error!("listUsbDevices failed: {}", e);
            rust_str_to_jstring(&mut env, "[]")
        }
    }
}

// ============================================================================
// JNI_OnLoad — 初始化日志
// ============================================================================

#[no_mangle]
pub extern "system" fn JNI_OnLoad(_vm: *mut jni::JavaVM, _reserved: *mut std::ffi::c_void) -> jint {
    // 初始化日志（环境变量控制级别）
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    )
    .try_init();

    log::info!("cyrio-jni loaded (version {}", env!("CARGO_PKG_VERSION"));

    // 返回 JNI 1.6（Android 16 拒绝 JNI_VERSION_1_8，使用最广泛支持的 1.6）
    jni::sys::JNI_VERSION_1_6
}
