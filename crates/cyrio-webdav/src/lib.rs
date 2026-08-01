//! # cyrio-webdav
//!
//! WebDAV 虚拟U盘服务器：把 Rio MP3 设备虚拟成 WebDAV 网络驱动器。
//!
//! 用户通过 Finder（Cmd+K）或 Windows 资源管理器（映射网络驱动器）挂载后，
//! 可像操作普通 U 盘一样管理设备上的歌曲和歌单。
//!
//! ## 虚拟目录结构
//! ```text
//! /
//! ├── 歌曲/
//! │   ├── 内置存储/        (mem_unit=0 的 MP3 文件)
//! │   ├── SD卡/            (mem_unit=1 的 MP3 文件)
//! │   └── _全部歌曲/       (两个 mem_unit 合并视图，只读)
//! └── 歌单/
//!     └── {playlist_name}/ (歌单内歌曲为虚拟文件)
//! ```
//!
//! ## 操作语义
//! - 往 `歌曲/{存储}/` 拖入 MP3 → 上传到对应存储
//! - 往 `歌单/{name}/` 拖入 MP3 → 先上传到内置存储 → 再加入该歌单
//! - 从 `歌曲/{存储}/` 删除 MP3 → 从设备删除歌曲文件
//! - 从 `歌单/{name}/` 删除 MP3 → 仅移除歌单引用，歌曲文件保留
//! - 在 `歌单/` 下创建新目录 → 创建新歌单
//! - `_全部歌曲/` 只读，禁止 PUT/DELETE/MKCOL
//!
//! ## 架构
//! - tiny_http 同步 HTTP 服务器（无 tokio 依赖，符合项目 smol 偏好）
//! - 专用 std::thread 运行服务器，recv_timeout(1s) 轮询 stop_flag
//! - 每个请求在独立 std::thread 中处理
//! - tiny_http::Request 不是 Send，必须在提取请求数据的线程内完成所有 Request 操作
//! - 设备操作通过 smol::block_on 在请求线程内执行，复用 `Arc<smol::Mutex<Option<RioDevice>>>`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cyrio_core::api::device::RioDevice;
use cyrio_core::api::upload::build_upload_header;
use cyrio_core::protocol::constants::{RIO_NUM_OFFSET, TYPE_MP3, TYPE_PLS};
use cyrio_core::protocol::fidl::{parse_fidl, serialize_fidl};
use serde::Serialize;
use smol::lock::Mutex as SmolMutex;

/// WebDAV 服务器绑定地址
const WEBDAV_ADDR: &str = "127.0.0.1:8765";

/// 缓存 TTL
const WEBDAV_CACHE_TTL: Duration = Duration::from_secs(10);

// ============================================================================
// WebDavServer：服务器入口
// ============================================================================

/// WebDAV 服务器状态（返回给 UI 层）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebDavStatus {
    /// 已停止
    Stopped,
    /// 运行中，含访问地址
    Running {
        /// 访问地址（如 http://127.0.0.1:8765）
        addr: String,
    },
    /// 发生错误
    Error(String),
}

/// WebDAV 服务器
///
/// 持有服务器线程句柄、停止标志、状态和 PROPFIND 缓存。
/// 通过 [`WebDavServer::new`] 创建，[`WebDavServer::start`] 启动，
/// [`WebDavServer::stop`] 停止。
pub struct WebDavServer {
    /// 服务器线程句柄
    thread: StdMutex<Option<JoinHandle<()>>>,
    /// 停止标志（服务器线程轮询）
    stop_flag: Arc<AtomicBool>,
    /// 当前状态
    status: StdMutex<WebDavStatus>,
    /// PROPFIND 缓存（避免 Finder 频繁请求导致 USB 独占）
    cache: Arc<PropfindCache>,
}

impl WebDavServer {
    /// 创建初始状态（未运行）
    pub fn new() -> Self {
        Self {
            thread: StdMutex::new(None),
            stop_flag: Arc::new(AtomicBool::new(false)),
            status: StdMutex::new(WebDavStatus::Stopped),
            cache: Arc::new(StdMutex::new(std::collections::HashMap::new())),
        }
    }

    /// 设置状态
    fn set_status(&self, status: WebDavStatus) {
        *self.status.lock().unwrap() = status;
    }

    /// 启动 WebDAV 服务器
    ///
    /// 在专用线程中运行 tiny_http 服务器，recv_timeout(1s) 轮询 stop_flag。
    /// 每个请求在独立 std::thread 中处理，复用 device 的 smol::Mutex。
    pub fn start(
        &self,
        device: Arc<SmolMutex<Option<RioDevice>>>,
    ) -> Result<String, String> {
        // 检查是否已在运行
        {
            let thread_guard = self.thread.lock().unwrap();
            if let Some(handle) = thread_guard.as_ref() {
                if !handle.is_finished() {
                    return Err("WebDAV 服务器已在运行".to_string());
                }
            }
        }

        // 检查设备是否已连接（同步检查，调用方应在 async 上下文中先检查）
        // 这里用 try_lock 立即检查，避免在同步上下文中阻塞
        // smol::lock::Mutex::try_lock() 返回 Option<MutexGuard>，不是 Result
        {
            match device.try_lock() {
                Some(guard) => {
                    if guard.is_none() {
                        return Err("设备未连接，无法启动 WebDAV".to_string());
                    }
                }
                None => {
                    // 设备锁被占用，假设设备已连接（避免误判）
                }
            }
        }

        // 创建服务器
        let server = tiny_http::Server::http(WEBDAV_ADDR)
            .map_err(|e| format!("绑定 {WEBDAV_ADDR} 失败: {e}"))?;

        // 重置 stop_flag
        self.stop_flag.store(false, Ordering::Relaxed);

        // 清空缓存（避免上次会话的过期条目影响新会话）
        {
            let mut cache = self.cache.lock().unwrap();
            cache.clear();
        }

        // 克隆共享状态
        let stop_flag = self.stop_flag.clone();
        let cache = self.cache.clone();

        // 启动服务器线程
        let handle = std::thread::Builder::new()
            .name("cyrio-webdav".into())
            .spawn(move || {
                log::info!("WebDAV 服务器启动于 http://{WEBDAV_ADDR}");
                loop {
                    // 检查停止标志
                    if stop_flag.load(Ordering::Relaxed) {
                        log::info!("WebDAV 服务器收到停止信号");
                        break;
                    }

                    // 等待请求（1 秒超时，便于轮询 stop_flag）
                    match server.recv_timeout(Duration::from_secs(1)) {
                        Ok(Some(req)) => {
                            let dev = device.clone();
                            let cache = cache.clone();
                            std::thread::Builder::new()
                                .name("cyrio-webdav-req".into())
                                .spawn(move || handle_request(req, dev, cache))
                                .ok();
                        }
                        Ok(None) => continue,
                        Err(e) => {
                            log::warn!("WebDAV 服务器 recv 错误: {e}");
                            continue;
                        }
                    }
                }
                log::info!("WebDAV 服务器线程退出");
            })
            .map_err(|e| format!("启动服务器线程失败: {e}"))?;

        // 保存线程句柄
        {
            let mut thread_guard = self.thread.lock().unwrap();
            *thread_guard = Some(handle);
        }

        // 更新状态
        let addr = format!("http://{WEBDAV_ADDR}");
        self.set_status(WebDavStatus::Running { addr: addr.clone() });

        Ok(addr)
    }

    /// 停止 WebDAV 服务器
    ///
    /// 设置停止标志后等待线程结束（最多 3 秒）。
    /// 调用方应在 async 上下文中用 `smol::unblock` 包装 join。
    pub fn stop(&self) -> Result<(), String> {
        self.stop_flag.store(true, Ordering::Relaxed);

        let handle = {
            let mut thread_guard = self.thread.lock().unwrap();
            thread_guard.take()
        };

        if let Some(handle) = handle {
            // 同步等待线程结束
            let _ = handle.join();
        }

        self.set_status(WebDavStatus::Stopped);
        Ok(())
    }

    /// 查询 WebDAV 服务器状态
    pub fn status(&self) -> WebDavStatus {
        self.status.lock().unwrap().clone()
    }

    /// 失效所有缓存（外部修改设备数据后调用）
    pub fn invalidate_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }
}

impl Default for WebDavServer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 自动挂载（平台特定）
// ============================================================================

/// 自动挂载 WebDAV 网络驱动器
///
/// macOS：用 `osascript` 让 Finder 挂载 WebDAV 卷到 /Volumes/
/// Windows：用 `net use` 映射网络驱动器到 Z: 盘
///
/// 这是一个阻塞操作（执行外部命令），调用方应在 `smol::unblock` 中调用。
pub fn mount_webdav() -> Result<String, String> {
    let url = format!("http://{WEBDAV_ADDR}");

    #[cfg(target_os = "macos")]
    {
        // 用 osascript 让 Finder 挂载 WebDAV 卷（而非 open 命令，后者会打开浏览器）
        let script = format!(
            r#"tell application "Finder" to mount volume "{url}""#
        );
        let result = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();
        match result {
            Ok(output) if output.status.success() => {
                Ok("已在 Finder 中挂载 WebDAV 卷".to_string())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("Finder 挂载失败: {stderr}"))
            }
            Err(e) => Err(format!("执行 osascript 失败: {e}")),
        }
    }

    #[cfg(target_os = "windows")]
    {
        let result = std::process::Command::new("net")
            .args(["use", "Z:", &url, "/persistent:no"])
            .output();
        match result {
            Ok(output) if output.status.success() => Ok("已映射到 Z: 盘".to_string()),
            Ok(output) => Err(format!(
                "挂载失败: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(e) => Err(format!("执行 net use 命令失败: {e}")),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("当前平台不支持自动挂载，请手动连接".to_string())
    }
}

// ============================================================================
// PROPFIND 缓存
// ============================================================================

/// PROPFIND 缓存（可跨线程共享）
type PropfindCache = StdMutex<std::collections::HashMap<String, PropfindCacheEntry>>;

/// PROPFIND 缓存条目
struct PropfindCacheEntry {
    self_entry: PropEntry,
    children: Vec<PropEntry>,
    fetched_at: Instant,
}

/// 缓存查询
fn cache_get(cache: &PropfindCache, key: &str) -> Option<(PropEntry, Vec<PropEntry>)> {
    let cache = cache.lock().unwrap();
    if let Some(entry) = cache.get(key) {
        if entry.fetched_at.elapsed() < WEBDAV_CACHE_TTL {
            return Some((entry.self_entry.clone(), entry.children.clone()));
        }
    }
    None
}

/// 缓存写入
fn cache_set(cache: &PropfindCache, key: &str, self_entry: PropEntry, children: Vec<PropEntry>) {
    let mut cache = cache.lock().unwrap();
    cache.insert(
        key.to_string(),
        PropfindCacheEntry {
            self_entry,
            children,
            fetched_at: Instant::now(),
        },
    );
}

/// 失效缓存（PUT/DELETE/MKCOL 后调用）
fn cache_invalidate_path(cache: &PropfindCache, key: &str) {
    let mut cache = cache.lock().unwrap();
    cache.remove(key);
    // 失效父目录缓存
    let path = key.trim_end_matches('/');
    if let Some((parent, _)) = path.rsplit_once('/') {
        let parent_key = format!("{parent}/");
        cache.remove(&parent_key);
    }
    // 失效 _全部歌曲 缓存（歌曲变化时）
    if key.contains("内置存储") || key.contains("SD卡") {
        cache.remove("/歌曲/_全部歌曲/");
    }
}

/// 尝试从父目录缓存中查找子项条目（避免文件级 PROPFIND 查设备）
fn try_cache_lookup_child(cache: &PropfindCache, cache_key: &str) -> Option<PropEntry> {
    let path = cache_key.trim_end_matches('/');
    if path.is_empty() || cache_key.ends_with('/') {
        return None;
    }
    let last_slash = path.rfind('/')?;
    let parent_key = format!("{}/", &path[..last_slash]);
    let filename = &path[last_slash + 1..];

    let (_, children) = cache_get(cache, &parent_key)?;
    children
        .iter()
        .find(|c| {
            let href_last = c.href.trim_end_matches('/').rsplit('/').next();
            href_last == Some(filename) || c.displayname == filename
        })
        .cloned()
}

// ============================================================================
// 虚拟路径解析
// ============================================================================

/// 解析后的虚拟路径
#[derive(Debug, Clone)]
enum VirtualPath {
    Root,
    SongsDir,
    SongsMem(u8),
    SongsAll,
    SongFile { mem_unit: u8, filename: String },
    PlaylistsDir,
    Playlist(String),
    PlaylistSong { playlist_name: String, filename: String },
    NotFound,
}

fn url_decode(s: &str) -> String {
    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}

fn normalize_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    url_decode(path)
}

fn resolve_path(url: &str) -> VirtualPath {
    let path = normalize_url(url);
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        return VirtualPath::Root;
    }
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        ["歌曲"] => VirtualPath::SongsDir,
        ["歌曲", "内置存储"] => VirtualPath::SongsMem(0),
        ["歌曲", "SD卡"] => VirtualPath::SongsMem(1),
        ["歌曲", "_全部歌曲"] => VirtualPath::SongsAll,
        ["歌曲", "内置存储", name] => VirtualPath::SongFile { mem_unit: 0, filename: name.to_string() },
        ["歌曲", "SD卡", name] => VirtualPath::SongFile { mem_unit: 1, filename: name.to_string() },
        ["歌单"] => VirtualPath::PlaylistsDir,
        ["歌单", name] => VirtualPath::Playlist(name.to_string()),
        ["歌单", playlist, filename] => VirtualPath::PlaylistSong {
            playlist_name: playlist.to_string(),
            filename: filename.to_string(),
        },
        _ => VirtualPath::NotFound,
    }
}

fn rio_name_to_filename(name: &str) -> String {
    if let Some(stripped) = name.strip_prefix("D:\\") {
        stripped.to_string()
    } else {
        name.to_string()
    }
}

// ============================================================================
// WebDAV 请求处理
// ============================================================================

fn read_request_body(req: &mut tiny_http::Request) -> Vec<u8> {
    let mut body = Vec::new();
    let _ = req.as_reader().read_to_end(&mut body);
    body
}

struct OwnedResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl OwnedResponse {
    fn new(status: u16) -> Self {
        Self { status, headers: Vec::new(), body: Vec::new() }
    }
    fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }
    fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }
    fn with_text(mut self, text: String) -> Self {
        self.body = text.into_bytes();
        self
    }
    fn respond(self, req: tiny_http::Request) {
        let mut response = if self.body.is_empty() {
            tiny_http::Response::from_string("")
        } else {
            tiny_http::Response::from_data(self.body)
        }
        .with_status_code(self.status);
        for (key, value) in &self.headers {
            if let Ok(header) = tiny_http::Header::from_bytes(key.as_bytes(), value.as_bytes()) {
                response = response.with_header(header);
            }
        }
        let _ = req.respond(response);
    }
}

fn handle_request(
    mut req: tiny_http::Request,
    device: Arc<SmolMutex<Option<RioDevice>>>,
    cache: Arc<PropfindCache>,
) {
    let method = req.method().as_str().to_string();
    let url = req.url().to_string();
    let depth = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Depth"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_else(|| "1".to_string());
    let body = read_request_body(&mut req);

    let response = smol::block_on(async move {
        process_webdav_request(&method, &url, &depth, &body, &device, &cache).await
    });
    response.respond(req);
}

async fn process_webdav_request(
    method: &str,
    url: &str,
    depth: &str,
    body: &[u8],
    device: &Arc<SmolMutex<Option<RioDevice>>>,
    cache: &Arc<PropfindCache>,
) -> OwnedResponse {
    log::debug!("WebDAV {} {} (depth={}, body={}B)", method, url, depth, body.len());
    match method {
        "OPTIONS" => handle_options(),
        "PROPFIND" => handle_propfind(url, depth, device, cache).await,
        "GET" | "HEAD" => handle_get(url, device).await,
        "PUT" => handle_put(url, body, device, cache).await,
        "DELETE" => handle_delete(url, device, cache).await,
        "MKCOL" => handle_mkcol(url, device, cache).await,
        _ => OwnedResponse::new(405).with_header("Allow", "OPTIONS, PROPFIND, GET, PUT, DELETE, MKCOL"),
    }
}

fn handle_options() -> OwnedResponse {
    OwnedResponse::new(200)
        .with_header("DAV", "1, 2")
        .with_header("Allow", "OPTIONS, PROPFIND, GET, PUT, DELETE, MKCOL")
        .with_header("MS-Author-Via", "DAV")
}

// ============================================================================
// PROPFIND
// ============================================================================

async fn handle_propfind(
    url: &str,
    depth: &str,
    device: &Arc<SmolMutex<Option<RioDevice>>>,
    cache: &Arc<PropfindCache>,
) -> OwnedResponse {
    if depth == "infinity" {
        return OwnedResponse::new(403).with_text("Depth: infinity not supported".to_string());
    }
    let cache_key = normalize_url(url);

    if let Some((self_entry, children)) = cache_get(cache, &cache_key) {
        let depth_zero = depth == "0";
        let xml = build_multistatus_xml(&self_entry, &children, depth_zero);
        return OwnedResponse::new(207)
            .with_header("Content-Type", "application/xml; charset=utf-8")
            .with_header("DAV", "1, 2")
            .with_text(xml);
    }

    if depth == "0" {
        if let Some(child) = try_cache_lookup_child(cache, &cache_key) {
            let xml = build_multistatus_xml(&child, &[], true);
            return OwnedResponse::new(207)
                .with_header("Content-Type", "application/xml; charset=utf-8")
                .with_header("DAV", "1, 2")
                .with_text(xml);
        }
    }

    let vpath = resolve_path(url);
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => return OwnedResponse::new(503).with_text("设备未连接".to_string()),
    };

    let (self_entry, children) = match collect_propfind_entries(&vpath, dev).await {
        Ok(e) => e,
        Err(msg) => return OwnedResponse::new(404).with_text(msg),
    };

    cache_set(cache, &cache_key, self_entry.clone(), children.clone());

    let depth_zero = depth == "0";
    let xml = build_multistatus_xml(&self_entry, &children, depth_zero);
    OwnedResponse::new(207)
        .with_header("Content-Type", "application/xml; charset=utf-8")
        .with_header("DAV", "1, 2")
        .with_text(xml)
}

#[derive(Clone)]
struct PropEntry {
    href: String,
    displayname: String,
    is_collection: bool,
    content_length: u64,
    content_type: String,
    last_modified: String,
    etag: String,
}

impl PropEntry {
    fn collection(href: String, displayname: String) -> Self {
        let etag = format!("\"dir-{}\"", stable_hash(&href));
        Self {
            href, displayname, is_collection: true,
            content_length: 0, content_type: String::new(),
            last_modified: webdav_epoch_date(), etag,
        }
    }
    fn file(href: String, displayname: String, content_length: u64, file_no: u32) -> Self {
        Self {
            href, displayname, is_collection: false,
            content_length, content_type: "audio/mpeg".to_string(),
            last_modified: webdav_epoch_date(),
            etag: format!("\"file-{file_no}-{content_length}\""),
        }
    }
}

static WEBDAV_EPOCH: std::sync::OnceLock<u64> = std::sync::OnceLock::new();

fn webdav_epoch_secs() -> u64 {
    *WEBDAV_EPOCH.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    })
}

fn webdav_epoch_date() -> String {
    http_date(webdav_epoch_secs())
}

fn stable_hash(s: &str) -> u64 {
    s.bytes()
        .fold(0xcbf29ce484222325u64, |acc, b| {
            acc.wrapping_mul(0x100000001b3).wrapping_add(b as u64)
        })
}

fn http_date(secs: u64) -> String {
    let days = secs / 86400;
    let remainder = secs % 86400;
    let hour = remainder / 3600;
    let minute = (remainder % 3600) / 60;
    let second = remainder % 60;
    let weekday = (days + 4) % 7;
    let weekday_name = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][weekday as usize];
    let (year, month, day) = days_to_ymd(days);
    let month_name = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][(month - 1) as usize];
    format!("{weekday_name}, {day:02} {month_name} {year} {hour:02}:{minute:02}:{second:02} GMT")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    let mut remaining = days;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year { break; }
        remaining -= days_in_year;
        year += 1;
    }
    let month_days = if is_leap_year(year) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &dim in &month_days {
        if remaining < dim { break; }
        remaining -= dim;
        month += 1;
    }
    (year, month, remaining + 1)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn url_encode_path(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => result.push_str(&format!("%{byte:02X}")),
        }
    }
    result
}

async fn collect_propfind_entries(
    vpath: &VirtualPath,
    device: &RioDevice,
) -> Result<(PropEntry, Vec<PropEntry>), String> {
    match vpath {
        VirtualPath::Root => {
            let self_entry = PropEntry::collection("/".to_string(), "/".to_string());
            let children = vec![
                PropEntry::collection("/歌曲/".to_string(), "歌曲".to_string()),
                PropEntry::collection("/歌单/".to_string(), "歌单".to_string()),
            ];
            Ok((self_entry, children))
        }
        VirtualPath::SongsDir => {
            let self_entry = PropEntry::collection("/歌曲/".to_string(), "歌曲".to_string());
            let children = vec![
                PropEntry::collection("/歌曲/内置存储/".to_string(), "内置存储".to_string()),
                PropEntry::collection("/歌曲/SD卡/".to_string(), "SD卡".to_string()),
                PropEntry::collection("/歌曲/_全部歌曲/".to_string(), "_全部歌曲".to_string()),
            ];
            Ok((self_entry, children))
        }
        VirtualPath::SongsMem(mem_unit) => {
            let label = mem_label(*mem_unit);
            let self_href = format!("/歌曲/{label}/");
            let self_entry = PropEntry::collection(self_href.clone(), label.to_string());
            let files = device.list_files(*mem_unit, |_| {}).await.map_err(|e| e.to_string())?;
            let children: Vec<PropEntry> = files
                .iter()
                .filter(|f| f.file_type == TYPE_MP3)
                .map(|f| {
                    let filename = rio_name_to_filename(&f.name);
                    PropEntry::file(format!("/歌曲/{label}/{filename}"), filename, f.size as u64, f.file_no)
                })
                .collect();
            Ok((self_entry, children))
        }
        VirtualPath::SongsAll => {
            let self_entry = PropEntry::collection("/歌曲/_全部歌曲/".to_string(), "_全部歌曲".to_string());
            let mut children = Vec::new();
            for mem_unit in 0..=1u8 {
                if let Ok(files) = device.list_files(mem_unit, |_| {}).await {
                    for f in files.iter().filter(|f| f.file_type == TYPE_MP3) {
                        let filename = rio_name_to_filename(&f.name);
                        children.push(PropEntry::file(
                            format!("/歌曲/_全部歌曲/{filename}"), filename, f.size as u64, f.file_no,
                        ));
                    }
                }
            }
            Ok((self_entry, children))
        }
        VirtualPath::SongFile { mem_unit, filename } => {
            let label = mem_label(*mem_unit);
            let files = device.list_files(*mem_unit, |_| {}).await.map_err(|e| e.to_string())?;
            let f = files
                .iter()
                .find(|f| f.file_type == TYPE_MP3 && rio_name_to_filename(&f.name) == *filename)
                .ok_or_else(|| format!("文件未找到: {filename}"))?;
            let href = format!("/歌曲/{label}/{filename}");
            let self_entry = PropEntry::file(href, filename.clone(), f.size as u64, f.file_no);
            Ok((self_entry, Vec::new()))
        }
        VirtualPath::PlaylistsDir => {
            let self_entry = PropEntry::collection("/歌单/".to_string(), "歌单".to_string());
            let mut children = Vec::new();
            for mem_unit in 0..=1u8 {
                if let Ok(files) = device.list_files(mem_unit, |_| {}).await {
                    for f in files.iter().filter(|f| f.file_type == TYPE_PLS) {
                        children.push(PropEntry::collection(
                            format!("/歌单/{}/", f.name), f.name.clone(),
                        ));
                    }
                }
            }
            Ok((self_entry, children))
        }
        VirtualPath::Playlist(playlist_name) => {
            let self_href = format!("/歌单/{playlist_name}/");
            let self_entry = PropEntry::collection(self_href, playlist_name.clone());
            let (playlist_file_no, playlist_mem) = find_playlist_by_name(device, playlist_name)
                .await
                .ok_or_else(|| format!("歌单未找到: {playlist_name}"))?;
            let songs = cyrio_core::api::playlist::list_playlist_songs(device, playlist_file_no, playlist_mem)
                .await
                .map_err(|e| e.to_string())?;
            let children: Vec<PropEntry> = songs
                .iter()
                .map(|ps| {
                    let filename = rio_name_to_filename(&ps.song.name);
                    PropEntry::file(
                        format!("/歌单/{playlist_name}/{filename}"), filename,
                        ps.song.size as u64, ps.song.file_no,
                    )
                })
                .collect();
            Ok((self_entry, children))
        }
        VirtualPath::PlaylistSong { playlist_name, filename } => {
            let (playlist_file_no, playlist_mem) = find_playlist_by_name(device, playlist_name)
                .await
                .ok_or_else(|| format!("歌单未找到: {playlist_name}"))?;
            let songs = cyrio_core::api::playlist::list_playlist_songs(device, playlist_file_no, playlist_mem)
                .await
                .map_err(|e| e.to_string())?;
            let ps = songs
                .iter()
                .find(|ps| rio_name_to_filename(&ps.song.name) == *filename)
                .ok_or_else(|| format!("歌曲未找到: {filename}"))?;
            let href = format!("/歌单/{playlist_name}/{filename}");
            let self_entry = PropEntry::file(href, filename.clone(), ps.song.size as u64, ps.song.file_no);
            Ok((self_entry, Vec::new()))
        }
        VirtualPath::NotFound => Err("路径未找到".to_string()),
    }
}

fn mem_label(mem_unit: u8) -> &'static str {
    if mem_unit == 0 { "内置存储" } else { "SD卡" }
}

async fn find_playlist_by_name(device: &RioDevice, name: &str) -> Option<(u32, u8)> {
    for mem_unit in 0..=1u8 {
        if let Ok(files) = device.list_files(mem_unit, |_| {}).await {
            for f in files.iter().filter(|f| f.file_type == TYPE_PLS) {
                if f.name == name {
                    return Some((f.file_no, mem_unit));
                }
            }
        }
    }
    None
}

fn build_multistatus_xml(self_entry: &PropEntry, children: &[PropEntry], depth_zero: bool) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
"#);
    xml.push_str(&format_entry_xml(self_entry));
    if !depth_zero {
        for entry in children {
            xml.push_str(&format_entry_xml(entry));
        }
    }
    xml.push_str("</D:multistatus>\n");
    xml
}

fn format_entry_xml(entry: &PropEntry) -> String {
    let resource_type = if entry.is_collection { "<D:collection/>".to_string() } else { String::new() };
    let content_length_xml = if entry.is_collection {
        String::new()
    } else {
        format!("<D:getcontentlength>{}</D:getcontentlength>", entry.content_length)
    };
    let content_type_xml = if entry.content_type.is_empty() {
        String::new()
    } else {
        format!("<D:getcontenttype>{}</D:getcontenttype>", entry.content_type)
    };
    format!(
        r#"  <D:response>
    <D:href>{href}</D:href>
    <D:propstat>
      <D:prop>
        <D:resourcetype>{resource_type}</D:resourcetype>
        <D:displayname>{displayname}</D:displayname>
        {content_length_xml}
        {content_type_xml}
        <D:getlastmodified>{last_modified}</D:getlastmodified>
        <D:getetag>{etag}</D:getetag>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
"#,
        href = url_encode_path(&entry.href),
        displayname = xml_escape(&entry.displayname),
        resource_type = resource_type,
        content_length_xml = content_length_xml,
        content_type_xml = content_type_xml,
        last_modified = entry.last_modified,
        etag = entry.etag,
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ============================================================================
// GET
// ============================================================================

async fn handle_get(url: &str, device: &Arc<SmolMutex<Option<RioDevice>>>) -> OwnedResponse {
    let vpath = resolve_path(url);
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => return OwnedResponse::new(503).with_text("设备未连接".to_string()),
    };
    match vpath {
        VirtualPath::SongFile { mem_unit, filename } => {
            let file_no = match find_song_by_name(dev, mem_unit, &filename).await {
                Some(no) => no,
                None => return OwnedResponse::new(404).with_text("文件未找到".to_string()),
            };
            let result = match dev.download_file(mem_unit, file_no, |_| {}).await {
                Ok(r) => r,
                Err(e) => return OwnedResponse::new(500).with_text(format!("下载失败: {e}")),
            };
            OwnedResponse::new(200).with_header("Content-Type", "audio/mpeg").with_body(result.data)
        }
        VirtualPath::PlaylistSong { playlist_name, filename } => {
            let (playlist_file_no, playlist_mem) = match find_playlist_by_name(dev, &playlist_name).await {
                Some(v) => v,
                None => return OwnedResponse::new(404).with_text("歌单未找到".to_string()),
            };
            let songs = match cyrio_core::api::playlist::list_playlist_songs(dev, playlist_file_no, playlist_mem).await {
                Ok(s) => s,
                Err(e) => return OwnedResponse::new(500).with_text(format!("读取歌单失败: {e}")),
            };
            let ps = match songs.iter().find(|ps| rio_name_to_filename(&ps.song.name) == filename) {
                Some(s) => s,
                None => return OwnedResponse::new(404).with_text("歌曲未找到".to_string()),
            };
            let result = match dev.download_file(ps.mem_unit, ps.song.file_no, |_| {}).await {
                Ok(r) => r,
                Err(e) => return OwnedResponse::new(500).with_text(format!("下载失败: {e}")),
            };
            OwnedResponse::new(200).with_header("Content-Type", "audio/mpeg").with_body(result.data)
        }
        _ => OwnedResponse::new(403).with_text("该路径不支持 GET".to_string()),
    }
}

async fn find_song_by_name(device: &RioDevice, mem_unit: u8, filename: &str) -> Option<u32> {
    let files = device.list_files(mem_unit, |_| {}).await.ok()?;
    files
        .iter()
        .find(|f| f.file_type == TYPE_MP3 && rio_name_to_filename(&f.name) == filename)
        .map(|f| f.file_no)
}

// ============================================================================
// PUT
// ============================================================================

async fn handle_put(
    url: &str,
    body: &[u8],
    device: &Arc<SmolMutex<Option<RioDevice>>>,
    cache: &Arc<PropfindCache>,
) -> OwnedResponse {
    let vpath = resolve_path(url);
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => return OwnedResponse::new(503).with_text("设备未连接".to_string()),
    };
    let result = match vpath {
        VirtualPath::SongsMem(mem_unit) | VirtualPath::SongFile { mem_unit, .. } => {
            let filename = extract_filename_from_url(url).unwrap_or_else(|| "upload.mp3".to_string());
            match upload_from_data(dev, body.to_vec(), mem_unit, filename).await {
                Ok(_) => {
                    let label = mem_label(mem_unit);
                    cache_invalidate_path(cache, &format!("/歌曲/{label}/"));
                    OwnedResponse::new(201)
                }
                Err(e) => OwnedResponse::new(500).with_text(format!("上传失败: {e}")),
            }
        }
        VirtualPath::Playlist(playlist_name) => {
            let filename = extract_filename_from_url(url).unwrap_or_else(|| "upload.mp3".to_string());
            let file_no = match upload_from_data(dev, body.to_vec(), 0, filename.clone()).await {
                Ok(no) => no,
                Err(e) => return OwnedResponse::new(500).with_text(format!("上传失败: {e}")),
            };
            let (playlist_file_no, playlist_mem) = match find_playlist_by_name(dev, &playlist_name).await {
                Some(v) => v,
                None => return OwnedResponse::new(404).with_text(format!("歌单未找到: {playlist_name}")),
            };
            match cyrio_core::api::playlist::add_to_playlist(dev, file_no, 0, playlist_file_no, playlist_mem).await {
                Ok(()) => {
                    cache_invalidate_path(cache, &format!("/歌单/{playlist_name}/"));
                    cache_invalidate_path(cache, "/歌曲/内置存储/");
                    OwnedResponse::new(201)
                }
                Err(e) => OwnedResponse::new(500).with_text(format!("加入歌单失败: {e}")),
            }
        }
        VirtualPath::PlaylistSong { .. } => {
            OwnedResponse::new(403).with_text("歌单内歌曲不支持覆盖，请先删除再上传".to_string())
        }
        VirtualPath::SongsAll => OwnedResponse::new(403).with_text("_全部歌曲 只读".to_string()),
        _ => OwnedResponse::new(403).with_text("该路径不支持 PUT".to_string()),
    };
    result
}

fn extract_filename_from_url(url: &str) -> Option<String> {
    let path = normalize_url(url);
    let path = path.trim_end_matches('/');
    path.rsplit('/').next().map(|s| s.to_string())
}

async fn upload_from_data(
    device: &RioDevice,
    data: Vec<u8>,
    mem_unit: u8,
    filename: String,
) -> Result<u32, String> {
    if data.is_empty() {
        return Err("文件数据为空".to_string());
    }
    let (header, id3v2_size) = build_upload_header(
        &data,
        &filename,
        &cyrio_core::api::upload::UploadTextOptions::default(),
    );
    let audio_data = &data[id3v2_size..];
    cyrio_core::api::types::precheck_free_space(device, mem_unit, audio_data.len())
        .await
        .map_err(|e| e.to_string())?;
    device
        .upload_file(mem_unit, &header, audio_data, |_| {}, None)
        .await
        .map_err(|e| e.to_string())
}

// ============================================================================
// DELETE
// ============================================================================

async fn handle_delete(
    url: &str,
    device: &Arc<SmolMutex<Option<RioDevice>>>,
    cache: &Arc<PropfindCache>,
) -> OwnedResponse {
    let vpath = resolve_path(url);
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => return OwnedResponse::new(503).with_text("设备未连接".to_string()),
    };
    match vpath {
        VirtualPath::SongFile { mem_unit, filename } => {
            let file_no = match find_song_by_name(dev, mem_unit, &filename).await {
                Some(no) => no,
                None => return OwnedResponse::new(404).with_text("文件未找到".to_string()),
            };
            match dev.delete_file(mem_unit, file_no).await {
                Ok(()) => {
                    let label = mem_label(mem_unit);
                    cache_invalidate_path(cache, &format!("/歌曲/{label}/"));
                    OwnedResponse::new(204)
                }
                Err(e) => OwnedResponse::new(500).with_text(format!("删除失败: {e}")),
            }
        }
        VirtualPath::PlaylistSong { playlist_name, filename } => {
            match remove_from_playlist_by_name(dev, &playlist_name, &filename).await {
                Ok(()) => {
                    cache_invalidate_path(cache, &format!("/歌单/{playlist_name}/"));
                    OwnedResponse::new(204)
                }
                Err(e) => OwnedResponse::new(500).with_text(format!("移除歌单引用失败: {e}")),
            }
        }
        VirtualPath::Playlist(playlist_name) => {
            let (playlist_file_no, playlist_mem) = match find_playlist_by_name(dev, &playlist_name).await {
                Some(v) => v,
                None => return OwnedResponse::new(404).with_text("歌单未找到".to_string()),
            };
            match dev.delete_file(playlist_mem, playlist_file_no).await {
                Ok(()) => {
                    cache_invalidate_path(cache, &format!("/歌单/{playlist_name}/"));
                    cache_invalidate_path(cache, "/歌单/");
                    OwnedResponse::new(204)
                }
                Err(e) => OwnedResponse::new(500).with_text(format!("删除歌单失败: {e}")),
            }
        }
        VirtualPath::SongsAll => OwnedResponse::new(403).with_text("_全部歌曲 只读".to_string()),
        _ => OwnedResponse::new(403).with_text("该路径不支持 DELETE".to_string()),
    }
}

async fn remove_from_playlist_by_name(
    device: &RioDevice,
    playlist_name: &str,
    song_filename: &str,
) -> Result<(), String> {
    let (playlist_file_no, playlist_mem) = find_playlist_by_name(device, playlist_name)
        .await
        .ok_or_else(|| format!("歌单未找到: {playlist_name}"))?;

    let mut song_file_no: Option<u32> = None;
    let mut song_mem_unit: u8 = 0;
    for mem_unit in 0..=1u8 {
        if let Ok(files) = device.list_files(mem_unit, |_| {}).await {
            for f in files.iter().filter(|f| f.file_type == TYPE_MP3) {
                if rio_name_to_filename(&f.name) == song_filename {
                    song_file_no = Some(f.file_no);
                    song_mem_unit = mem_unit;
                    break;
                }
            }
        }
        if song_file_no.is_some() { break; }
    }
    let song_file_no = song_file_no.ok_or_else(|| format!("歌曲未找到: {song_filename}"))?;

    let download = device.download_file(playlist_mem, playlist_file_no, |_| {}).await.map_err(|e| e.to_string())?;
    let mut header = download.header;
    let header_buffer = download.header_buffer;
    let fidl_data = &download.data;

    let playlist = parse_fidl(fidl_data).map_err(|e| e.to_string())?;
    let target_rio_num = if song_mem_unit == 1 { song_file_no } else { song_file_no + RIO_NUM_OFFSET };
    let filtered_entries: Vec<_> = playlist
        .entries
        .iter()
        .filter(|entry| entry.rio_num != target_rio_num)
        .cloned()
        .collect();
    let removed = playlist.entries.len() - filtered_entries.len();
    if removed == 0 {
        return Err("歌单中未找到该歌曲".to_string());
    }
    log::info!("remove_from_playlist: 移除 {} 首歌曲（rio_num=0x{:06x}）", removed, target_rio_num);

    let new_fid = serialize_fidl(&cyrio_core::protocol::fidl::FidlPlaylist { entries: filtered_entries });
    cyrio_core::api::types::precheck_free_space(device, playlist_mem, new_fid.len())
        .await
        .map_err(|e| e.to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    header.size = new_fid.len() as u32;
    header.mod_date = now;

    device
        .overwrite_file(playlist_mem, playlist_file_no, &header, &new_fid, |_| {}, Some(&*header_buffer))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================================
// MKCOL
// ============================================================================

async fn handle_mkcol(
    url: &str,
    device: &Arc<SmolMutex<Option<RioDevice>>>,
    cache: &Arc<PropfindCache>,
) -> OwnedResponse {
    let vpath = resolve_path(url);
    let guard = device.lock().await;
    let dev = match guard.as_ref() {
        Some(d) => d,
        None => return OwnedResponse::new(503).with_text("设备未连接".to_string()),
    };
    match vpath {
        VirtualPath::Playlist(playlist_name) => {
            match cyrio_core::api::playlist::create_playlist(dev, &playlist_name, 0).await {
                Ok(_) => {
                    cache_invalidate_path(cache, "/歌单/");
                    OwnedResponse::new(201)
                }
                Err(e) => OwnedResponse::new(500).with_text(format!("创建歌单失败: {e}")),
            }
        }
        VirtualPath::SongsAll => OwnedResponse::new(403).with_text("_全部歌曲 只读".to_string()),
        _ => OwnedResponse::new(403).with_text("仅支持在 /歌单/ 下创建目录".to_string()),
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_decode_basic() {
        assert_eq!(url_decode("hello"), "hello");
        assert_eq!(url_decode("%E6%AD%8C%E6%9B%B2"), "歌曲");
        assert_eq!(url_decode("/%E6%AD%8C%E6%9B%B2/"), "/歌曲/");
    }

    #[test]
    fn normalize_url_strips_query() {
        assert_eq!(normalize_url("/歌曲/?x=1"), "/歌曲/");
    }

    #[test]
    fn resolve_path_root() {
        assert!(matches!(resolve_path("/"), VirtualPath::Root));
        assert!(matches!(resolve_path(""), VirtualPath::Root));
    }

    #[test]
    fn resolve_path_songs_dir() {
        assert!(matches!(resolve_path("/歌曲/"), VirtualPath::SongsDir));
        assert!(matches!(resolve_path("/歌曲"), VirtualPath::SongsDir));
    }

    #[test]
    fn resolve_path_songs_mem() {
        assert!(matches!(resolve_path("/歌曲/内置存储/"), VirtualPath::SongsMem(0)));
        assert!(matches!(resolve_path("/歌曲/SD卡/"), VirtualPath::SongsMem(1)));
    }

    #[test]
    fn resolve_path_song_file() {
        match resolve_path("/歌曲/内置存储/test.mp3") {
            VirtualPath::SongFile { mem_unit, filename } => {
                assert_eq!(mem_unit, 0);
                assert_eq!(filename, "test.mp3");
            }
            _ => panic!("expected SongFile"),
        }
    }

    #[test]
    fn resolve_path_playlist() {
        match resolve_path("/歌单/我的歌单/") {
            VirtualPath::Playlist(name) => assert_eq!(name, "我的歌单"),
            _ => panic!("expected Playlist"),
        }
    }

    #[test]
    fn resolve_path_playlist_song() {
        match resolve_path("/歌单/我的歌单/song.mp3") {
            VirtualPath::PlaylistSong { playlist_name, filename } => {
                assert_eq!(playlist_name, "我的歌单");
                assert_eq!(filename, "song.mp3");
            }
            _ => panic!("expected PlaylistSong"),
        }
    }

    #[test]
    fn rio_name_to_filename_strips_prefix() {
        assert_eq!(rio_name_to_filename("D:\\test.mp3"), "test.mp3");
        assert_eq!(rio_name_to_filename("test.mp3"), "test.mp3");
    }

    #[test]
    fn extract_filename_from_url_basic() {
        assert_eq!(
            extract_filename_from_url("/歌曲/内置存储/test.mp3"),
            Some("test.mp3".to_string())
        );
    }
}
