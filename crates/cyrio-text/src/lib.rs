//! cyrio-text：cyrio 文本处理 crate
//!
//! 提供两个核心功能：
//! - [`slug::to_slug`]：中文→拼音 slug 转换（"赛马"→"Sai-Ma"）
//! - [`strip::strip_noise`]：去除标题中的无关词汇（Hi-Res、4K、括号内容等）
//!
//! # 设计目标
//! - 纯 Rust 实现，无 C/Python 依赖
//! - 不依赖 AI/LLM，使用规则引擎
//! - 可独立测试，便于维护

pub mod kana;
pub mod rules;
pub mod slug;
pub mod strip;

pub use kana::{contains_kana, is_kana, kana_syllables, kana_to_romaji};
pub use slug::{to_slug, SlugOptions};
pub use strip::{strip_noise, StripOptions, StripResult};
