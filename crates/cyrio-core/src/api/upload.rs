//! 上传相关 API：ID3v2 解析 + MP3 上传 + 批量上传 + 路径展开
//!
//! 把原本散落在 `cyrio-app/src/task.rs` 和 `cyrio-tauri/src/commands.rs` 的
//! 重复逻辑（ID3 解析、header 构造、上传流程、目录展开）统一抽到这里，
//! 让两个 UI 层都调用同一份实现。
//!
//! # ID3 解析
//! 支持 ID3v2.3/v2.4，提取 title/artist/album/year/genre/track/composer/cover_art。
//! 解析失败时返回空对象（不抛错），调用方可用文件名兜底。
//!
//! # 上传流程
//! 读文件 → 跳过 ID3v2 → 提取 ID3 标签 → 存储空间预检 → 构造 rio_file_t → upload_file

use std::path::{Path, PathBuf};

use cyrio_text::{SlugOptions, StripOptions, strip_noise, to_slug};

use crate::api::device::{RioDevice, UploadProgress};
use crate::api::types::precheck_free_space;
use crate::error::{CyrioError, Result};
use crate::protocol::constants::TYPE_MP3;
use crate::protocol::rio_file::RioFile;

/// ID3v2 标签解析结果
#[derive(Debug, Default, Clone)]
pub struct Id3Tags {
    /// 标题（TIT2）
    pub title: String,
    /// 艺术家（TPE1）
    pub artist: String,
    /// 专辑（TALB）
    pub album: String,
    /// 年份（TYER / TDRC）
    pub year: String,
    /// 流派（TCON）
    pub genre: String,
    /// 音轨号（TRCK）
    pub track: String,
    /// 作曲（TCOM）
    pub composer: String,
    /// 专辑封面（APIC 帧的图片数据）
    pub cover_art: Option<Vec<u8>>,
}

/// 批量上传单个文件结果
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// 文件路径
    pub path: PathBuf,
    /// 是否成功
    pub success: bool,
    /// 成功时为 file_no，失败时为 -1
    pub file_no: i64,
    /// 错误信息（失败时）
    pub error: String,
}

/// 上传时文本处理选项（由 UI 层从 AppSettings 构造传入）
///
/// 控制上传 MP3 时是否对 title 应用 slug（中文→拼音）和 strip（去词）。
/// 解决"歌曲传输编码没同步"问题：playlist 用 rename.rs 已能正确写中文，
/// song 上传用此结构同步应用文本处理，保证一致体验。
///
/// `#[serde(rename_all = "camelCase")]`：Tauri 2.0 前端用 camelCase 传参
/// （如 `applySlug` 而非 `apply_slug`），serde 需要匹配 camelCase 字段名。
/// `#[serde(default)]`：允许前端省略部分字段，缺失字段使用 Default 值。
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UploadTextOptions {
    /// 是否应用 slug（中文→拼音）
    pub apply_slug: bool,
    /// 是否应用 strip（去词）
    pub apply_strip: bool,
    /// 是否去除括号内容
    pub strip_parentheses: bool,
    /// 是否去除引号内容
    pub strip_quotes: bool,
    /// 是否去除音质/规格停用词
    pub strip_quality_tags: bool,
    /// 自定义停用词
    pub custom_stop_words: Vec<String>,
}

/// 根据文本处理选项处理标题
///
/// 处理顺序：先 strip（去词），再 slug（转拼音）。
/// 若两个选项都关闭，返回原标题。
///
/// # 示例
/// ```
/// use cyrio_core::api::upload::{process_title, UploadTextOptions};
///
/// // 仅 slug（保留括号内容）：括号内中文转拼音，英文原样保留
/// let slug_only = UploadTextOptions {
///     apply_slug: true,
///     apply_strip: false,
///     ..Default::default()
/// };
/// assert_eq!(
///     process_title("【洛天依 原创】Hi-Res 赛马", &slug_only),
///     "【Luo-Tian-Yi Yuan-Chuang】Hi-Res Sai-Ma"
/// );
///
/// // slug + strip（默认去除括号内容与 Hi-Res 停用词）
/// let both = UploadTextOptions {
///     apply_slug: true,
///     apply_strip: true,
///     strip_parentheses: true,
///     strip_quotes: true,
///     strip_quality_tags: true,
///     custom_stop_words: vec![],
/// };
/// assert_eq!(process_title("【洛天依 原创】Hi-Res 赛马", &both), "Sai-Ma");
/// ```
pub fn process_title(title: &str, opts: &UploadTextOptions) -> String {
    let mut result = title.to_string();
    if opts.apply_strip {
        let strip_opts = StripOptions {
            custom_stop_words: opts.custom_stop_words.clone(),
            strip_parentheses: opts.strip_parentheses,
            strip_quotes: opts.strip_quotes,
            strip_quality_tags: opts.strip_quality_tags,
        };
        result = strip_noise(&result, &strip_opts).cleaned;
    }
    if opts.apply_slug {
        let slug_opts = SlugOptions::default();
        result = to_slug(&result, &slug_opts);
    }
    result
}

// ============================================================================
// ID3v2 解析
// ============================================================================

/// 计算 ID3v2 标签的总字节数（含 10B 头）
///
/// 若文件不以 "ID3" 魔数开头，返回 0。
/// ID3v2 大小字段使用 syncsafe 编码（每字节仅低 7 位有效）。
pub fn get_id3v2_size(buf: &[u8]) -> usize {
    if buf.len() < 10 {
        return 0;
    }
    if buf[0] != b'I' || buf[1] != b'D' || buf[2] != b'3' {
        return 0;
    }
    let size = ((buf[6] as usize & 0x7f) << 21)
        | ((buf[7] as usize & 0x7f) << 14)
        | ((buf[8] as usize & 0x7f) << 7)
        | (buf[9] as usize & 0x7f);
    if size > 16 * 1024 * 1024 {
        return 0;
    }
    10 + size
}

/// 从 MP3 文件缓冲区提取 ID3 标签
///
/// 支持 ID3v2.3/v2.4。解析失败时返回空对象（不抛错），调用方可用文件名兜底。
pub fn read_id3_tags(buf: &[u8]) -> Id3Tags {
    let mut tags = Id3Tags::default();
    if buf.len() < 10 || &buf[0..3] != b"ID3" {
        return tags;
    }
    let id3v2_size = get_id3v2_size(buf);
    if id3v2_size > buf.len() {
        return tags;
    }
    let body = &buf[10..id3v2_size];
    let mut pos = 0;
    while pos + 10 <= body.len() {
        let frame_id = &body[pos..pos + 4];
        if frame_id == b"\0\0\0\0" {
            break;
        }
        let size = ((body[pos + 4] as usize) << 24)
            | ((body[pos + 5] as usize) << 16)
            | ((body[pos + 6] as usize) << 8)
            | (body[pos + 7] as usize);
        if pos + 10 + size > body.len() {
            break;
        }
        let frame_data = &body[pos + 10..pos + 10 + size];
        match frame_id {
            b"TIT2" => tags.title = decode_id3_text(frame_data),
            b"TPE1" => tags.artist = decode_id3_text(frame_data),
            b"TALB" => tags.album = decode_id3_text(frame_data),
            b"TYER" | b"TDRC" => tags.year = decode_id3_text(frame_data),
            b"TCON" => tags.genre = decode_id3_text(frame_data),
            b"TRCK" => tags.track = decode_id3_text(frame_data),
            b"TCOM" => tags.composer = decode_id3_text(frame_data),
            b"APIC" => tags.cover_art = parse_apic_frame(frame_data),
            _ => {}
        }
        pos += 10 + size;
    }
    tags
}

/// 解析 APIC 帧，提取图片数据
fn parse_apic_frame(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 {
        return None;
    }
    let mut p = 1; // skip encoding byte
    // 跳过 MIME type（以 \0 结尾）
    while p < data.len() && data[p] != 0 {
        p += 1;
    }
    p += 1; // skip \0
    if p >= data.len() {
        return None;
    }
    p += 1; // skip picture type byte
    // 跳过 description（以 \0 结尾）
    while p < data.len() && data[p] != 0 {
        p += 1;
    }
    p += 1; // skip \0
    if p < data.len() {
        Some(data[p..].to_vec())
    } else {
        None
    }
}

/// 解码 ID3 文本帧
///
/// 第一个字节是编码标志：0=ISO-8859-1, 1=UTF-16LE, 2=UTF-16BE, 3=UTF-8
fn decode_id3_text(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let encoding = data[0];
    let text = &data[1..];
    match encoding {
        0 | 3 => String::from_utf8_lossy(
            &text.iter().take_while(|&&b| b != 0).copied().collect::<Vec<_>>(),
        )
        .into_owned(),
        1 => {
            let bytes: Vec<u8> = text.iter().take_while(|&&b| b != 0).copied().collect();
            String::from_utf16_lossy(
                &bytes
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
        }
        2 => {
            let bytes: Vec<u8> = text.iter().take_while(|&&b| b != 0).copied().collect();
            String::from_utf16_lossy(
                &bytes
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
        }
        _ => String::new(),
    }
}

// ============================================================================
// 上传 header 构造
// ============================================================================

/// 构造上传用 rio_file_t header
///
/// - 跳过 ID3v2 标签，取纯音频数据长度作为 size
/// - 提取 ID3 标签（title/artist/album），title 为空时用文件名兜底
/// - 应用 `text_opts` 处理 title（slug/strip），与 playlist 编码同步
/// - name 字段统一为 `D:\<文件名>` 格式（匹配 Windows 原版软件行为）
/// - file_type = TYPE_MP3，file_no/start 由设备分配
pub fn build_upload_header(
    file_data: &[u8],
    file_name: &str,
    text_opts: &UploadTextOptions,
) -> (RioFile, usize) {
    let id3v2_size = get_id3v2_size(file_data);
    let audio_data = &file_data[id3v2_size..];
    let id3_tags = read_id3_tags(file_data);

    let name_field = if file_name.starts_with("D:\\") {
        file_name.to_string()
    } else {
        format!("D:\\{}", file_name)
    };

    let file_stem = Path::new(file_name)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let title = if !id3_tags.title.is_empty() {
        id3_tags.title
    } else {
        file_stem
    };
    // 应用文本处理（slug/strip），与 playlist 编码同步
    let title = process_title(&title, text_opts);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);

    let header = RioFile {
        file_no: 0, // 设备分配
        start: 0,   // 设备分配
        size: audio_data.len() as u32,
        time: 0,
        mod_date: now,
        bits: 0,
        file_type: TYPE_MP3,
        sample_rate: 0,
        bit_rate: 0,
        name: name_field,
        title,
        artist: id3_tags.artist,
        album: id3_tags.album,
    };

    (header, id3v2_size)
}

// ============================================================================
// 上传流程
// ============================================================================

/// 上传单个 MP3 文件到设备
///
/// 流程：读文件 → 跳过 ID3v2 → 存储空间预检 → 构造 header → upload_file
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `mem_unit`：目标内存单元（0=内置, 1=SD 卡）
/// - `path`：MP3 文件路径
/// - `text_opts`：文本处理选项（slug/strip），与 playlist 编码同步
/// - `progress`：进度回调（接收 `UploadProgress`）
///
/// # 返回
/// 设备分配的新 file_no
pub async fn upload_mp3<F>(
    device: &RioDevice,
    mem_unit: u8,
    path: &Path,
    text_opts: &UploadTextOptions,
    progress: F,
) -> Result<u32>
where
    F: Fn(UploadProgress),
{
    // 1. 提取文件名（在 move 之前）
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.mp3")
        .to_string();
    log::info!(
        "upload_mp3: start mem_unit={} path={} name={}",
        mem_unit,
        path.display(),
        file_name
    );

    // 2. 读取文件
    let path_owned = path.to_path_buf();
    let file_data = smol::unblock(move || std::fs::read(&path_owned))
        .await
        .map_err(|e| {
            log::error!("upload_mp3: read file failed: {}", e);
            CyrioError::Other(format!("读取文件失败: {}", e))
        })?;
    log::info!(
        "upload_mp3: file read {} bytes, building header",
        file_data.len()
    );

    // 3. 构造 header + 跳过 ID3v2
    let (header, id3v2_size) = build_upload_header(&file_data, &file_name, text_opts);
    let audio_data = &file_data[id3v2_size..];
    log::info!(
        "upload_mp3: header built id3v2_size={} audio_size={} title={:?} mod_date={}",
        id3v2_size,
        audio_data.len(),
        header.title,
        header.mod_date
    );

    // 4. 存储空间预检
    precheck_free_space(device, mem_unit, audio_data.len()).await?;
    log::info!("upload_mp3: precheck passed, starting USB upload");

    // 5. 上传（带进度回调）
    let file_no = device
        .upload_file(mem_unit, &header, audio_data, progress, None)
        .await?;
    log::info!(
        "upload_mp3: success file_no={} name={}",
        file_no, file_name
    );
    Ok(file_no)
}

/// 展开路径数组中的目录，递归收集所有 .mp3 文件
///
/// 用于拖拽上传：用户可能拖入文件或目录，此函数把目录展开成 .mp3 文件列表。
/// 文件直接保留（非 .mp3 文件会被过滤）。
pub fn expand_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for p in paths {
        if p.is_dir() {
            collect_mp3_recursive(&p, &mut result);
        } else if p.extension().and_then(|e| e.to_str()) == Some("mp3") {
            result.push(p);
        }
    }
    result
}

/// 递归收集目录下所有 .mp3 文件
fn collect_mp3_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_mp3_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("mp3") {
            out.push(path);
        }
    }
}

/// 批量上传多个 MP3 文件
///
/// 逐个上传，返回每个文件的结果。进度回调在每个文件上传期间触发。
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `mem_unit`：目标内存单元
/// - `paths`：MP3 文件路径列表
/// - `text_opts`：文本处理选项（slug/strip），与 playlist 编码同步
/// - `progress`：进度回调（接收 `UploadProgress`，每个文件上传期间触发）
///
/// # 返回
/// 每个文件的 `UploadResult`
pub async fn upload_mp3_batch<F>(
    device: &RioDevice,
    mem_unit: u8,
    paths: Vec<PathBuf>,
    text_opts: &UploadTextOptions,
    progress: F,
) -> Vec<UploadResult>
where
    F: Fn(UploadProgress),
{
    let mut results = Vec::with_capacity(paths.len());
    for path in paths {
        let result = upload_mp3(device, mem_unit, &path, text_opts, &progress).await;
        match result {
            Ok(file_no) => results.push(UploadResult {
                path,
                success: true,
                file_no: file_no as i64,
                error: String::new(),
            }),
            Err(e) => results.push(UploadResult {
                path,
                success: false,
                file_no: -1,
                error: e.to_string(),
            }),
        }
    }
    results
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_id3v2_size_returns_zero_for_no_tag() {
        let buf = [0u8; 100];
        assert_eq!(get_id3v2_size(&buf), 0);
    }

    #[test]
    fn get_id3v2_size_returns_zero_for_short_buffer() {
        let buf = [0u8; 5];
        assert_eq!(get_id3v2_size(&buf), 0);
    }

    #[test]
    fn get_id3v2_size_parses_valid_header() {
        let mut buf = vec![0u8; 110];
        buf[0..3].copy_from_slice(b"ID3");
        buf[3] = 3;
        buf[4] = 0;
        buf[5] = 0;
        buf[6] = 0;
        buf[7] = 0;
        buf[8] = 0;
        buf[9] = 100;
        assert_eq!(get_id3v2_size(&buf), 110);
    }

    #[test]
    fn read_id3_tags_returns_empty_for_no_tag() {
        let buf = [0u8; 100];
        let tags = read_id3_tags(&buf);
        assert!(tags.title.is_empty());
        assert!(tags.artist.is_empty());
        assert!(tags.album.is_empty());
    }

    #[test]
    fn read_id3_tags_parses_utf8_title() {
        let title = "测试歌曲";
        let title_bytes = title.as_bytes();
        let frame_size = 1 + title_bytes.len();
        let total_size = 10 + 10 + frame_size;

        let mut buf = vec![0u8; total_size];
        buf[0..3].copy_from_slice(b"ID3");
        buf[3] = 3;
        let body_size = total_size - 10;
        buf[6] = ((body_size >> 21) & 0x7f) as u8;
        buf[7] = ((body_size >> 14) & 0x7f) as u8;
        buf[8] = ((body_size >> 7) & 0x7f) as u8;
        buf[9] = (body_size & 0x7f) as u8;

        buf[10..14].copy_from_slice(b"TIT2");
        buf[14..18].copy_from_slice(&(frame_size as u32).to_be_bytes());
        buf[18..20].copy_from_slice(&[0, 0]);
        buf[20] = 3; // UTF-8
        buf[21..21 + title_bytes.len()].copy_from_slice(title_bytes);

        let tags = read_id3_tags(&buf);
        assert_eq!(tags.title, title);
    }

    #[test]
    fn decode_id3_text_handles_utf8() {
        let data = [3, b'H', b'i', 0];
        assert_eq!(decode_id3_text(&data), "Hi");
    }

    #[test]
    fn decode_id3_text_handles_empty() {
        assert_eq!(decode_id3_text(&[]), "");
    }

    #[test]
    fn build_upload_header_strips_id3v2() {
        // 构造 ID3v2 头 + 少量音频数据
        let title = "Test";
        let title_bytes = title.as_bytes();
        let frame_size = 1 + title_bytes.len();
        let id3_size = 10 + 10 + frame_size;
        let audio_size = 100;
        let mut buf = vec![0u8; id3_size + audio_size];
        buf[0..3].copy_from_slice(b"ID3");
        buf[3] = 3;
        let body_size = id3_size - 10;
        buf[6] = ((body_size >> 21) & 0x7f) as u8;
        buf[7] = ((body_size >> 14) & 0x7f) as u8;
        buf[8] = ((body_size >> 7) & 0x7f) as u8;
        buf[9] = (body_size & 0x7f) as u8;
        buf[10..14].copy_from_slice(b"TIT2");
        buf[14..18].copy_from_slice(&(frame_size as u32).to_be_bytes());
        buf[18..20].copy_from_slice(&[0, 0]);
        buf[20] = 3;
        buf[21..21 + title_bytes.len()].copy_from_slice(title_bytes);

        let (header, offset) = build_upload_header(&buf, "test.mp3", &UploadTextOptions::default());
        assert_eq!(offset, id3_size);
        assert_eq!(header.size as usize, audio_size);
        assert_eq!(header.title, "Test");
        assert_eq!(header.name, "D:\\test.mp3");
        assert_eq!(header.file_type, TYPE_MP3);
    }

    #[test]
    fn expand_paths_filters_non_mp3() {
        let tmp = std::env::temp_dir().join("cyrio_test_expand");
        std::fs::create_dir_all(&tmp).ok();
        std::fs::write(tmp.join("a.mp3"), b"data").ok();
        std::fs::write(tmp.join("b.txt"), b"data").ok();

        let paths = vec![tmp.clone()];
        let result = expand_paths(paths);
        assert_eq!(result.len(), 1);
        assert!(result[0].to_string_lossy().ends_with("a.mp3"));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
