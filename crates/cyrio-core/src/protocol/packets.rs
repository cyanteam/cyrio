//! 握手包构造与解析（64 字节固定长度）
//!
//! 所有握手包都是 64 字节：
//! - `bytes[0..7]`：8 字节 ASCII 魔数（如 `b"CRIODATA"`）
//! - `bytes[8..11]`：4 字节**大端** CRC32（仅 CRIODATA 有，CRIOINFO 为 0）
//! - `bytes[12..63]`：52 字节 0 填充
//!
//! ## 主机发送
//! - [`build_crio_data`]：数据块前导包，后接 16384B 数据。CRC32 = `crc32(后接数据)`
//! - [`build_crio_info`]：文件头前导包，后接 2048B 头。**无 CRC**，4 字节字段必须为 0
//! - [`build_crio_abort`]：中止传输包，用于异常时尽量让设备退出等待状态
//!
//! ## 设备返回
//! - `SRIORDY`：设备就绪
//! - `SRIODATA`：数据块确认
//! - `SRIODONE`：传输完成
//! - `SRIONOFL`：文件不存在
//! - `SRIODELS`：删除开始确认
//! - `SRIODELD`：删除完成
//! - `SRIOFMTD`：格式化完成
//!
//! ## 关键陷阱
//! 1. CRC32 用**大端**字节序写入（rioutil `big32_2_arch32` 宏），不是小端
//! 2. CRC32 算法是 rioutil 非标准变体（详见 [`super::crc32`]），不是标准 ZIP CRC32
//! 3. CRIOINFO 的 4 字节 CRC 字段必须填 0，**不**调用 crc32
//! 4. 设备返回的魔数可能是 7 字节（如 `SRIORDY`）+ 1 字节 `0x00`，
//!    [`parse_magic`] 按已知魔数列表匹配，再兜底前 8 字节 latin1
//!
//! # 来源
//! 移植自 NodeJS 项目 `rio-rs/node/src/protocol/packets.ts`。

use crate::error::{CyrioError, Result};
use crate::protocol::constants::{
    MAGIC_CRIOABRT, MAGIC_CRIODATA, MAGIC_CRIOINFO, MAGIC_SRIODELD, MAGIC_SRIODELS, MAGIC_SRIODATA,
    MAGIC_SRIODONE, MAGIC_SRIOFMTD, MAGIC_SRIORDY, MAGIC_SRIONOFL, PKT_HANDSHAKE,
};
use crate::protocol::crc32::crc32;

/// 已知的主机发送魔数（8 字节）
const HOST_MAGICS: &[&[u8]] = &[MAGIC_CRIODATA, MAGIC_CRIOINFO, MAGIC_CRIOABRT];

/// 已知的设备返回魔数（部分 7 字节，部分 8 字节）
const DEVICE_MAGICS: &[&[u8]] = &[
    MAGIC_SRIORDY,
    MAGIC_SRIODATA,
    MAGIC_SRIODONE,
    MAGIC_SRIONOFL,
    MAGIC_SRIODELS,
    MAGIC_SRIODELD,
    MAGIC_SRIOFMTD,
];

/// 构造 CRIODATA 握手包（数据块前导）
///
/// 包格式：`8B "CRIODATA" + 4B **BE** crc32(chunk) + 52B 0`
///
/// rioutil 的 `write_cksum_rio` 用 `big32_2_arch32` 宏把 CRC 转为大端后写入，
/// 真机实测只接受大端字节序的 CRC。
///
/// # 参数
/// - `chunk`：后续要发送的 16384B 数据块
///
/// # 返回
/// 64 字节 `[u8; 64]`
pub fn build_crio_data(chunk: &[u8]) -> [u8; PKT_HANDSHAKE] {
    let mut buf = [0u8; PKT_HANDSHAKE];
    buf[..MAGIC_CRIODATA.len()].copy_from_slice(MAGIC_CRIODATA);
    // CRC32 大端写入（rioutil big32_2_arch32 宏）
    let crc = crc32(chunk);
    buf[8..12].copy_from_slice(&crc.to_be_bytes());
    buf
}

/// 构造 CRIOINFO 握手包（文件头前导）
///
/// 包格式：`8B "CRIOINFO" + 4B 0x00000000（**无 CRC**） + 52B 0`
///
/// 关键：4 字节 CRC 字段必须填 0，不能调用 crc32。这是协议最常见的实现错误。
pub fn build_crio_info() -> [u8; PKT_HANDSHAKE] {
    let mut buf = [0u8; PKT_HANDSHAKE];
    buf[..MAGIC_CRIOINFO.len()].copy_from_slice(MAGIC_CRIOINFO);
    // bytes[8..12] 保持 0
    buf
}

/// 构造 CRIOABRT 握手包（中止传输）
///
/// 包格式：`8B "CRIOABRT" + 56B 0`
///
/// 用于异常时尽力让设备退出等待状态。设备不一定会响应，但发完后再抛错。
pub fn build_crio_abort() -> [u8; PKT_HANDSHAKE] {
    let mut buf = [0u8; PKT_HANDSHAKE];
    buf[..MAGIC_CRIOABRT.len()].copy_from_slice(MAGIC_CRIOABRT);
    buf
}

/// 从握手包解析魔数字符串
///
/// 按已知魔数列表尝试匹配（不区分 7/8 字节）。匹配失败则返回前 8 字节 latin1 解码
/// （去掉尾部 NUL）。
///
/// # 参数
/// - `buf`：至少 8 字节的切片
///
/// # 返回
/// 识别出的魔数字符串
pub fn parse_magic(buf: &[u8]) -> String {
    // 优先匹配主机魔数（8 字节）
    for magic in HOST_MAGICS {
        if buf.starts_with(magic) {
            return String::from_utf8_lossy(magic).into_owned();
        }
    }
    // 再匹配设备魔数（7 或 8 字节）
    for magic in DEVICE_MAGICS {
        if buf.starts_with(magic) {
            return String::from_utf8_lossy(magic).into_owned();
        }
    }
    // 兜底：返回前 8 字节 latin1，去掉 NUL
    let n = buf.len().min(8);
    let raw = &buf[..n];
    let s = String::from_utf8_lossy(raw);
    s.trim_end_matches('\0').to_string()
}

/// 断言握手包魔数与期望一致，不一致则返回 [`CyrioError::MagicMismatch`]
///
/// # 参数
/// - `buf`：接收到的握手包
/// - `expected`：期望的魔数（如 `MAGIC_SRIORDY`）
/// - `context`：错误上下文描述（如 `"upload init"`），用于错误消息
pub fn expect_magic(buf: &[u8], expected: &[u8], context: &str) -> Result<()> {
    let expected_str = String::from_utf8_lossy(expected);
    let actual = parse_magic(buf);
    if actual.as_bytes() == expected {
        Ok(())
    } else if buf.starts_with(expected) {
        // 防御性：parse_magic 已识别则上面分支返回；这里处理 expected 不在已知表
        Ok(())
    } else {
        // raw hex 用于调试
        let n = buf.len().min(8);
        let hex: String = buf[..n].iter().map(|b| format!("{:02x}", b)).collect();
        Err(CyrioError::MagicMismatch {
            expected: format!("{} (context: {})", expected_str, context),
            got: format!("{} (raw: {})", actual, hex),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_crio_data_writes_magic_and_be_crc() {
        let chunk = [0u8; 16384]; // 全零 → crc32 = 0
        let pkt = build_crio_data(&chunk);
        // 前 8 字节是 "CRIODATA"
        assert_eq!(&pkt[..8], b"CRIODATA");
        // CRC 为 0，大端写入也是 4 个 0
        assert_eq!(&pkt[8..12], &[0, 0, 0, 0]);
        // 后 52 字节全 0
        assert_eq!(&pkt[12..], &[0u8; 52]);
    }

    #[test]
    fn build_crio_data_with_nonzero_chunk_writes_be_crc() {
        let chunk = b"123456789"; // crc32 = 0x0328b978
        let pkt = build_crio_data(chunk);
        assert_eq!(&pkt[8..12], &[0x03, 0x28, 0xb9, 0x78]); // big-endian
    }

    #[test]
    fn build_crio_info_has_zero_crc_field() {
        let pkt = build_crio_info();
        assert_eq!(&pkt[..8], b"CRIOINFO");
        // CRC 字段必须为 0（不是 crc32 计算结果）
        assert_eq!(&pkt[8..12], &[0, 0, 0, 0]);
        assert_eq!(&pkt[12..], &[0u8; 52]);
    }

    #[test]
    fn build_crio_abort_format() {
        let pkt = build_crio_abort();
        assert_eq!(&pkt[..8], b"CRIOABRT");
        assert_eq!(&pkt[8..], &[0u8; 56]);
    }

    #[test]
    fn parse_magic_recognizes_host_magics() {
        let pkt = build_crio_data(&[0u8; 16]);
        assert_eq!(parse_magic(&pkt), "CRIODATA");
        let pkt = build_crio_info();
        assert_eq!(parse_magic(&pkt), "CRIOINFO");
        let pkt = build_crio_abort();
        assert_eq!(parse_magic(&pkt), "CRIOABRT");
    }

    #[test]
    fn parse_magic_recognizes_device_magics() {
        // SRIORDY 7 字节 + 1 字节 0x00
        let mut buf = [0u8; 64];
        buf[..7].copy_from_slice(b"SRIORDY");
        assert_eq!(parse_magic(&buf), "SRIORDY");
        // SRIODONE 8 字节
        let mut buf = [0u8; 64];
        buf[..8].copy_from_slice(b"SRIODONE");
        assert_eq!(parse_magic(&buf), "SRIODONE");
    }

    #[test]
    fn parse_magic_fallback_returns_first_8_bytes_trimmed() {
        // 未知魔数 → 兜底返回前 8 字节（去掉 NUL）
        let mut buf = [0u8; 64];
        buf[..5].copy_from_slice(b"HELLO");
        assert_eq!(parse_magic(&buf), "HELLO");
    }

    #[test]
    fn expect_magic_ok_for_matching_buf() {
        let pkt = build_crio_info();
        assert!(expect_magic(&pkt, MAGIC_CRIOINFO, "test").is_ok());
    }

    #[test]
    fn expect_magic_err_for_mismatched_buf() {
        let pkt = build_crio_abort();
        let err = expect_magic(&pkt, MAGIC_SRIORDY, "upload init").unwrap_err();
        // 错误消息包含上下文
        let msg = err.to_string();
        assert!(msg.contains("upload init"), "msg: {}", msg);
        assert!(msg.contains("SRIORDY"), "msg: {}", msg);
        assert!(msg.contains("CRIOABRT"), "msg: {}", msg);
    }

    #[test]
    fn packets_are_64_bytes() {
        assert_eq!(build_crio_data(&[0u8; 16]).len(), 64);
        assert_eq!(build_crio_info().len(), 64);
        assert_eq!(build_crio_abort().len(), 64);
    }
}
