//! rio_mem_t 结构体序列化（256 字节）
//!
//! 通过 `OP_RIO_MEMRI` (0x68) 读取，描述一个内存单元（内置闪存或 SD 卡）。
//!
//! ## 关键陷阱
//! - S-Series 中 `size`/`used`/`free`/`system` 字段单位是**字节**，显示时
//!   `/1024/1024` 得 MB。
//! - 老机型（Rio Riot 等）单位是 KB——本库仅支持 S-Series，按字节处理。
//! - 若请求不存在的内存单元（如未插 SD 卡时查询单元 1），设备返回 256B 全 0，
//!   `size == 0` 即视为不存在。
//!
//! # 来源
//! 移植自 NodeJS 项目 `rio-rs/node/src/protocol/rioMem.ts`。

use crate::error::{CyrioError, Result};
use crate::protocol::constants::{
    OFF_MEM_FREE, OFF_MEM_MODEL, OFF_MEM_NAME, OFF_MEM_SIZE, OFF_MEM_SYSTEM, OFF_MEM_USED,
    RIO_MEM_SIZE, RIO_STRING_LEN,
};

/// Rio 内存单元信息（对应 256 字节的 rio_mem_t 结构）
///
/// 字段含义详见 `docs/PROTOCOL.md` §12。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RioMem {
    /// u32 @ 0x10：总大小（字节）
    pub size: u32,
    /// u32 @ 0x14：已用字节
    pub used: u32,
    /// u32 @ 0x18：空闲字节
    pub free: u32,
    /// u32 @ 0x1c：系统保留字节
    pub system: u32,
    /// char[64] @ 0x40：内存单元名（如 "Internal Memory"）
    pub name: String,
    /// char[64] @ 0xc0：型号字符串
    pub model: String,
}

impl RioMem {
    /// 创建一个全 0 的空 `RioMem` 模板
    pub fn empty() -> Self {
        Self::default()
    }

    /// 判断该内存单元是否存在
    ///
    /// 设备未插 SD 卡时查询单元 1 会返回 `size == 0`。
    pub fn is_present(&self) -> bool {
        self.size > 0
    }

    /// 返回内存单元大小的 MB 字符串（人类可读）
    ///
    /// 字段单位是字节，`/1024/1024` 得 MB。
    pub fn format_size(&self) -> String {
        let mb = self.size as f64 / (1024.0 * 1024.0);
        let free_mb = self.free as f64 / (1024.0 * 1024.0);
        let used_mb = self.used as f64 / (1024.0 * 1024.0);
        format!(
            "{:.1}MB used / {:.1}MB free / {:.1}MB total",
            used_mb, free_mb, mb
        )
    }
}

/// 将 [`RioMem`] 序列化为 256 字节数组
///
/// 主要用于测试场景；正常使用时 `RioMem` 是从设备读取的，无需序列化。
pub fn serialize_rio_mem(mem: &RioMem) -> [u8; RIO_MEM_SIZE] {
    let mut buf = [0u8; RIO_MEM_SIZE];
    write_u32_le(&mut buf, OFF_MEM_SIZE, mem.size);
    write_u32_le(&mut buf, OFF_MEM_USED, mem.used);
    write_u32_le(&mut buf, OFF_MEM_FREE, mem.free);
    write_u32_le(&mut buf, OFF_MEM_SYSTEM, mem.system);
    write_fixed_string(&mut buf, OFF_MEM_NAME, RIO_STRING_LEN, &mem.name);
    write_fixed_string(&mut buf, OFF_MEM_MODEL, RIO_STRING_LEN, &mem.model);
    buf
}

/// 从 256 字节切片反序列化为 [`RioMem`]
///
/// # 错误
/// 缓冲区长度不足 [`RIO_MEM_SIZE`] 时返回 [`CyrioError::Parse`]。
pub fn parse_rio_mem(buf: &[u8]) -> Result<RioMem> {
    if buf.len() < RIO_MEM_SIZE {
        return Err(CyrioError::Parse(format!(
            "parse_rio_mem: buffer too short (got {}, need {})",
            buf.len(),
            RIO_MEM_SIZE
        )));
    }
    Ok(RioMem {
        size: read_u32_le(buf, OFF_MEM_SIZE),
        used: read_u32_le(buf, OFF_MEM_USED),
        free: read_u32_le(buf, OFF_MEM_FREE),
        system: read_u32_le(buf, OFF_MEM_SYSTEM),
        name: read_fixed_string(buf, OFF_MEM_NAME, RIO_STRING_LEN),
        model: read_fixed_string(buf, OFF_MEM_MODEL, RIO_STRING_LEN),
    })
}

// ============================================================================
// 内部工具函数（与 rio_file.rs 复用同样语义，本模块自包含以减少耦合）
// ============================================================================

fn read_u32_le(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

fn read_fixed_string(buf: &[u8], offset: usize, max_len: usize) -> String {
    let slice = &buf[offset..offset + max_len];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(max_len);
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

fn write_fixed_string(buf: &mut [u8], offset: usize, max_len: usize, value: &str) {
    for b in &mut buf[offset..offset + max_len] {
        *b = 0;
    }
    let bytes = value.as_bytes();
    let write_len = bytes.len().min(max_len);
    buf[offset..offset + write_len].copy_from_slice(&bytes[..write_len]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rio_mem_is_all_zero() {
        let m = RioMem::empty();
        assert_eq!(m.size, 0);
        assert_eq!(m.used, 0);
        assert_eq!(m.free, 0);
        assert_eq!(m.name, "");
        assert!(!m.is_present()); // size=0 → 不存在
    }

    #[test]
    fn serialize_writes_fields_le() {
        let mut m = RioMem::empty();
        m.size = 64 * 1024 * 1024; // 64MB
        m.used = 10 * 1024 * 1024;
        m.free = 50 * 1024 * 1024;
        m.system = 4 * 1024 * 1024;
        m.name = "Internal Memory".to_string();
        m.model = "Rio S50".to_string();

        let buf = serialize_rio_mem(&m);

        assert_eq!(read_u32_le(&buf, OFF_MEM_SIZE), 64 * 1024 * 1024);
        assert_eq!(read_u32_le(&buf, OFF_MEM_USED), 10 * 1024 * 1024);
        assert_eq!(read_u32_le(&buf, OFF_MEM_FREE), 50 * 1024 * 1024);
        assert_eq!(read_u32_le(&buf, OFF_MEM_SYSTEM), 4 * 1024 * 1024);

        // 字符串：UTF-8 + NUL（"Internal Memory" 是 15 字节）
        assert_eq!(&buf[OFF_MEM_NAME..OFF_MEM_NAME + 15], b"Internal Memory");
        assert_eq!(buf[OFF_MEM_NAME + 15], 0);
    }

    #[test]
    fn parse_reads_fields() {
        let mut m = RioMem::empty();
        m.size = 128 * 1024 * 1024;
        m.used = 30 * 1024 * 1024;
        m.free = 90 * 1024 * 1024;
        m.system = 8 * 1024 * 1024;
        m.name = "SD Card".to_string();
        m.model = "Rio S30S".to_string();

        let buf = serialize_rio_mem(&m);
        let parsed = parse_rio_mem(&buf).unwrap();

        assert_eq!(parsed, m);
        assert!(parsed.is_present());
    }

    #[test]
    fn parse_returns_error_for_short_buffer() {
        let buf = [0u8; 100];
        let err = parse_rio_mem(&buf).unwrap_err();
        assert!(matches!(err, CyrioError::Parse(_)));
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn all_zero_buffer_means_not_present() {
        let buf = [0u8; RIO_MEM_SIZE];
        let m = parse_rio_mem(&buf).unwrap();
        assert!(!m.is_present());
        assert_eq!(m.size, 0);
    }

    #[test]
    fn format_size_renders_mb() {
        let mut m = RioMem::empty();
        m.size = 64 * 1024 * 1024;
        m.used = 10 * 1024 * 1024;
        m.free = 50 * 1024 * 1024;
        let s = m.format_size();
        assert!(s.contains("10.0MB used"));
        assert!(s.contains("50.0MB free"));
        assert!(s.contains("64.0MB total"));
    }

    #[test]
    fn round_trip_preserves_all_fields() {
        let mut m = RioMem::empty();
        m.size = 0x10000000;
        m.used = 0x8000000;
        m.free = 0x4000000;
        m.system = 0x2000000;
        m.name = "测试内存".to_string();
        m.model = "Rio S35S".to_string();

        let buf = serialize_rio_mem(&m);
        let parsed = parse_rio_mem(&buf).unwrap();
        assert_eq!(parsed, m);
    }
}
