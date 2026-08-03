//! 内置停用词规则
//!
//! 这些词汇常见于 B 站等视频平台下载的歌曲标题，对 Rio 设备的小屏幕显示无意义。
//! 用户可通过 [`StripOptions::custom_stop_words`](super::strip::StripOptions::custom_stop_words) 添加自定义停用词。

/// 内置停用词列表
///
/// 包含以下类别：
/// - 音质标记：Hi-Res、无损、FLAC、APE、SQ、HQ 等
/// - 视频规格：4K、8K、高清、HDR、60FPS 等
/// - 创作类型：原创、remix、cover、MV、PV 等
/// - 来源标记：bilibili、B站、哔哩哔哩
/// - 用户示例：在百万级播音室大声听
///
/// 匹配时大小写敏感（因为 "4K" 和 "4k" 视觉上差异明显，且原词通常正确大小写）。
/// 对于可能多种大小写的词（如 Hi-Res/Hi-res/hires），显式列出所有变体。
pub const BUILTIN_STOP_WORDS: &[&str] = &[
    // ===== 音质标记 =====
    "Hi-Res",
    "Hi-res",
    "HiRes",
    "Hires",
    "hires",
    "无损",
    "FLAC",
    "flac",
    "APE",
    "ape",
    "SQ",
    "HQ",
    "Lossless",
    "lossless",
    // ===== 视频规格 =====
    "4K",
    "8K",
    "2K",
    "1080P",
    "1080p",
    "720P",
    "720p",
    "高清",
    "HDR",
    "hdr",
    "60FPS",
    "60fps",
    "30FPS",
    "30fps",
    // ===== 创作类型 =====
    "原创",
    "remix",
    "Remix",
    "REMIX",
    "cover",
    "Cover",
    "COVER",
    // ===== 媒体类型 =====
    "MV",
    "PV",
    "mv",
    "pv",
    // ===== 来源标记 =====
    "bilibili",
    "B站",
    "哔哩哔哩",
    // ===== 用户示例 =====
    "在百万级播音室大声听",
];

/// 检查词是否为内置停用词（大小写敏感）
pub fn is_builtin_stop_word(word: &str) -> bool {
    BUILTIN_STOP_WORDS.contains(&word)
}
