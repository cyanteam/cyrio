//! 中文→拼音 + 日语假名→罗马字 slug 转换
//!
//! 将标题中的中文字符转为拼音、日语假名转为罗马字，便于 Rio 设备（字库有限）显示。
//!
//! # 示例
//! ```
//! use cyrio_text::{to_slug, SlugOptions};
//!
//! let opts = SlugOptions::default();
//! assert_eq!(to_slug("【洛天依 原创】赛马", &opts), "【Luo-Tian-Yi Yuan-Chuang】 Sai-Ma");
//! // 日语假名也会转为罗马字
//! assert_eq!(to_slug("ありがとう", &opts), "A-Ri-Ga-To-U");
//! ```

use crate::kana::{is_kana, kana_syllables, kana_to_romaji};
use pinyin::ToPinyin;

/// slug 转换选项
#[derive(Debug, Clone)]
pub struct SlugOptions {
    /// 是否保留标点符号（【】() 等），false 时去除所有非字母数字非空格字符
    pub keep_punctuation: bool,
    /// 拼音词分隔符，默认 '-'
    pub separator: char,
    /// 是否首字母大写
    pub capitalize: bool,
}

impl Default for SlugOptions {
    fn default() -> Self {
        Self {
            keep_punctuation: true,
            separator: '-',
            capitalize: true,
        }
    }
}

/// 将字符串中的中文转为拼音、日语假名转为罗马字
///
/// # 处理规则
/// - 中文字符 → 拼音（可选首字母大写），连续中文用 `separator` 连接
/// - 日语假名（平假名/片假名）→ 罗马字（可选首字母大写），连续假名用 `separator` 连接
/// - 全角字母数字 → 半角（防止 Rio 设备显示乱码）
/// - 非中文非假名字符 → 原样保留（或根据 `keep_punctuation` 过滤）
/// - 从非中文非假名（非空格、非开括号）过渡到中文/假名时，插入空格分隔
/// - 从开括号（【( 等）过渡到中文/假名时，不插入空格
///
/// # 示例
/// ```
/// use cyrio_text::{to_slug, SlugOptions};
///
/// let opts = SlugOptions::default();
/// assert_eq!(to_slug("赛马", &opts), "Sai-Ma");
/// assert_eq!(to_slug("我的歌", &opts), "Wo-De-Ge");
/// // 日语假名转罗马字
/// assert_eq!(to_slug("サクラ", &opts), "Sa-Ku-Ra");
/// ```
pub fn to_slug(input: &str, opts: &SlugOptions) -> String {
    let mut result = String::new();
    let mut cjk_buf = String::new();

    for ch in input.chars() {
        if ch.to_pinyin().is_some() {
            // 中文字符 - 累积
            cjk_buf.push(ch);
        } else if is_kana(ch) {
            // 日语假名 - 累积
            cjk_buf.push(ch);
        } else {
            // 非中文非假名 - 先刷新 CJK 缓冲区
            if !cjk_buf.is_empty() {
                flush_cjk(&mut result, &cjk_buf, opts);
                cjk_buf.clear();
            }
            // 全角→半角转换
            let ch = fullwidth_to_halfwidth(ch);
            // 添加非 CJK 字符
            if opts.keep_punctuation || ch.is_alphanumeric() || ch.is_whitespace() {
                result.push(ch);
            }
        }
    }

    // 刷新剩余的 CJK 字符
    if !cjk_buf.is_empty() {
        flush_cjk(&mut result, &cjk_buf, opts);
    }

    result
}

/// 将累积的中文/假名字符转为拼音/罗马字并追加到 result
fn flush_cjk(result: &mut String, cjk: &str, opts: &SlugOptions) {
    // 判断是否需要插入空格
    if needs_space_before(result) {
        result.push(' ');
    }

    // 分离中文字符和假名字符，分别处理
    // 策略：逐字符判断，中文用拼音，假名用罗马字
    // 连续的中文作为一个拼音组，连续的假名作为一个罗马字组
    let mut parts: Vec<String> = Vec::new();
    let mut chinese_buf = String::new();
    let mut kana_buf = String::new();

    for ch in cjk.chars() {
        if ch.to_pinyin().is_some() {
            // 中文字符
            if !kana_buf.is_empty() {
                // 先刷新假名缓冲区
                parts.push(convert_kana_part(&kana_buf, opts));
                kana_buf.clear();
            }
            chinese_buf.push(ch);
        } else if is_kana(ch) {
            // 假名字符
            if !chinese_buf.is_empty() {
                // 先刷新中文缓冲区
                parts.push(convert_chinese_part(&chinese_buf, opts));
                chinese_buf.clear();
            }
            kana_buf.push(ch);
        } else {
            // 其他 CJK 字符（如 CJK 扩展区字符），原样保留
            if !chinese_buf.is_empty() {
                parts.push(convert_chinese_part(&chinese_buf, opts));
                chinese_buf.clear();
            }
            if !kana_buf.is_empty() {
                parts.push(convert_kana_part(&kana_buf, opts));
                kana_buf.clear();
            }
            parts.push(ch.to_string());
        }
    }

    // 刷新剩余缓冲区
    if !chinese_buf.is_empty() {
        parts.push(convert_chinese_part(&chinese_buf, opts));
    }
    if !kana_buf.is_empty() {
        parts.push(convert_kana_part(&kana_buf, opts));
    }

    let sep = opts.separator.to_string();
    result.push_str(&parts.join(&sep));
}

/// 将中文字符串转为拼音
fn convert_chinese_part(chinese: &str, opts: &SlugOptions) -> String {
    let parts: Vec<String> = chinese
        .chars()
        .filter_map(|ch| ch.to_pinyin())
        .map(|p| {
            let s = p.plain().to_string();
            if opts.capitalize {
                capitalize_first(&s)
            } else {
                s
            }
        })
        .collect();

    let sep = opts.separator.to_string();
    parts.join(&sep)
}

/// 将假名字符串转为罗马字（按音节分割，用 separator 连接）
fn convert_kana_part(kana: &str, opts: &SlugOptions) -> String {
    let syllables = kana_syllables(kana);
    let parts: Vec<String> = syllables
        .iter()
        .map(|syl| {
            let romaji = kana_to_romaji(syl);
            if opts.capitalize {
                capitalize_first(&romaji)
            } else {
                romaji
            }
        })
        .collect();

    let sep = opts.separator.to_string();
    parts.join(&sep)
}

/// 全角字符转半角
///
/// 全角字母数字（！～￣）转为半角等效字符，防止 Rio 设备显示乱码。
/// 全角空格（U+3000）转为普通空格。
fn fullwidth_to_halfwidth(ch: char) -> char {
    // 全角空格 → 半角空格
    if ch == '\u{3000}' {
        return ' ';
    }
    // 全角 ASCII（！～）→ 半角 ASCII（!～）
    // U+FF01 (!) 到 U+FF5E (~) 偏移 0xFF01 - 0x21 = 0xFFE0
    if ('\u{FF01}'..='\u{FF5E}').contains(&ch) {
        return char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch);
    }
    ch
}

/// 判断 result 末尾是否需要插入空格（从非中文/假名过渡到中文/假名时）
fn needs_space_before(result: &str) -> bool {
    if result.is_empty() || result.ends_with(' ') {
        return false;
    }
    let last_ch = result.chars().last().unwrap();
    // 开括号后不需要空格（如 【洛 → 【Luo）
    !is_opening_bracket(last_ch)
}

/// 是否为开括号
fn is_opening_bracket(ch: char) -> bool {
    matches!(
        ch,
        '【' | '[' | '(' | '（' | '「' | '『' | '<' | '〈' | '〔' | '〖'
    )
}

/// 首字母大写
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_slug_basic() {
        let opts = SlugOptions::default();
        assert_eq!(
            to_slug("【洛天依 原创】赛马", &opts),
            "【Luo-Tian-Yi Yuan-Chuang】 Sai-Ma"
        );
    }

    #[test]
    fn to_slug_single_word() {
        let opts = SlugOptions::default();
        assert_eq!(to_slug("赛马", &opts), "Sai-Ma");
    }

    #[test]
    fn to_slug_keeps_english() {
        let opts = SlugOptions::default();
        // 英文原样保留，中文转拼音
        assert_eq!(to_slug("My Song 我的歌", &opts), "My Song Wo-De-Ge");
    }

    #[test]
    fn to_slug_pure_chinese() {
        let opts = SlugOptions::default();
        assert_eq!(to_slug("测试", &opts), "Ce-Shi");
    }

    #[test]
    fn to_slug_no_capitalize() {
        let opts = SlugOptions {
            capitalize: false,
            ..Default::default()
        };
        assert_eq!(to_slug("赛马", &opts), "sai-ma");
    }

    #[test]
    fn to_slug_custom_separator() {
        let opts = SlugOptions {
            separator: '_',
            ..Default::default()
        };
        assert_eq!(to_slug("赛马", &opts), "Sai_Ma");
    }

    #[test]
    fn to_slug_no_punctuation() {
        let opts = SlugOptions {
            keep_punctuation: false,
            ..Default::default()
        };
        // 去除标点：【】 被移除
        assert_eq!(to_slug("【洛天依】赛马", &opts), "Luo-Tian-Yi Sai-Ma");
    }

    #[test]
    fn to_slug_empty() {
        let opts = SlugOptions::default();
        assert_eq!(to_slug("", &opts), "");
    }

    #[test]
    fn to_slug_pure_english() {
        let opts = SlugOptions::default();
        assert_eq!(to_slug("Hello World", &opts), "Hello World");
    }

    #[test]
    fn to_slug_with_parentheses() {
        let opts = SlugOptions::default();
        // 圆括号也是开括号，后不插空格
        assert_eq!(to_slug("(洛天依)赛马", &opts), "(Luo-Tian-Yi) Sai-Ma");
    }

    // ===== 日语假名测试 =====

    #[test]
    fn to_slug_katakana() {
        let opts = SlugOptions::default();
        assert_eq!(to_slug("サクラ", &opts), "Sa-Ku-Ra");
    }

    #[test]
    fn to_slug_hiragana() {
        let opts = SlugOptions::default();
        assert_eq!(to_slug("ありがとう", &opts), "A-Ri-Ga-To-U");
    }

    #[test]
    fn to_slug_mixed_chinese_kana() {
        let opts = SlugOptions::default();
        // 中文和假名混合，中文和假名部分用分隔符连接
        let result = to_slug("歌サクラ", &opts);
        assert_eq!(result, "Ge-Sa-Ku-Ra");
    }

    #[test]
    fn to_slug_japanese_with_yoon() {
        let opts = SlugOptions::default();
        // 拗音测试
        assert_eq!(to_slug("きょう", &opts), "Kyo-U");
    }

    #[test]
    fn to_slug_japanese_with_sokuon() {
        let opts = SlugOptions::default();
        // 促音测试
        assert_eq!(to_slug("がっこう", &opts), "Ga-Kko-U");
    }

    #[test]
    fn to_slug_fullwidth_to_halfwidth() {
        let opts = SlugOptions::default();
        // 全角字母数字转半角
        assert_eq!(to_slug("ＡＢＣ１２３", &opts), "ABC123");
    }

    #[test]
    fn to_slug_fullwidth_space() {
        let opts = SlugOptions::default();
        // 全角空格转半角空格，中文仍转拼音
        assert_eq!(to_slug("歌　曲", &opts), "Ge Qu");
    }

    #[test]
    fn to_slug_mixed_all() {
        let opts = SlugOptions::default();
        // 中文 + 日语 + 英文混合
        let result = to_slug("我的歌 My Song サクラ", &opts);
        assert_eq!(result, "Wo-De-Ge My Song Sa-Ku-Ra");
    }
}
