//! 用户面向类型 + 通用辅助函数
//!
//! 把 [`crate::protocol::rio_file::RioFile`] 转为更易用的 [`Song`] / [`Playlist`]，
//! 并提供写入操作共用的 [`precheck_free_space`] 存储空间预检。
//!
//! 字符串编码：rio_file_t 的 name/title/artist/album 是 64 字节 UTF-8 + NUL 填充，
//! [`crate::protocol::rio_file::parse_rio_file`] 已用 UTF-8 解码，无需额外转换。
//!
//! # 来源
//! 移植自 NodeJS `rio-rs/node/src/api/device.ts` 的辅助函数部分。

use crate::api::device::RioDevice;
use crate::error::{CyrioError, Result};
use crate::protocol::constants::{RIO_FILE_SIZE, TYPE_MP3, TYPE_PLS};
use crate::protocol::rio_file::RioFile;
use crate::protocol::rio_mem::RioMem;

/// 歌曲信息（用户面向）
///
/// 由 [`crate::protocol::rio_file::RioFile`] 转换而来，
/// `bit_rate` 已从 `kbps << 7` 转换为 kbps。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    /// 文件号（设备内部的唯一编号，用于下载/删除/加入歌单）
    pub file_no: u32,
    /// 文件大小（字节）
    pub size: u32,
    /// 时长（秒）
    pub time: u32,
    /// 比特率（kbps，已从 `<< 7` 转换）
    pub bit_rate: u32,
    /// 采样率（Hz）
    pub sample_rate: u32,
    /// 文件名（UTF-8）
    pub name: String,
    /// 标题（UTF-8）
    pub title: String,
    /// 艺术家（UTF-8）
    pub artist: String,
    /// 专辑（UTF-8）
    pub album: String,
}

/// 播放列表信息（用户面向）
///
/// `file_type == TYPE_PLS` 的 [`RioFile`] 即为歌单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    /// 文件号
    pub file_no: u32,
    /// 文件大小（字节，FIDL 二进制长度）
    pub size: u32,
    /// 歌单名（UTF-8）
    pub name: String,
    /// 标题（UTF-8）
    pub title: String,
}

/// 将 [`RioFile`] 转为 [`Song`]
///
/// `bit_rate` 从 `kbps << 7` 转换为 kbps。
pub fn rio_file_to_song(file: &RioFile) -> Song {
    Song {
        file_no: file.file_no,
        size: file.size,
        time: file.time,
        bit_rate: file.bit_rate >> 7, // kbps << 7 → kbps
        sample_rate: file.sample_rate,
        name: file.name.clone(),
        title: file.title.clone(),
        artist: file.artist.clone(),
        album: file.album.clone(),
    }
}

/// 将 [`RioFile`] 转为 [`Playlist`]（仅当 `file_type == TYPE_PLS`）
pub fn rio_file_to_playlist(file: &RioFile) -> Playlist {
    Playlist {
        file_no: file.file_no,
        size: file.size,
        name: file.name.clone(),
        title: file.title.clone(),
    }
}

/// 判断 [`RioFile`] 是否为 MP3 文件
pub fn is_mp3_file(file: &RioFile) -> bool {
    file.file_type == TYPE_MP3
}

/// 判断 [`RioFile`] 是否为播放列表
pub fn is_playlist_file(file: &RioFile) -> bool {
    file.file_type == TYPE_PLS
}

/// 写入前存储空间预检（PROTOCOL.md §16.6，所有写入操作必调）
///
/// 检查目标内存单元是否存在 + 空闲空间是否足够。
/// 需要的空闲空间 = 音频字节数 + rio_file_t 头（2048B）。
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `mem_unit`：内存单元（0=内置, 1=SD 卡）
/// - `audio_bytes`：要写入的音频数据字节数（不含 2048B 头）
///
/// # 返回
/// 内存单元信息（供调用方进一步使用）
///
/// # 错误
/// - 内存单元不存在（如 SD 卡未插入）
/// - 空间不足
pub async fn precheck_free_space(
    device: &RioDevice,
    mem_unit: u8,
    audio_bytes: usize,
) -> Result<RioMem> {
    let mem = device.get_memory_info(mem_unit).await?;
    if !mem.is_present() {
        return Err(CyrioError::Device(format!(
            "内存单元 {} 不存在{}",
            mem_unit,
            if mem_unit == 1 { "（SD 卡未插入？）" } else { "" }
        )));
    }
    // 需要：音频数据 + rio_file_t 头（2048B）
    let required = audio_bytes + RIO_FILE_SIZE;
    if (mem.free as usize) < required {
        return Err(CyrioError::Device(format!(
            "存储空间不足：需要 {} 字节（{}），仅有 {} 字节空闲（{}）。{}",
            required,
            format_bytes(required),
            mem.free,
            format_bytes(mem.free as usize),
            mem.format_size()
        )));
    }
    Ok(mem)
}

/// 字节数 → 人类可读字符串
pub fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        return format!("{}B", bytes);
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1}KB", bytes as f64 / 1024.0);
    }
    format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::TYPE_MP3;
    use crate::protocol::rio_file::RioFile;

    #[test]
    fn rio_file_to_song_converts_bit_rate() {
        let mut f = RioFile::empty();
        f.file_no = 48;
        f.size = 1024;
        f.bit_rate = 16384; // 128kbps << 7
        f.name = "test.mp3".to_string();
        let s = rio_file_to_song(&f);
        assert_eq!(s.file_no, 48);
        assert_eq!(s.bit_rate, 128);
        assert_eq!(s.name, "test.mp3");
    }

    #[test]
    fn is_mp3_file_detects_type() {
        let mut f = RioFile::empty();
        f.file_type = TYPE_MP3;
        assert!(is_mp3_file(&f));
        f.file_type = 0;
        assert!(!is_mp3_file(&f));
    }

    #[test]
    fn format_bytes_thresholds() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(1023), "1023B");
        assert_eq!(format_bytes(1024), "1.0KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0MB");
        assert_eq!(format_bytes(1024 * 1024 * 5), "5.0MB");
    }
}
