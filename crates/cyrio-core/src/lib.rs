//! # cyrio-core
//!
//! Diamond Rio S-Series USB MP3 播放器的协议核心层。
//!
//! 平台无关、pure Rust：仅依赖标准库 + byteorder + thiserror + log。
//! 不直接依赖任何 USB 库（nusb/webusb），通过 [`transport::Transport`] trait 抽象。
//!
//! ## 模块结构
//! - [`protocol`]：USB 协议层（操作码、包格式、CRC32、rio_file_t、FIDL）
//! - [`api`]：高层 API（list_songs、upload_song 等）
//! - [`transport`]：USB transport trait（由平台特定 crate 实现）
//! - [`error`]：统一错误类型
//!
//! ## 移植自 NodeJS 项目
//! 对应 `/Users/smile/BACKFILE/project/rust/rio-rs/node/src/protocol/` 与 `api/`。
//! 协议规范见 `docs/PROTOCOL.md`。

#![warn(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::missing_docs_in_private_items)]

pub mod api;
pub mod error;
pub mod protocol;
pub mod transport;

/// crate 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
