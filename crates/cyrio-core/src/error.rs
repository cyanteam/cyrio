//! 统一错误类型
//!
//! 整个 cyrio-core 用 [`CyrioError`] 表示错误，配合 `thiserror` 自动实现 Display + From。

use thiserror::Error;

/// cyrio 核心错误类型
#[derive(Debug, Error)]
pub enum CyrioError {
    /// USB transport 错误（nusb / webusb 上报）
    #[error("USB transport error: {0}")]
    Transport(String),

    /// 设备返回的 magic 不匹配（协议错误）
    #[error("magic mismatch: expected {expected}, got {got}")]
    MagicMismatch {
        /// 期望的 magic 字符串
        expected: String,
        /// 实际收到的字符串
        got: String,
    },

    /// 设备返回的 RIO 号错误（如 SRIONOFL 文件不存在）
    #[error("device error: {0}")]
    Device(String),

    /// 文件号无效 / 不存在
    #[error("file not found: fileNo={0}")]
    FileNotFound(u32),

    /// 存储空间不足
    #[error("insufficient free space: need {needed} bytes, have {free} bytes")]
    InsufficientSpace {
        /// 需要的字节数
        needed: u64,
        /// 可用字节数
        free: u64,
    },

    /// 协议解析错误（FIDL / rio_file_t 等）
    #[error("parse error: {0}")]
    Parse(String),

    /// 输入输出错误（文件读写）
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 其他错误
    #[error("{0}")]
    Other(String),
}

/// crate 内统一的 Result 类型
pub type Result<T> = std::result::Result<T, CyrioError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_works() {
        let e = CyrioError::FileNotFound(48);
        assert_eq!(e.to_string(), "file not found: fileNo=48");
    }
}
