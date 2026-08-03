//! FIDL/ST10 播放列表二进制格式
//!
//! 播放列表作为 `type = TYPE_PLS` 的文件存储在 Rio 设备上，文件内容是此二进制格式。
//!
//! ## 文件结构
//! - 12 字节头部：
//!   - 4B `"FIDL"` 魔数
//!   - 2B `"ST"` 子类型
//!   - 1B `version_major`（`0x01`）
//!   - 1B `version_minor`（`0x00`）
//!   - 1B `0x00`（保留）
//!   - 3B `nsongs`（小端 24 位）
//! - N 个 6 字节条目：3B `rio_num`（小端 24 位）+ 3B `sflags`
//!
//! ## 字段含义
//! - `rio_num`：文件在设备上的内部编号（`rio_file_t.file_no`）
//! - `sflags`：3 字节状态标志，rioutil 中读取但写入时通常全 0
//!
//! # 来源
//! 移植自 NodeJS 项目 `rio-rs/node/src/protocol/fidl.ts`。

use crate::error::{CyrioError, Result};
use crate::protocol::constants::{
    FIDL_ENTRY_SIZE, FIDL_HEADER_SIZE, FIDL_MAGIC, FIDL_OFF_RIO_NUM, FIDL_OFF_SFLAGS,
    FIDL_SUBTYPE, FIDL_VERSION_MAJOR, FIDL_VERSION_MINOR,
};

/// 单个播放列表条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FidlEntry {
    /// 3 字节小端：文件编号（`rio_file_t.file_no`）
    pub rio_num: u32,
    /// 3 字节状态标志（通常全 0）
    pub sflags: [u8; 3],
}

/// 整个播放列表（条目数组）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FidlPlaylist {
    /// 条目列表
    pub entries: Vec<FidlEntry>,
}

impl FidlPlaylist {
    /// 创建一个空播放列表
    pub fn empty() -> Self {
        Self::default()
    }
}

/// 解析 FIDL/ST10 二进制为 [`FidlPlaylist`]
///
/// 校验头部魔数 `"FIDL"` + `"ST"` + 版本号（必须是 `0x01 0x00`）+ 1B `0x00` +
/// 3B `nsongs`，然后循环读取 6 字节条目。
///
/// 若实际条目数少于头部声明，仅返回实际读到的条目（rioutil 行为）。
///
/// # 错误
/// - 缓冲区长度不足 [`FIDL_HEADER_SIZE`]：返回 [`CyrioError::Parse`]
/// - 魔数 / 子类型 / 版本号不匹配：返回 [`CyrioError::Parse`]
pub fn parse_fidl(buf: &[u8]) -> Result<FidlPlaylist> {
    if buf.len() < FIDL_HEADER_SIZE {
        return Err(CyrioError::Parse(format!(
            "parse_fidl: buffer too short for header (got {}, need {})",
            buf.len(),
            FIDL_HEADER_SIZE
        )));
    }

    // 校验魔数 "FIDL"
    if &buf[0..4] != FIDL_MAGIC {
        let got = String::from_utf8_lossy(&buf[0..4]);
        return Err(CyrioError::Parse(format!(
            "parse_fidl: bad magic, expected {:?}, got {:?}",
            String::from_utf8_lossy(FIDL_MAGIC),
            got
        )));
    }

    // 校验子类型 "ST"
    if &buf[4..6] != FIDL_SUBTYPE {
        let got = String::from_utf8_lossy(&buf[4..6]);
        return Err(CyrioError::Parse(format!(
            "parse_fidl: bad subtype, expected {:?}, got {:?}",
            String::from_utf8_lossy(FIDL_SUBTYPE),
            got
        )));
    }

    // 校验版本号
    let ver_major = buf[6];
    let ver_minor = buf[7];
    if ver_major != FIDL_VERSION_MAJOR || ver_minor != FIDL_VERSION_MINOR {
        return Err(CyrioError::Parse(format!(
            "parse_fidl: unsupported version {}.{}, expected {}.{}",
            ver_major, ver_minor, FIDL_VERSION_MAJOR, FIDL_VERSION_MINOR
        )));
    }

    // 1 字节 0x00（rioutil 中是 unk[8]，固定 0）
    // 3 字节 nsongs（小端 24 位）
    let nsongs = read_u24_le(buf, 9);

    let mut entries = Vec::with_capacity(nsongs as usize);
    for i in 0..nsongs {
        let offset = FIDL_HEADER_SIZE + (i as usize) * FIDL_ENTRY_SIZE;
        if offset + FIDL_ENTRY_SIZE > buf.len() {
            // rioutil 行为：实际条目少于声明时 warning，但不报错
            break;
        }
        let rio_num = read_u24_le(buf, offset + FIDL_OFF_RIO_NUM);
        let mut sflags = [0u8; 3];
        sflags.copy_from_slice(&buf[offset + FIDL_OFF_SFLAGS..offset + FIDL_OFF_SFLAGS + 3]);
        entries.push(FidlEntry { rio_num, sflags });
    }

    Ok(FidlPlaylist { entries })
}

/// 将 [`FidlPlaylist`] 序列化为 FIDL/ST10 二进制
///
/// 返回 `12 字节头 + N×6 字节条目` 的 `Vec<u8>`。
pub fn serialize_fidl(playlist: &FidlPlaylist) -> Vec<u8> {
    let nsongs = playlist.entries.len();
    let mut buf = vec![0u8; FIDL_HEADER_SIZE + nsongs * FIDL_ENTRY_SIZE];

    // 写头部
    buf[0..4].copy_from_slice(FIDL_MAGIC);
    buf[4..6].copy_from_slice(FIDL_SUBTYPE);
    buf[6] = FIDL_VERSION_MAJOR;
    buf[7] = FIDL_VERSION_MINOR;
    buf[8] = 0x00;
    write_u24_le(&mut buf, 9, nsongs as u32);

    // 写条目
    for (i, entry) in playlist.entries.iter().enumerate() {
        let offset = FIDL_HEADER_SIZE + i * FIDL_ENTRY_SIZE;
        write_u24_le(&mut buf, offset + FIDL_OFF_RIO_NUM, entry.rio_num);
        buf[offset + FIDL_OFF_SFLAGS..offset + FIDL_OFF_SFLAGS + 3]
            .copy_from_slice(&entry.sflags);
    }

    buf
}

/// 便捷方法：解析已有 FIDL → 追加一条 → 重新序列化
///
/// 用于 `add_to_playlist` API：从设备下载歌单 → 追加歌曲条目 → 覆盖回设备。
///
/// # 参数
/// - `buf`：原始 FIDL 二进制
/// - `rio_num`：要追加的歌曲文件号
/// - `sflags`：状态标志（3 字节，默认全 0）
pub fn append_to_fidl(buf: &[u8], rio_num: u32, sflags: [u8; 3]) -> Result<Vec<u8>> {
    let mut playlist = parse_fidl(buf)?;
    playlist.entries.push(FidlEntry { rio_num, sflags });
    Ok(serialize_fidl(&playlist))
}

// ============================================================================
// 内部工具：3 字节小端整数读写（FIDL 条目专用）
// ============================================================================

/// 读取小端 24 位无符号整数（3 字节）
fn read_u24_le(buf: &[u8], offset: usize) -> u32 {
    (buf[offset] as u32) | ((buf[offset + 1] as u32) << 8) | ((buf[offset + 2] as u32) << 16)
}

/// 写入小端 24 位无符号整数（3 字节，仅低 24 位有效）
fn write_u24_le(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset] = (value & 0xff) as u8;
    buf[offset + 1] = ((value >> 8) & 0xff) as u8;
    buf[offset + 2] = ((value >> 16) & 0xff) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_playlist_serializes_to_header_only() {
        let p = FidlPlaylist::empty();
        let buf = serialize_fidl(&p);
        assert_eq!(buf.len(), FIDL_HEADER_SIZE);
        // 头部字段
        assert_eq!(&buf[0..4], b"FIDL");
        assert_eq!(&buf[4..6], b"ST");
        assert_eq!(buf[6], 0x01);
        assert_eq!(buf[7], 0x00);
        assert_eq!(buf[8], 0x00);
        // nsongs = 0
        assert_eq!(read_u24_le(&buf, 9), 0);
    }

    #[test]
    fn parse_empty_buffer_returns_error() {
        let buf = [];
        let err = parse_fidl(&buf).unwrap_err();
        assert!(matches!(err, CyrioError::Parse(_)));
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn parse_bad_magic_returns_error() {
        let mut buf = serialize_fidl(&FidlPlaylist::empty());
        buf[0] = b'X';
        let err = parse_fidl(&buf).unwrap_err();
        assert!(err.to_string().contains("bad magic"));
    }

    #[test]
    fn parse_bad_subtype_returns_error() {
        let mut buf = serialize_fidl(&FidlPlaylist::empty());
        buf[4] = b'X';
        let err = parse_fidl(&buf).unwrap_err();
        assert!(err.to_string().contains("bad subtype"));
    }

    #[test]
    fn parse_bad_version_returns_error() {
        let mut buf = serialize_fidl(&FidlPlaylist::empty());
        buf[6] = 0x02; // major version 不匹配
        let err = parse_fidl(&buf).unwrap_err();
        assert!(err.to_string().contains("unsupported version"));
    }

    #[test]
    fn round_trip_with_entries() {
        let mut p = FidlPlaylist::empty();
        p.entries.push(FidlEntry {
            rio_num: 0x4090,
            sflags: [0, 0, 0],
        });
        p.entries.push(FidlEntry {
            rio_num: 0x40a0,
            sflags: [1, 0, 0],
        });
        p.entries.push(FidlEntry {
            rio_num: 0x40b0,
            sflags: [0xff, 0xee, 0xdd],
        });

        let buf = serialize_fidl(&p);
        assert_eq!(buf.len(), FIDL_HEADER_SIZE + 3 * FIDL_ENTRY_SIZE);

        let parsed = parse_fidl(&buf).unwrap();
        assert_eq!(parsed, p);
    }

    #[test]
    fn nsongs_field_matches_entries_len() {
        let mut p = FidlPlaylist::empty();
        for i in 0..5 {
            p.entries.push(FidlEntry {
                rio_num: 0x4000 + i,
                sflags: [0; 3],
            });
        }
        let buf = serialize_fidl(&p);
        assert_eq!(read_u24_le(&buf, 9), 5);
    }

    #[test]
    fn parse_handles_truncated_entries() {
        // 头部声明 5 首歌，但只提供 2 个条目的数据
        let mut buf = vec![0u8; FIDL_HEADER_SIZE + 2 * FIDL_ENTRY_SIZE];
        buf[0..4].copy_from_slice(b"FIDL");
        buf[4..6].copy_from_slice(b"ST");
        buf[6] = 0x01;
        buf[7] = 0x00;
        buf[8] = 0x00;
        write_u24_le(&mut buf, 9, 5); // 声明 5 首

        // 写 2 个条目
        write_u24_le(&mut buf, FIDL_HEADER_SIZE + FIDL_OFF_RIO_NUM, 0x4001);
        write_u24_le(
            &mut buf,
            FIDL_HEADER_SIZE + FIDL_ENTRY_SIZE + FIDL_OFF_RIO_NUM,
            0x4002,
        );

        let parsed = parse_fidl(&buf).unwrap();
        // rioutil 行为：只返回实际能读到的条目
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].rio_num, 0x4001);
        assert_eq!(parsed.entries[1].rio_num, 0x4002);
    }

    #[test]
    fn append_to_fidl_adds_entry() {
        let mut p = FidlPlaylist::empty();
        p.entries.push(FidlEntry {
            rio_num: 0x4001,
            sflags: [0; 3],
        });
        let original = serialize_fidl(&p);

        let appended = append_to_fidl(&original, 0x4002, [0; 3]).unwrap();
        let parsed = parse_fidl(&appended).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].rio_num, 0x4001);
        assert_eq!(parsed.entries[1].rio_num, 0x4002);
    }

    #[test]
    fn read_write_u24_le_round_trip() {
        let mut buf = [0u8; 4];
        write_u24_le(&mut buf, 0, 0x123456);
        assert_eq!(buf[0], 0x56);
        assert_eq!(buf[1], 0x34);
        assert_eq!(buf[2], 0x12);
        assert_eq!(read_u24_le(&buf, 0), 0x123456);
    }

    #[test]
    fn u24_handles_max_value() {
        let mut buf = [0u8; 3];
        write_u24_le(&mut buf, 0, 0xFFFFFF);
        assert_eq!(buf, [0xff, 0xff, 0xff]);
        assert_eq!(read_u24_le(&buf, 0), 0xFFFFFF);
    }

    #[test]
    fn u24_truncates_high_byte() {
        let mut buf = [0u8; 4];
        write_u24_le(&mut buf, 0, 0x01020304);
        // 只写低 24 位
        assert_eq!(buf[0], 0x04);
        assert_eq!(buf[1], 0x03);
        assert_eq!(buf[2], 0x02);
        assert_eq!(buf[3], 0x00); // 未写入
    }
}
