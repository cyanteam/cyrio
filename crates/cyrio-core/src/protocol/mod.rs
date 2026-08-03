//! # 协议层
//!
//! Diamond Rio S-Series USB 协议的实现，平台无关。
//!
//! ## 子模块
//! - [`constants`]：操作码、PID、内存单元常量
//! - [`crc32`]：rioutil 兼容 CRC32（与 nodejs `src/protocol/crc32.ts` 对齐）
//! - [`packets`]：命令包构造（CRIODATA / CRIOINFO / CRIOABORT 等）
//! - [`rio_file`]：rio_file_t 结构体（2048B，文件元数据）
//! - [`rio_mem`]：rio_mem_t 结构体（256B，内存单元信息）
//! - [`fidl`]：FIDL 播放列表二进制格式
//!
//! ## 协议规范
//! 详见 `docs/PROTOCOL.md`（从 nodejs 项目复制）。

pub mod constants;
pub mod crc32;
pub mod fidl;
pub mod packets;
pub mod rio_file;
pub mod rio_mem;
