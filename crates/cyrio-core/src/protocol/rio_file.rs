//! rio_file_t 结构体序列化（2048 字节）
//!
//! 每个文件在 Rio 设备上由一个 2048 字节的头描述，包含 `file_no`、`size`、`type`、
//! 比特率、采样率、`name`/`title`/`artist`/`album` 等字段。
//!
//! ## 字段布局
//! 字段偏移量定义见 [`super::constants`] 的 `OFF_*` 常量。所有数值字段为小端 u32，
//! 字符串字段为 UTF-8 + NUL 填充。
//!
//! ## 关键约束
//! - `bits` 字段：所有文件类型都只置位 `0x110`（**不设 bit 0**），否则设备固件
//!   会对 name/title 做 latin1→UTF-8 双重编码，导致软件端和设备端均显示乱码。
//!   真机实测：MP3 和 PLS 都受 bit 0 影响，统一清除。
//! - `bit_rate` 字段单位是 `kbps << 7`（128kbps 存为 16384），由调用方换算。
//! - 上传时 `start` 字段设 0，由设备分配闪存偏移。
//! - `0x1c-0x23`、`0x2c-0xbf` 等未定义区域保留 0，反序列化时忽略。
//! - 字符串字段用 UTF-8 + NUL 填充（与原版 Windows 软件行为一致）。
//!
//! ## S-Series 播放列表兼容性
//! 修改播放列表时必须保留原始 2048B 头中的未知字段（如 `unk1[4]`），否则设备
//! 无法识别。协议层 [`parse_rio_file`] 只解析已知字段，调用方（API 层）需自行
//! 保留原始 `header_buffer`，覆盖时通过 [`overwrite_rio_file_fields`] 重用。
//!
//! # 来源
//! 移植自 NodeJS 项目 `rio-rs/node/src/protocol/rioFile.ts`。

use crate::error::{CyrioError, Result};
use crate::protocol::constants::{
    BITS_REQUIRED, OFF_ALBUM, OFF_ARTIST, OFF_BIT_RATE, OFF_BITS, OFF_FILE_NO, OFF_MOD_DATE,
    OFF_NAME, OFF_SAMPLE_RATE, OFF_SIZE, OFF_START, OFF_TIME, OFF_TITLE, OFF_TYPE, RIO_FILE_SIZE,
    RIO_STRING_LEN,
};

/// Rio 文件元数据（对应 2048 字节的 rio_file_t 结构）
///
/// 字段含义详见 `docs/PROTOCOL.md` §11。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RioFile {
    /// u32 @ 0x00：文件编号（1-based，由设备分配）
    pub file_no: u32,
    /// u32 @ 0x04：文件在闪存中的起始偏移（上传时设 0）
    pub start: u32,
    /// u32 @ 0x08：文件大小（字节数）
    pub size: u32,
    /// u32 @ 0x0c：时长（秒）
    pub time: u32,
    /// u32 @ 0x10：修改时间（Unix 时间戳，秒）
    pub mod_date: u32,
    /// u32 @ 0x14：标志位（序列化时强制 |= (BITS_REQUIRED & !0x01)，统一清 bit 0）
    pub bits: u32,
    /// u32 @ 0x18：文件类型 FourCC（TYPE_MP3 / TYPE_PLS 等）
    pub file_type: u32,
    /// u32 @ 0x24：采样率（Hz，如 44100）
    pub sample_rate: u32,
    /// u32 @ 0x28：比特率（kbps << 7，128kbps 存 16384）
    pub bit_rate: u32,
    /// char[64] @ 0xc0：文件名（UTF-8 + NUL）
    pub name: String,
    /// char[64] @ 0x100：标题
    pub title: String,
    /// char[64] @ 0x140：艺术家
    pub artist: String,
    /// char[64] @ 0x180：专辑
    pub album: String,
}

impl RioFile {
    /// 创建一个全 0 的空 `RioFile` 模板
    ///
    /// 用于上传前由调用方填充字段。
    pub fn empty() -> Self {
        Self::default()
    }
}

/// 将 [`RioFile`] 序列化为 2048 字节数组
///
/// 强制置位 bits 必须位：所有文件类型都用 `BITS_REQUIRED & !0x01`（`0x110`），
/// 统一清除 bit 0，避免设备固件对 name/title 做 latin1→UTF-8 双重编码。
/// 字符串字段用 UTF-8 + NUL 填充，超长截断。
/// 未定义区域填 0。
pub fn serialize_rio_file(file: &RioFile) -> [u8; RIO_FILE_SIZE] {
    let mut buf = [0u8; RIO_FILE_SIZE];

    write_u32_le(&mut buf, OFF_FILE_NO, file.file_no);
    write_u32_le(&mut buf, OFF_START, file.start);
    write_u32_le(&mut buf, OFF_SIZE, file.size);
    write_u32_le(&mut buf, OFF_TIME, file.time);
    write_u32_le(&mut buf, OFF_MOD_DATE, file.mod_date);

    // 所有文件类型都不设 bit 0：设备固件在 bit 0=1 时会对 name/title 做
    // latin1→UTF-8 双重编码，导致软件端读到 "æµè¯æ­å" 等乱码，设备端也显示 latin1 字符。
    // 真机实测：MP3 和 PLS 都受 bit 0 影响，原版软件/NodeJS 创建的文件均 bit 0=0。
    let required = BITS_REQUIRED & !0x01;
    let bits = file.bits | required;
    write_u32_le(&mut buf, OFF_BITS, bits);

    write_u32_le(&mut buf, OFF_TYPE, file.file_type);
    write_u32_le(&mut buf, OFF_SAMPLE_RATE, file.sample_rate);
    write_u32_le(&mut buf, OFF_BIT_RATE, file.bit_rate);

    write_fixed_string(&mut buf, OFF_NAME, RIO_STRING_LEN, &file.name);
    write_fixed_string(&mut buf, OFF_TITLE, RIO_STRING_LEN, &file.title);
    write_fixed_string(&mut buf, OFF_ARTIST, RIO_STRING_LEN, &file.artist);
    write_fixed_string(&mut buf, OFF_ALBUM, RIO_STRING_LEN, &file.album);

    buf
}

/// 从 2048 字节切片反序列化为 [`RioFile`]
///
/// 忽略所有未定义区域（保留字节）。
/// 字符串字段用 UTF-8 解码，自动去除尾部 NUL。
///
/// # 错误
/// 缓冲区长度不足 [`RIO_FILE_SIZE`] 时返回 [`CyrioError::Parse`]。
pub fn parse_rio_file(buf: &[u8]) -> Result<RioFile> {
    if buf.len() < RIO_FILE_SIZE {
        return Err(CyrioError::Parse(format!(
            "parse_rio_file: buffer too short (got {}, need {})",
            buf.len(),
            RIO_FILE_SIZE
        )));
    }

    Ok(RioFile {
        file_no: read_u32_le(buf, OFF_FILE_NO),
        start: read_u32_le(buf, OFF_START),
        size: read_u32_le(buf, OFF_SIZE),
        time: read_u32_le(buf, OFF_TIME),
        mod_date: read_u32_le(buf, OFF_MOD_DATE),
        bits: read_u32_le(buf, OFF_BITS),
        file_type: read_u32_le(buf, OFF_TYPE),
        sample_rate: read_u32_le(buf, OFF_SAMPLE_RATE),
        bit_rate: read_u32_le(buf, OFF_BIT_RATE),
        name: read_fixed_string(buf, OFF_NAME, RIO_STRING_LEN),
        title: read_fixed_string(buf, OFF_TITLE, RIO_STRING_LEN),
        artist: read_fixed_string(buf, OFF_ARTIST, RIO_STRING_LEN),
        album: read_fixed_string(buf, OFF_ALBUM, RIO_STRING_LEN),
    })
}

/// 在原始 header_buffer 上覆盖特定字段，保留未知字段
///
/// 用于 S-Series 播放列表修改场景：从设备读取 2048B 原始头 → 修改 `file_no` /
/// `size` / `mod_date` 等已知字段 → 写回设备。保留 `unk1[4]` 等未知字段，
/// 防止设备无法识别播放列表。
///
/// # 参数
/// - `header_buffer`：从设备读取的原始 2048B 头（会被修改）
/// - `updates`：要覆盖的字段值
pub fn overwrite_rio_file_fields(
    header_buffer: &mut [u8; RIO_FILE_SIZE],
    updates: &RioFileUpdates,
) {
    if let Some(file_no) = updates.file_no {
        write_u32_le(header_buffer, OFF_FILE_NO, file_no);
    }
    if let Some(start) = updates.start {
        write_u32_le(header_buffer, OFF_START, start);
    }
    if let Some(size) = updates.size {
        write_u32_le(header_buffer, OFF_SIZE, size);
    }
    if let Some(time) = updates.time {
        write_u32_le(header_buffer, OFF_TIME, time);
    }
    if let Some(mod_date) = updates.mod_date {
        write_u32_le(header_buffer, OFF_MOD_DATE, mod_date);
    }
    if let Some(bits) = updates.bits {
        // 所有文件类型统一清除 bit 0：设备固件在 bit 0=1 时对 name/title 做
        // latin1→UTF-8 双重编码，导致乱码。即使原始 bits 有 bit 0=1（旧文件），
        // 覆盖时也要修正。真机实测 MP3 和 PLS 都受影响。
        let required = BITS_REQUIRED & !0x01;
        let final_bits = (bits & !0x01) | required;
        write_u32_le(header_buffer, OFF_BITS, final_bits);
    }
    if let Some(file_type) = updates.file_type {
        write_u32_le(header_buffer, OFF_TYPE, file_type);
    }
    if let Some(sample_rate) = updates.sample_rate {
        write_u32_le(header_buffer, OFF_SAMPLE_RATE, sample_rate);
    }
    if let Some(bit_rate) = updates.bit_rate {
        write_u32_le(header_buffer, OFF_BIT_RATE, bit_rate);
    }
    if let Some(name) = &updates.name {
        write_fixed_string(header_buffer, OFF_NAME, RIO_STRING_LEN, name);
    }
    if let Some(title) = &updates.title {
        write_fixed_string(header_buffer, OFF_TITLE, RIO_STRING_LEN, title);
    }
    if let Some(artist) = &updates.artist {
        write_fixed_string(header_buffer, OFF_ARTIST, RIO_STRING_LEN, artist);
    }
    if let Some(album) = &updates.album {
        write_fixed_string(header_buffer, OFF_ALBUM, RIO_STRING_LEN, album);
    }
}

/// [`overwrite_rio_file_fields`] 使用的部分更新结构
///
/// 所有字段为 `Option`，`None` 表示保留原值。
#[derive(Debug, Default, Clone)]
pub struct RioFileUpdates {
    /// 文件编号
    pub file_no: Option<u32>,
    /// 起始偏移
    pub start: Option<u32>,
    /// 文件大小
    pub size: Option<u32>,
    /// 时长（秒）
    pub time: Option<u32>,
    /// 修改时间
    pub mod_date: Option<u32>,
    /// 标志位（写入时仍会 |= (BITS_REQUIRED & !0x01)，统一清 bit 0）
    pub bits: Option<u32>,
    /// 文件类型
    pub file_type: Option<u32>,
    /// 采样率
    pub sample_rate: Option<u32>,
    /// 比特率
    pub bit_rate: Option<u32>,
    /// 文件名
    pub name: Option<String>,
    /// 标题
    pub title: Option<String>,
    /// 艺术家
    pub artist: Option<String>,
    /// 专辑
    pub album: Option<String>,
}

// ============================================================================
// 内部工具函数（与 nodejs src/util/buffer.ts 等价）
// ============================================================================

/// 读取小端 u32
fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

/// 写入小端 u32
fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

/// 读取定长字符串字段，智能检测编码
///
/// 正常情况（PLS bit 0=0 或 MP3 文件）：设备原样返回 UTF-8 字节，直接解码即可。
///
/// 异常情况（PLS bit 0=1 的旧歌单）：设备固件把 name/title 等字段的字节按 latin1
/// 解释后重新编码为 UTF-8 返回（双重编码）。此时 UTF-8 解码成功但字符串只含
/// latin1 范围字符（U+0080-U+00FF），需要转回 latin1 字节再用 GBK/UTF-8 解码。
///
/// 解码顺序：
/// 1. UTF-8 → 若成功且含非 latin1 字符（如中文），直接返回（正常 UTF-8）
/// 2. UTF-8 成功但只含 latin1 范围字符 → 双重编码检测：
///    转回 latin1 字节后尝试 GBK 解码（恢复原始中文）
/// 3. 原始字节 GBK 解码（未经设备双重编码的原始 GBK 字节）
/// 4. latin1 fallback（纯字节 → 码点，保证不丢失数据）
fn read_fixed_string(buf: &[u8], offset: usize, max_len: usize) -> String {
    let slice = &buf[offset..offset + max_len];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(max_len);
    let bytes = &slice[..end];
    if bytes.is_empty() {
        return String::new();
    }
    // 1. 尝试 UTF-8（正常情况：原版软件/我们写入的 UTF-8）
    if let Ok(s) = std::str::from_utf8(bytes) {
        // 检测双重编码：旧 PLS（bit 0=1）的 UTF-8 字节被设备按 latin1→UTF-8 双重编码。
        // 表现为 UTF-8 解码成功，但字符串只含 latin1 范围字符（U+0080-U+00FF）。
        if s.bytes().any(|b| b >= 0x80) {
            let all_latin1 = s.chars().all(|c| (c as u32) <= 0xFF);
            if all_latin1 {
                let latin1_bytes: Vec<u8> = s.chars().map(|c| c as u8).collect();
                // 双重编码的原始字节可能是 UTF-8 或 GBK，先试 UTF-8 再试 GBK
                if let Ok(recovered) = std::str::from_utf8(&latin1_bytes) {
                    return recovered.to_string();
                }
                let (cow, had_errors) =
                    encoding_rs::GBK.decode_without_bom_handling(&latin1_bytes);
                if !had_errors {
                    return cow.into_owned();
                }
            }
        }
        return s.to_string();
    }
    // 2. 尝试 GBK（原厂中文 Windows 软件上传的中文是 GBK）
    let (cow, had_errors) = encoding_rs::GBK.decode_without_bom_handling(bytes);
    if !had_errors {
        return cow.into_owned();
    }
    // 3. latin1 fallback（纯字节 → 码点，保证不丢失数据）
    bytes.iter().map(|&b| b as char).collect()
}

/// 写入 UTF-8 + NUL 填充的定长字符串
///
/// 原版 Windows Rio 软件和 NodeJS cyrio 均用 UTF-8 编码写入 name/title 等字段，
/// 设备固件原样存储（PLS 文件需 bit 0=0，见 [`serialize_rio_file`]）。
/// 超长截断（按字节，不按字符）。先清零目标区域，再写入 UTF-8 字节。
fn write_fixed_string(buf: &mut [u8], offset: usize, max_len: usize, value: &str) {
    // 清零目标区域
    for b in &mut buf[offset..offset + max_len] {
        *b = 0;
    }
    // UTF-8 编码写入（Rust String 内部即 UTF-8）
    let bytes = value.as_bytes();
    let take = bytes.len().min(max_len);
    buf[offset..offset + take].copy_from_slice(&bytes[..take]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::{TYPE_MP3, TYPE_PLS};

    #[test]
    fn empty_rio_file_is_all_zero() {
        let f = RioFile::empty();
        assert_eq!(f.file_no, 0);
        assert_eq!(f.size, 0);
        assert_eq!(f.name, "");
    }

    #[test]
    fn serialize_writes_known_fields_le() {
        let mut f = RioFile::empty();
        f.file_no = 48;
        f.size = 1024;
        f.mod_date = 0x12345678;
        f.file_type = TYPE_MP3;
        f.sample_rate = 44100;
        f.bit_rate = 16384; // 128kbps << 7
        f.name = "test.mp3".to_string();

        let buf = serialize_rio_file(&f);

        assert_eq!(read_u32_le(&buf, OFF_FILE_NO), 48);
        assert_eq!(read_u32_le(&buf, OFF_SIZE), 1024);
        assert_eq!(read_u32_le(&buf, OFF_MOD_DATE), 0x12345678);
        assert_eq!(read_u32_le(&buf, OFF_TYPE), TYPE_MP3);
        assert_eq!(read_u32_le(&buf, OFF_SAMPLE_RATE), 44100);
        assert_eq!(read_u32_le(&buf, OFF_BIT_RATE), 16384);

        // 字符串：UTF-8 + NUL
        assert_eq!(&buf[OFF_NAME..OFF_NAME + 8], b"test.mp3");
        assert_eq!(buf[OFF_NAME + 8], 0); // NUL 填充
    }

    #[test]
    fn serialize_forces_bits_required() {
        // 所有文件类型：强制 |= (BITS_REQUIRED & !0x01)，统一清 bit 0
        let mut f = RioFile::empty();
        f.bits = 0;
        let buf = serialize_rio_file(&f);
        assert_eq!(read_u32_le(&buf, OFF_BITS), BITS_REQUIRED & !0x01);

        // 已置位的位不应丢失（bit 0 会被清掉）
        f.bits = 0x80; // BITS_DOWNLOADABLE
        let buf = serialize_rio_file(&f);
        assert_eq!(read_u32_le(&buf, OFF_BITS), 0x80 | (BITS_REQUIRED & !0x01));
    }

    #[test]
    fn serialize_never_sets_bit0() {
        // 所有文件类型都不设 bit 0：设备固件在 bit 0=1 时对 name/title 做 latin1→UTF-8
        // 双重编码，导致乱码。真机实测 MP3 和 PLS 都受影响，统一清 bit 0。
        let mut f = RioFile::empty();
        f.file_type = TYPE_PLS;
        f.bits = 0;
        f.name = "测试歌单".to_string();
        let buf = serialize_rio_file(&f);

        let bits = read_u32_le(&buf, OFF_BITS);
        assert_eq!(bits, BITS_REQUIRED & !0x01, "PLS should not set bit 0");
        assert_eq!(bits & 0x01, 0, "bit 0 must be 0 for PLS");
        // 其他必需位（0x10、0x100）仍要置位
        assert_eq!(bits & 0x110, 0x110);

        // MP3 文件也不设 bit 0（与 PLS 一致）
        let mut mp3 = RioFile::empty();
        mp3.file_type = TYPE_MP3;
        mp3.bits = 0;
        let mp3_buf = serialize_rio_file(&mp3);
        let mp3_bits = read_u32_le(&mp3_buf, OFF_BITS);
        assert_eq!(
            mp3_bits,
            BITS_REQUIRED & !0x01,
            "MP3 should also clear bit 0 (unified encoding)"
        );
        assert_eq!(mp3_bits & 0x01, 0, "bit 0 must be 0 for MP3");
        assert_eq!(mp3_bits & 0x110, 0x110, "other required bits still set");
    }

    #[test]
    fn parse_reads_known_fields() {
        let mut f = RioFile::empty();
        f.file_no = 144;
        f.size = 4096;
        f.file_type = TYPE_MP3;
        f.name = "song.mp3".to_string();
        f.artist = "Artist".to_string();

        let buf = serialize_rio_file(&f);
        let parsed = parse_rio_file(&buf).unwrap();

        assert_eq!(parsed.file_no, 144);
        assert_eq!(parsed.size, 4096);
        assert_eq!(parsed.file_type, TYPE_MP3);
        assert_eq!(parsed.name, "song.mp3");
        assert_eq!(parsed.artist, "Artist");
    }

    #[test]
    fn parse_returns_error_for_short_buffer() {
        let buf = [0u8; 100];
        let err = parse_rio_file(&buf).unwrap_err();
        assert!(matches!(err, CyrioError::Parse(_)));
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn round_trip_preserves_all_fields() {
        let mut f = RioFile::empty();
        f.file_no = 224;
        f.start = 0x10000;
        f.size = 0xabcdef;
        f.time = 180;
        f.mod_date = 0x17000000;
        f.bits = 0x110; // bit 0 会被清除，用 0x110 才能 round-trip
        f.file_type = TYPE_MP3;
        f.sample_rate = 48000;
        f.bit_rate = 32000; // ~250kbps
        // 仅用 latin1 范围内的字符（round-trip 才能保持一致）
        f.name = "song.mp3".to_string();
        f.title = "Title".to_string();
        f.artist = "Artist".to_string();
        f.album = "Album".to_string();

        let buf = serialize_rio_file(&f);
        let parsed = parse_rio_file(&buf).unwrap();

        assert_eq!(parsed, f);
    }

    #[test]
    fn utf8_encoding_handles_chinese() {
        // 中文字符用 UTF-8 编码写入（与原版 Windows 软件行为一致）
        let mut f = RioFile::empty();
        f.title = "测试".to_string();

        let buf = serialize_rio_file(&f);
        // "测试" 的 UTF-8 编码: E6 B5 8B E8 AF 95
        assert_eq!(buf[OFF_TITLE], 0xE6);
        assert_eq!(buf[OFF_TITLE + 1], 0xB5);
        assert_eq!(buf[OFF_TITLE + 2], 0x8B);
        assert_eq!(buf[OFF_TITLE + 3], 0xE8);
        assert_eq!(buf[OFF_TITLE + 4], 0xAF);
        assert_eq!(buf[OFF_TITLE + 5], 0x95);
        // UTF-8 字节直接 parse 即可正确解码
        let parsed = parse_rio_file(&buf).unwrap();
        assert_eq!(parsed.title, "测试");
    }

    #[test]
    fn utf8_roundtrip_playlist_name_chinese() {
        // 模拟 create_playlist 写入 "测试歌单" → 设备原样返回 → list_playlists 读取的完整往返
        // PLS 不设 bit 0，设备原样返回 UTF-8 字节，无需双重编码检测
        let mut f = RioFile::empty();
        f.file_type = TYPE_PLS;
        f.name = "测试歌单".to_string();
        f.title = "测试歌单".to_string();

        let buf = serialize_rio_file(&f);
        // 验证写入的是 UTF-8 字节
        // "测试歌单" 的 UTF-8 编码: E6 B5 8B E8 AF 95 E6 AD 8C E5 8D 95
        assert_eq!(
            &buf[OFF_NAME..OFF_NAME + 12],
            &[
                0xE6, 0xB5, 0x8B, 0xE8, 0xAF, 0x95, 0xE6, 0xAD, 0x8C, 0xE5, 0x8D, 0x95
            ]
        );
        // PLS bit 0=0，设备原样返回 UTF-8，直接 parse 即可
        let parsed = parse_rio_file(&buf).unwrap();
        assert_eq!(parsed.name, "测试歌单");
        assert_eq!(parsed.title, "测试歌单");
    }

    #[test]
    fn utf8_double_encoding_backward_compat() {
        // 向后兼容：旧版本创建的 PLS（bit 0=1）会被设备双重编码。
        // 写入 UTF-8 "测试歌单" → 设备按 latin1 解释 UTF-8 字节 → 重新编码为 UTF-8
        // 软件双重检测：UTF-8 解码成功但全为 latin1 字符 → 转回 latin1 字节 → UTF-8 解码恢复
        let original = "测试歌单";
        let utf8_bytes = original.as_bytes(); // E6 B5 8B E8 AF 95 E6 AD 8C E5 8D 95

        // 模拟设备固件双重编码：按 latin1 解释 UTF-8 字节，再编码为 UTF-8
        let latin1_str: String = utf8_bytes.iter().map(|&b| b as char).collect();
        let double_encoded = latin1_str.as_bytes(); // 每个字节 → 2-3 字节 UTF-8

        // 构造设备返回的 header buffer
        let mut device_buf = [0u8; RIO_FILE_SIZE];
        device_buf[OFF_TITLE..OFF_TITLE + double_encoded.len()]
            .copy_from_slice(double_encoded);

        // 软件读取：双重编码检测恢复 UTF-8 字节，UTF-8 解码得到中文
        let parsed = parse_rio_file(&device_buf).unwrap();
        assert_eq!(parsed.title, "测试歌单");
    }

    #[test]
    fn utf8_encoding_handles_latin1_chars() {
        // latin1 范围字符（如 é = U+00E9）用 UTF-8 编码写入
        // é 的 UTF-8 编码: C3 A9
        let mut f = RioFile::empty();
        f.title = "caf\u{00E9}".to_string();

        let buf = serialize_rio_file(&f);
        // 验证写入的是 UTF-8 字节
        assert_eq!(&buf[OFF_TITLE..OFF_TITLE + 5], "caf\u{00E9}".as_bytes());
        assert_eq!(buf[OFF_TITLE + 3], 0xC3);
        assert_eq!(buf[OFF_TITLE + 4], 0xA9);
    }

    #[test]
    fn read_fixed_string_decodes_utf8_chinese() {
        // 模拟 NodeJS cyrio 上传的中文歌：UTF-8 字节直接存入
        let mut buf = [0u8; RIO_STRING_LEN];
        let utf8_bytes = "测试".as_bytes(); // E6 B5 8B E8 AF 95
        buf[..utf8_bytes.len()].copy_from_slice(utf8_bytes);

        let s = read_fixed_string(&buf, 0, RIO_STRING_LEN);
        assert_eq!(s, "测试");
    }

    #[test]
    fn read_fixed_string_decodes_gbk_chinese() {
        // 模拟原厂中文 Windows 软件上传的中文歌：GBK 字节
        // "测试" 的 GBK 编码: B2 E2 CA D4
        let mut buf = [0u8; RIO_STRING_LEN];
        buf[0] = 0xB2;
        buf[1] = 0xE2;
        buf[2] = 0xCA;
        buf[3] = 0xD4;

        let s = read_fixed_string(&buf, 0, RIO_STRING_LEN);
        assert_eq!(s, "测试");
    }

    #[test]
    fn read_fixed_string_decodes_double_encoded_gbk() {
        // 模拟设备固件把 GBK 字节按 latin1 解释后重新编码为 UTF-8（双重编码）
        // "测试歌单" 的 GBK 编码: B2 E2 CA D4 B8 E8 B5 A5
        // 按 latin1 解释为 "²âÊÔ¸èµ¥"，再编码为 UTF-8:
        let double_encoded = "²âÊÔ¸èµ¥";
        let utf8_bytes = double_encoded.as_bytes();
        let mut buf = [0u8; RIO_STRING_LEN];
        buf[..utf8_bytes.len()].copy_from_slice(utf8_bytes);

        let s = read_fixed_string(&buf, 0, RIO_STRING_LEN);
        assert_eq!(s, "测试歌单");
    }

    #[test]
    fn string_truncates_when_too_long() {
        let mut f = RioFile::empty();
        // 64 字节 ASCII，超长（max_len=64 时正好满，65 字节才会截断）
        let long_name = "a".repeat(80);
        f.name = long_name.clone();

        let buf = serialize_rio_file(&f);
        // 写入区域只有 64 字节，全部为 'a'
        assert_eq!(&buf[OFF_NAME..OFF_NAME + RIO_STRING_LEN], &[b'a'; 64]);
        // 解析出来是 64 个 'a'
        let parsed = parse_rio_file(&buf).unwrap();
        assert_eq!(parsed.name, "a".repeat(64));
    }

    #[test]
    fn read_fixed_string_handles_no_nul() {
        let mut buf = [b'x'; 64];
        // 整个区域无 NUL
        let s = read_fixed_string(&buf, 0, 64);
        assert_eq!(s, "x".repeat(64));
        // 修改一个为 0
        buf[10] = 0;
        let s = read_fixed_string(&buf, 0, 64);
        assert_eq!(s, "xxxxxxxxxx");
    }

    #[test]
    fn overwrite_preserves_unknown_fields() {
        // 模拟从设备读取的原始头：未知字段 0x1c..0x23 填非零值
        let mut original = [0u8; RIO_FILE_SIZE];
        // 制造一些"未知字段"的非零数据
        for b in original.iter_mut().take(0x24).skip(0x1c) {
            *b = 0xAB;
        }
        // 写入原始 file_no
        write_u32_le(&mut original, OFF_FILE_NO, 100);

        // 用 overwrite 修改 file_no 和 size
        let updates = RioFileUpdates {
            file_no: Some(200),
            size: Some(2048),
            ..Default::default()
        };
        overwrite_rio_file_fields(&mut original, &updates);

        // 已知字段被覆盖
        assert_eq!(read_u32_le(&original, OFF_FILE_NO), 200);
        assert_eq!(read_u32_le(&original, OFF_SIZE), 2048);
        // 未知字段保留
        assert_eq!(&original[0x1c..0x24], &[0xAB; 8]);
    }

    #[test]
    fn overwrite_bits_still_forces_required() {
        // 所有文件类型：强制 |= (BITS_REQUIRED & !0x01)，统一清 bit 0
        let mut buf = [0u8; RIO_FILE_SIZE];
        let updates = RioFileUpdates {
            bits: Some(0),
            ..Default::default()
        };
        overwrite_rio_file_fields(&mut buf, &updates);
        assert_eq!(read_u32_le(&buf, OFF_BITS), BITS_REQUIRED & !0x01);

        // PLS 文件：同样不设 bit 0
        let mut pls_buf = [0u8; RIO_FILE_SIZE];
        write_u32_le(&mut pls_buf, OFF_TYPE, TYPE_PLS);
        let pls_updates = RioFileUpdates {
            bits: Some(0),
            ..Default::default()
        };
        overwrite_rio_file_fields(&mut pls_buf, &pls_updates);
        assert_eq!(
            read_u32_le(&pls_buf, OFF_BITS),
            BITS_REQUIRED & !0x01,
            "PLS overwrite should not set bit 0"
        );

        // 所有文件类型：即使输入 bits 有 bit 0=1（旧文件），也要清除
        let mut old_buf = [0u8; RIO_FILE_SIZE];
        let old_updates = RioFileUpdates {
            bits: Some(0x11000111), // 旧文件的 bits，含 bit 0=1
            ..Default::default()
        };
        overwrite_rio_file_fields(&mut old_buf, &old_updates);
        assert_eq!(
            read_u32_le(&old_buf, OFF_BITS) & 0x01,
            0,
            "overwrite must clear bit 0 even if input has it set"
        );

        // MP3 文件同样清除 bit 0
        let mut mp3_buf = [0u8; RIO_FILE_SIZE];
        write_u32_le(&mut mp3_buf, OFF_TYPE, TYPE_MP3);
        let mp3_updates = RioFileUpdates {
            bits: Some(0x111),
            ..Default::default()
        };
        overwrite_rio_file_fields(&mut mp3_buf, &mp3_updates);
        assert_eq!(
            read_u32_le(&mp3_buf, OFF_BITS) & 0x01,
            0,
            "MP3 overwrite must clear bit 0"
        );
    }

    #[test]
    fn overwrite_fixes_double_encoded_name() {
        // 模拟 add_to_playlist 修复场景：
        // 设备上 PLS bit 0=1，name 被 dual-encoded 返回
        // parse_rio_file 正确恢复了中文 name
        // overwrite_rio_file_fields 用正确 name 覆盖双重编码字节 + 清除 bit 0
        let original_utf8 = "测试歌单";
        let utf8_bytes = original_utf8.as_bytes();

        // 模拟设备双重编码：latin1 解释 UTF-8 字节 → 重新编码为 UTF-8
        let latin1_str: String = utf8_bytes.iter().map(|&b| b as char).collect();
        let double_encoded = latin1_str.as_bytes();

        // 构造设备返回的 header buffer（含双重编码 name + bit 0=1）
        let mut buf = [0u8; RIO_FILE_SIZE];
        write_u32_le(&mut buf, OFF_TYPE, TYPE_PLS);
        write_u32_le(&mut buf, OFF_BITS, 0x11000111); // bit 0=1
        buf[OFF_NAME..OFF_NAME + double_encoded.len()].copy_from_slice(double_encoded);
        buf[OFF_TITLE..OFF_TITLE + double_encoded.len()].copy_from_slice(double_encoded);

        // parse_rio_file 正确恢复中文（双重编码检测）
        let parsed = parse_rio_file(&buf).unwrap();
        assert_eq!(parsed.name, "测试歌单");

        // overwrite：用正确的 name 覆盖，并修正 bits
        let updates = RioFileUpdates {
            name: Some(parsed.name.clone()),
            title: Some(parsed.title.clone()),
            bits: Some(parsed.bits),
            ..Default::default()
        };
        overwrite_rio_file_fields(&mut buf, &updates);

        // 验证 name 字段现在是正确的 UTF-8 字节
        let name_bytes = &buf[OFF_NAME..OFF_NAME + utf8_bytes.len()];
        assert_eq!(name_bytes, utf8_bytes, "name should be correct UTF-8");

        // 验证 bit 0 被清除
        assert_eq!(
            read_u32_le(&buf, OFF_BITS) & 0x01,
            0,
            "bit 0 should be cleared"
        );
    }
}
