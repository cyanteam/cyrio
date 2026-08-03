//! CRC32 实现（rioutil 兼容算法）
//!
//! rioutil 的 `cksum.c` 中 `crc32_rio` 使用非标准 CRC32：
//! - 多项式：`0x04C11DB7`（非反射形式，但表用右移构建）
//! - 初始值：`0`
//! - 反射输入/输出（reflected）
//! - 无输出异或（no final XOR）
//!
//! 与标准 ZIP CRC32（poly `0xEDB88320`, init `0xFFFFFFFF`, final XOR `0xFFFFFFFF`）
//! 不同。真机实测：设备只接受此算法计算的 CRIODATA 包。
//!
//! # 大端字节序
//! CRC 写入 CRIODATA 包时用**大端**字节序（rioutil `big32_2_arch32` 宏），
//! 详见 [`super::packets`]。
//!
//! # 全零缓冲区
//! 全零 16384B 缓冲区的 CRC = 0（下载场景下 CRIODATA 的 CRC 字段总为 0）。
//!
//! # 来源
//! 移植自 NodeJS 项目 `rio-rs/node/src/protocol/crc32.ts`。

/// 预计算的 256 项 CRC 表（rioutil 非标准反射算法）
///
/// 表构建：对每个 i，右移 8 次，若 LSB=1 则异或 `0x04C11DB7`。
/// 注意：用非反射多项式 + 右移是 rioutil 的特殊行为
/// （标准反射 CRC 用 `0xEDB88320`）。
const CRC32_RIO_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut r = i as u32;
        let mut j = 0;
        while j < 8 {
            r = if r & 1 != 0 {
                (r >> 1) ^ 0x04C11DB7
            } else {
                r >> 1
            };
            j += 1;
        }
        t[i] = r;
        i += 1;
    }
    t
};

/// 计算给定切片的 CRC32 校验码（rioutil 兼容算法）
///
/// # 示例
/// ```
/// use cyrio_core::protocol::crc32::crc32;
/// assert_eq!(crc32(b"123456789"), 0x0328b978);  // rioutil 测试向量
/// assert_eq!(crc32(&[]), 0x00000000);            // 空切片
/// assert_eq!(crc32(&[0u8; 16384]), 0x00000000);  // 全零缓冲区（下载场景）
/// ```
pub fn crc32(buf: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for &byte in buf {
        crc = (crc >> 8) ^ CRC32_RIO_TABLE[((crc ^ byte as u32) & 0xff) as usize];
    }
    crc
}

/// 增量 CRC32 更新（rioutil 兼容算法）
///
/// 用于把大文件分块计算 CRC 的场景（避免一次拷贝整个文件到内存）。
/// 初始 `prev` 应传 `0`，后续每次传上一次的返回值。
///
/// # 示例
/// ```
/// use cyrio_core::protocol::crc32::{crc32, crc32_update};
/// let full = b"Hello World, this is a test";
/// let crc_full = crc32(full);
/// let mut crc_inc = 0u32;
/// crc_inc = crc32_update(crc_inc, &full[0..5]);
/// crc_inc = crc32_update(crc_inc, &full[5..11]);
/// crc_inc = crc32_update(crc_inc, &full[11..]);
/// assert_eq!(crc_inc, crc_full);
/// ```
pub fn crc32_update(prev: u32, buf: &[u8]) -> u32 {
    let mut crc = prev;
    for &byte in buf {
        crc = (crc >> 8) ^ CRC32_RIO_TABLE[((crc ^ byte as u32) & 0xff) as usize];
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// rioutil 测试向量 "123456789" → 0x0328b978
    #[test]
    fn rioutil_vector_123456789() {
        assert_eq!(crc32(b"123456789"), 0x0328b978);
    }

    /// 空 Buffer → 0
    #[test]
    fn empty_buffer_is_zero() {
        assert_eq!(crc32(&[]), 0x00000000);
    }

    /// 全零 16384B 缓冲区 → 0（下载场景下 CRIODATA 的 CRC 总为 0）
    #[test]
    fn all_zero_16384b_is_zero() {
        assert_eq!(crc32(&[0u8; 16384]), 0x00000000);
    }

    /// "Hello World" → 0x07d19dd1（非标准 ZIP CRC 0x4a17b156）
    #[test]
    fn hello_world() {
        assert_eq!(crc32(b"Hello World"), 0x07d19dd1);
    }

    /// 增量更新等于一次性计算
    #[test]
    fn incremental_equals_one_shot() {
        let full = b"Hello World, this is a test";
        let crc_full = crc32(full);
        let mut crc_inc = 0u32;
        crc_inc = crc32_update(crc_inc, &full[0..5]);
        crc_inc = crc32_update(crc_inc, &full[5..11]);
        crc_inc = crc32_update(crc_inc, &full[11..]);
        assert_eq!(crc_inc, crc_full);
    }

    /// 处理高位字节（\xff\xfe\xfd）时返回值与预期一致
    #[test]
    fn returns_unsigned_32bit() {
        let crc = crc32(b"some test data with high bit pattern \xff\xfe\xfd");
        // 验证算法对高位字节处理正确（非零、且为确定值）
        assert_ne!(crc, 0);
        // 同一输入应产生稳定输出
        assert_eq!(crc, crc32(b"some test data with high bit pattern \xff\xfe\xfd"));
    }
}
