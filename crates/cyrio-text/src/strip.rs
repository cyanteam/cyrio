//! 去除标题中的无关词汇
//!
//! B 站等平台下载的歌曲标题常含 Hi-Res、4K、原创、活动名称等无关词汇，
//! Rio 设备显示区域小，这些词汇浪费空间。本模块用规则引擎（非 AI）去除。
//!
//! # 处理顺序
//! 1. 去除括号内容（活动名称等）
//! 2. 去除引号内容（歌词片段等）
//! 3. 去除停用词（Hi-Res、4K、高清等）
//! 4. 折叠多余空白

use crate::rules;

/// 去词选项
#[derive(Debug, Clone)]
pub struct StripOptions {
    /// 自定义停用词（除内置停用词外额外去除）
    pub custom_stop_words: Vec<String>,
    /// 是否去除括号内容（(活动名称)、【现场版】等）
    pub strip_parentheses: bool,
    /// 是否去除引号内容（"歌词片段"、"歌词片段"等）
    pub strip_quotes: bool,
    /// 是否去除音质/规格等停用词（Hi-Res、4K、高清等）
    pub strip_quality_tags: bool,
}

impl Default for StripOptions {
    fn default() -> Self {
        Self {
            custom_stop_words: vec![],
            strip_parentheses: true,
            strip_quotes: true,
            strip_quality_tags: true,
        }
    }
}

/// 去词结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripResult {
    /// 清理后的标题
    pub cleaned: String,
    /// 被去除的片段列表（用于日志/确认）
    pub removed: Vec<String>,
}

/// 去除标题中的无关词汇
///
/// # 示例
/// ```
/// use cyrio_text::{strip_noise, StripOptions};
///
/// let opts = StripOptions::default();
/// let result = strip_noise("歌曲 Hi-Res 无损 4K", &opts);
/// assert_eq!(result.cleaned, "歌曲");
/// ```
pub fn strip_noise(input: &str, opts: &StripOptions) -> StripResult {
    let mut current = input.to_string();
    let mut removed = Vec::new();

    // 1. 去除括号内容
    if opts.strip_parentheses {
        let (s, r) = strip_parentheses_content(&current);
        current = s;
        removed.extend(r);
    }

    // 2. 去除引号内容
    if opts.strip_quotes {
        let (s, r) = strip_quoted_content(&current);
        current = s;
        removed.extend(r);
    }

    // 3. 去除停用词
    if opts.strip_quality_tags || !opts.custom_stop_words.is_empty() {
        let (s, r) = strip_stop_words(&current, opts);
        current = s;
        removed.extend(r);
    }

    // 4. 折叠多余空白
    current = collapse_whitespace(&current);

    StripResult {
        cleaned: current,
        removed,
    }
}

/// 去除括号内容（包括括号本身）
///
/// 支持的括号对：
/// - `( )` 英文圆括号
/// - `（ ）` 中文圆括号
/// - `[ ]` 英文方括号
/// - `【 】` 中文方括号
/// - `{ }` 英文花括号
/// - `「 」` 日文引号
/// - `『 』` 日文双引号
///
/// 未闭合的括号内容会被保留（避免数据丢失）。
fn strip_parentheses_content(input: &str) -> (String, Vec<String>) {
    let mut result = String::new();
    let mut removed = Vec::new();
    // 用栈记录每层括号的起始位置（在 pending 中的位置）
    // pending 存储当前正在构建的括号内容（含开括号本身）
    let mut pending = String::new();
    let mut bracket_stack: Vec<usize> = vec![]; // 记录每个开括号在 pending 中的字符位置

    for ch in input.chars() {
        if is_opening_bracket(ch) {
            bracket_stack.push(pending.chars().count());
            pending.push(ch);
        } else if is_closing_bracket(ch) {
            if let Some(open_pos) = bracket_stack.pop() {
                // 匹配到开括号 - 提取括号内内容（不含括号本身）
                let pending_str = pending.clone();
                let open_char_count = open_pos + 1; // 开括号后的位置
                let content: String = pending_str.chars().skip(open_char_count).collect();
                let content = content.trim().to_string();
                if !content.is_empty() {
                    removed.push(content);
                }
                // 丢弃 pending 中开括号及之后的内容
                pending = pending_str.chars().take(open_pos).collect();
            } else {
                // 没有匹配的开括号 - 保留闭括号
                pending.push(ch);
            }
        } else {
            pending.push(ch);
        }

        // 如果栈为空，把 pending 追加到 result
        if bracket_stack.is_empty() && !pending.is_empty() {
            result.push_str(&pending);
            pending.clear();
        }
    }

    // 未闭合的括号内容保留（避免数据丢失）
    if !pending.is_empty() {
        result.push_str(&pending);
    }

    (result, removed)
}

/// 去除引号内容（包括引号本身）
///
/// 支持的引号对：
/// - `" "` 英文双引号
/// - `" "` 中文双引号
/// - `' '` 英文单引号
/// - `' '` 中文单引号
fn strip_quoted_content(input: &str) -> (String, Vec<String>) {
    let mut result = String::new();
    let mut removed = Vec::new();
    let mut buf = String::new();
    let mut in_quote: Option<char> = None;

    for ch in input.chars() {
        if let Some(opening) = in_quote {
            if matches_closing_quote(ch, opening) {
                // 闭合引号
                let content = buf.trim().to_string();
                if !content.is_empty() {
                    removed.push(content);
                }
                buf.clear();
                in_quote = None;
            } else {
                buf.push(ch);
            }
        } else if is_opening_quote(ch) {
            in_quote = Some(ch);
            buf.clear();
        } else {
            result.push(ch);
        }
    }

    // 未闭合的引号内容保留
    if let Some(_opening) = in_quote {
        if !buf.is_empty() {
            result.push_str(&buf);
        }
    }

    (result, removed)
}

/// 去除停用词（内置 + 自定义）
fn strip_stop_words(input: &str, opts: &StripOptions) -> (String, Vec<String>) {
    let mut result = input.to_string();
    let mut removed = Vec::new();

    // 收集所有要去除的词
    let mut words_to_strip: Vec<&str> = vec![];
    if opts.strip_quality_tags {
        words_to_strip.extend(rules::BUILTIN_STOP_WORDS.iter().copied());
    }
    if !opts.custom_stop_words.is_empty() {
        // 自定义词稍后处理（因为是 String，不能直接 join 到 &str）
    }

    // 去除内置停用词（按长度降序，优先匹配长词，避免短词误匹配）
    let mut sorted_words: Vec<&str> = words_to_strip.clone();
    sorted_words.sort_by(|a, b| b.len().cmp(&a.len()));

    for word in &sorted_words {
        if result.contains(word) {
            removed.push(word.to_string());
            result = result.replace(word, " ");
        }
    }

    // 去除自定义停用词
    let mut custom_sorted = opts.custom_stop_words.clone();
    custom_sorted.sort_by(|a, b| b.len().cmp(&a.len()));
    for word in &custom_sorted {
        if !word.is_empty() && result.contains(word) {
            removed.push(word.clone());
            result = result.replace(word, " ");
        }
    }

    (result, removed)
}

/// 折叠多余空白（多个空格变一个，去除首尾空格）
fn collapse_whitespace(input: &str) -> String {
    let mut result = String::new();
    let mut prev_was_space = false;

    for ch in input.chars() {
        if ch.is_whitespace() {
            if !prev_was_space && !result.is_empty() {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }

    // 去除尾部空格
    if result.ends_with(' ') {
        result.pop();
    }

    result
}

// ============================================================================
// 括号/引号识别
// ============================================================================

fn is_opening_bracket(ch: char) -> bool {
    matches!(
        ch,
        '(' | '（' | '[' | '【' | '{' | '「' | '『' | '〈' | '〔' | '〖'
    )
}

fn is_closing_bracket(ch: char) -> bool {
    matches!(
        ch,
        ')' | '）' | ']' | '】' | '}' | '」' | '』' | '〉' | '〕' | '〗'
    )
}

fn is_opening_quote(ch: char) -> bool {
    matches!(ch, '"' | '\u{201C}' | '\'' | '\u{2018}' | '「' | '『')
}

fn matches_closing_quote(ch: char, opening: char) -> bool {
    match opening {
        '"' => ch == '"',
        '\u{201C}' => ch == '\u{201D}',
        '\'' => ch == '\'',
        '\u{2018}' => ch == '\u{2019}',
        '「' => ch == '」',
        '『' => ch == '』',
        _ => false,
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_quality_tags() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲 Hi-Res 无损 4K", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_removes_parens() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲（活动名称）", &opts);
        assert_eq!(result.cleaned, "歌曲");
        assert!(result.removed.contains(&"活动名称".to_string()));
    }

    #[test]
    fn strip_removes_brackets() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲【现场版】", &opts);
        assert_eq!(result.cleaned, "歌曲");
        assert!(result.removed.contains(&"现场版".to_string()));
    }

    #[test]
    fn strip_removes_english_parens() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲 (Live Version)", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_removes_quotes() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲 \"歌词片段\"", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_removes_chinese_quotes() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲“歌词片段”", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_preserves_normal() {
        let opts = StripOptions::default();
        let result = strip_noise("我的歌曲", &opts);
        assert_eq!(result.cleaned, "我的歌曲");
        assert!(result.removed.is_empty());
    }

    #[test]
    fn strip_custom_words() {
        let opts = StripOptions {
            custom_stop_words: vec!["自定义词".to_string()],
            ..Default::default()
        };
        let result = strip_noise("歌曲 自定义词", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_combined() {
        let opts = StripOptions::default();
        let result = strip_noise("【洛天依 原创】Hi-Res 无损 4K 赛马", &opts);
        assert_eq!(result.cleaned, "赛马");
    }

    #[test]
    fn strip_preserves_content_no_quality_tags() {
        let opts = StripOptions {
            strip_quality_tags: false,
            ..Default::default()
        };
        let result = strip_noise("歌曲 4K", &opts);
        assert_eq!(result.cleaned, "歌曲 4K");
    }

    #[test]
    fn strip_nested_parens() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲（活动（内层）名称）", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_unclosed_parens_preserved() {
        let opts = StripOptions::default();
        // 未闭合括号 - 内容保留（避免数据丢失）
        let result = strip_noise("歌曲（未闭合", &opts);
        assert_eq!(result.cleaned, "歌曲（未闭合");
    }

    #[test]
    fn strip_bilibili_source() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲 bilibili 4K", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_user_example() {
        // 用户示例：去除"在百万级播音室大声听"
        let opts = StripOptions::default();
        let result = strip_noise("歌曲 在百万级播音室大声听", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }

    #[test]
    fn strip_collapse_whitespace() {
        let opts = StripOptions::default();
        let result = strip_noise("歌曲    4K   无损", &opts);
        assert_eq!(result.cleaned, "歌曲");
    }
}
