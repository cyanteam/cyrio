//! 日语假名→罗马字（Romaji）转换
//!
//! 将平假名/片假名转为罗马字，便于 Rio 设备（字库有限）显示。
//! 用户要求"看得懂即可，不必强行追求读音"，主要给用户做区分，比乱码强一点。
//!
//! # 支持范围
//! - 平假名（U+3040–U+309F）
//! - 片假名（U+30A0–U+30FF）
//! - 拗音（きゃ/kya 等组合假名）
//! - 促音（っ/ッ → 双辅音）
//! - 长音符号（ー → 重复前一元音）
//! - 浊音/半浊音（が/ga、ぱ/pa 等）
//!
//! # 不支持
//! - 汉字→日语读音（太复杂，且很多汉字与中文共享，pinyin crate 已部分覆盖）

// ============================================================================
// 假名→罗马字查找表
// ============================================================================

/// 平假名→罗马字（基本音）
const HIRAGANA_BASE: &[(char, &str)] = &[
    // 清音
    ('あ', "a"), ('い', "i"), ('う', "u"), ('え', "e"), ('お', "o"),
    ('か', "ka"), ('き', "ki"), ('く', "ku"), ('け', "ke"), ('こ', "ko"),
    ('さ', "sa"), ('し', "shi"), ('す', "su"), ('せ', "se"), ('そ', "so"),
    ('た', "ta"), ('ち', "chi"), ('つ', "tsu"), ('て', "te"), ('と', "to"),
    ('な', "na"), ('に', "ni"), ('ぬ', "nu"), ('ね', "ne"), ('の', "no"),
    ('は', "ha"), ('ひ', "hi"), ('ふ', "fu"), ('へ', "he"), ('ほ', "ho"),
    ('ま', "ma"), ('み', "mi"), ('む', "mu"), ('め', "me"), ('も', "mo"),
    ('や', "ya"), ('ゆ', "yu"), ('よ', "yo"),
    ('ら', "ra"), ('り', "ri"), ('る', "ru"), ('れ', "re"), ('ろ', "ro"),
    ('わ', "wa"), ('を', "wo"), ('ん', "n"),
    // 浊音
    ('が', "ga"), ('ぎ', "gi"), ('ぐ', "gu"), ('げ', "ge"), ('ご', "go"),
    ('ざ', "za"), ('じ', "ji"), ('ず', "zu"), ('ぜ', "ze"), ('ぞ', "zo"),
    ('だ', "da"), ('ぢ', "di"), ('づ', "du"), ('で', "de"), ('ど', "do"),
    ('ば', "ba"), ('び', "bi"), ('ぶ', "bu"), ('べ', "be"), ('ぼ', "bo"),
    // 半浊音
    ('ぱ', "pa"), ('ぴ', "pi"), ('ぷ', "pu"), ('ぺ', "pe"), ('ぽ', "po"),
    // 小假名（独立使用时）
    ('ぁ', "a"), ('ぃ', "i"), ('ぅ', "u"), ('ぇ', "e"), ('ぉ', "o"),
    ('ゃ', "ya"), ('ゅ', "yu"), ('ょ', "yo"),
    ('ゎ', "wa"),
    // 其他
    ('ゐ', "wi"), ('ゑ', "we"),
];

/// 片假名→罗马字（基本音）
const KATAKANA_BASE: &[(char, &str)] = &[
    // 清音
    ('ア', "a"), ('イ', "i"), ('ウ', "u"), ('エ', "e"), ('オ', "o"),
    ('カ', "ka"), ('キ', "ki"), ('ク', "ku"), ('ケ', "ke"), ('コ', "ko"),
    ('サ', "sa"), ('シ', "shi"), ('ス', "su"), ('セ', "se"), ('ソ', "so"),
    ('タ', "ta"), ('チ', "chi"), ('ツ', "tsu"), ('テ', "te"), ('ト', "to"),
    ('ナ', "na"), ('ニ', "ni"), ('ヌ', "nu"), ('ネ', "ne"), ('ノ', "no"),
    ('ハ', "ha"), ('ヒ', "hi"), ('フ', "fu"), ('ヘ', "he"), ('ホ', "ho"),
    ('マ', "ma"), ('ミ', "mi"), ('ム', "mu"), ('メ', "me"), ('モ', "mo"),
    ('ヤ', "ya"), ('ユ', "yu"), ('ヨ', "yo"),
    ('ラ', "ra"), ('リ', "ri"), ('ル', "ru"), ('レ', "re"), ('ロ', "ro"),
    ('ワ', "wa"), ('ヲ', "wo"), ('ン', "n"),
    // 浊音
    ('ガ', "ga"), ('ギ', "gi"), ('グ', "gu"), ('ゲ', "ge"), ('ゴ', "go"),
    ('ザ', "za"), ('ジ', "ji"), ('ズ', "zu"), ('ゼ', "ze"), ('ゾ', "zo"),
    ('ダ', "da"), ('ヂ', "di"), ('ヅ', "du"), ('デ', "de"), ('ド', "do"),
    ('バ', "ba"), ('ビ', "bi"), ('ブ', "bu"), ('ベ', "be"), ('ボ', "bo"),
    // 半浊音
    ('パ', "pa"), ('ピ', "pi"), ('プ', "pu"), ('ペ', "pe"), ('ポ', "po"),
    // 小假名
    ('ァ', "a"), ('ィ', "i"), ('ゥ', "u"), ('ェ', "e"), ('ォ', "o"),
    ('ャ', "ya"), ('ュ', "yu"), ('ョ', "yo"),
    ('ヮ', "wa"),
    // 其他
    ('ヴ', "vu"), ('ヵ', "ka"), ('ヶ', "ke"),
];

/// 拗音组合通过 `lookup_yoon` 函数的 match 语句实现（见下方）

/// 小や/ゆ/よ（平假名）
const YOON_SMALL_HIRA: [char; 3] = ['ゃ', 'ゅ', 'ょ'];

/// 小や/ゆ/よ（片假名）
const YOON_SMALL_KATA: [char; 3] = ['ャ', 'ュ', 'ョ'];

/// 促音标记
const SOKUON_HIRA: char = 'っ';
const SOKUON_KATA: char = 'ッ';

/// 长音符号
const PROLONGED_MARK: char = 'ー';

// ============================================================================
// 查找辅助函数
// ============================================================================

/// 查找平假名对应的基本罗马字
fn lookup_hiragana(ch: char) -> Option<&'static str> {
    HIRAGANA_BASE
        .iter()
        .find(|(c, _)| *c == ch)
        .map(|(_, s)| *s)
}

/// 查找片假名对应的基本罗马字
fn lookup_katakana(ch: char) -> Option<&'static str> {
    KATAKANA_BASE
        .iter()
        .find(|(c, _)| *c == ch)
        .map(|(_, s)| *s)
}

/// 查找假名（平假名或片假名）对应的基本罗马字
fn lookup_kana(ch: char) -> Option<&'static str> {
    lookup_hiragana(ch).or_else(|| lookup_katakana(ch))
}

/// 将假名字符串分割为音节块
///
/// 每个音节块是一个 1-3 字符的字符串，包含：
/// - 单个假名（如 "か"）
/// - 假名 + 小拗音（如 "きゃ"）
/// - 促音 + 假名（如 "っか"）
/// - 促音 + 假名 + 小拗音（如 "っきゃ"）
/// - 长音符号（"ー"）附加到前一音节
///
/// 返回的每个块可独立通过 `kana_to_romaji` 转换为罗马字。
pub fn kana_syllables(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut syllables: Vec<String> = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // 促音（っ/ッ）→ 与下一个假名组合
        if is_sokuon(ch) {
            let mut syllable = String::new();
            syllable.push(ch);
            i += 1;
            // 添加下一个假名
            if i < chars.len() && is_kana(chars[i]) && !is_sokuon(chars[i]) {
                syllable.push(chars[i]);
                i += 1;
                // 检查是否有小拗音
                if i < chars.len() && is_small_yoon(chars[i]).is_some() {
                    syllable.push(chars[i]);
                    i += 1;
                }
            }
            syllables.push(syllable);
            continue;
        }

        // 长音符号（ー）→ 附加到前一音节
        if is_prolonged(ch) {
            if let Some(last) = syllables.last_mut() {
                last.push(ch);
            } else {
                // 没有前一音节，单独作为一个音节
                syllables.push(ch.to_string());
            }
            i += 1;
            continue;
        }

        // 基本假名
        if is_kana(ch) {
            let mut syllable = String::new();
            syllable.push(ch);
            i += 1;
            // 检查是否有小拗音
            if i < chars.len() && is_small_yoon(chars[i]).is_some() {
                syllable.push(chars[i]);
                i += 1;
            }
            // 检查是否有长音符号
            if i < chars.len() && is_prolonged(chars[i]) {
                syllable.push(chars[i]);
                i += 1;
            }
            syllables.push(syllable);
            continue;
        }

        // 非假名字符 → 单独一个音节
        syllables.push(ch.to_string());
        i += 1;
    }

    syllables
}

/// 判断字符是否为小や/ゆ/よ（平假名或片假名）
fn is_small_yoon(ch: char) -> Option<usize> {
    // 返回 Some(0)=ya, Some(1)=yu, Some(2)=yo
    YOON_SMALL_HIRA
        .iter()
        .position(|&c| c == ch)
        .or_else(|| YOON_SMALL_KATA.iter().position(|&c| c == ch))
}

/// 判断字符是否为促音（っ/ッ）
fn is_sokuon(ch: char) -> bool {
    ch == SOKUON_HIRA || ch == SOKUON_KATA
}

/// 判断字符是否为长音符号（ー）
fn is_prolonged(ch: char) -> bool {
    ch == PROLONGED_MARK
}

/// 判断字符是否为日语假名
pub fn is_kana(ch: char) -> bool {
    matches!(ch,
        '\u{3040}'..='\u{309F}'  // 平假名
        | '\u{30A0}'..='\u{30FF}'  // 片假名
    )
}

// ============================================================================
// 核心转换函数
// ============================================================================

/// 将假名字符串转换为罗马字
///
/// 处理顺序：
/// 1. 逐字符遍历
/// 2. 遇到促音（っ/ッ）→ 记录，双写下一个辅音
/// 3. 遇到基本假名 → 检查下一个字符是否为小や/ゆ/よ（拗音）
///    - 是 → 输出组合罗马字
///    - 否 → 输出基本罗马字
/// 4. 遇到长音符号（ー）→ 重复前一罗马字的最后一个元音
/// 5. 遇到小假名（非拗音上下文）→ 输出对应元音
///
/// # 示例
/// ```
/// use cyrio_text::kana::kana_to_romaji;
/// assert_eq!(kana_to_romaji("ありがとう"), "arigatou");
/// assert_eq!(kana_to_romaji("サクラ"), "sakura");
/// assert_eq!(kana_to_romaji("きょう"), "kyou");
/// assert_eq!(kana_to_romaji("がっこう"), "gakkou");
/// ```
pub fn kana_to_romaji(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::new();
    let mut i = 0;
    let mut last_vowel: Option<char> = None;

    while i < chars.len() {
        let ch = chars[i];

        // 1. 促音（っ/ッ）→ 双写下一个辅音
        if is_sokuon(ch) {
            // 查看下一个字符的罗马字首字母
            if i + 1 < chars.len() {
                if let Some(next_romaji) = lookup_kana(chars[i + 1]) {
                    if let Some(first_ch) = next_romaji.chars().next() {
                        // ん 后面的促音不需要双写 n
                        if first_ch != 'n' && !result.ends_with('n') {
                            result.push(first_ch);
                        }
                    }
                }
            }
            i += 1;
            continue;
        }

        // 2. 长音符号（ー）→ 重复前一元音
        if is_prolonged(ch) {
            if let Some(v) = last_vowel {
                result.push(v);
            }
            i += 1;
            continue;
        }

        // 3. 基本假名
        if let Some(romaji) = lookup_kana(ch) {
            // 检查是否为拗音（当前假名 + 小や/ゆ/よ）
            if i + 1 < chars.len() {
                if let Some(yoon_idx) = is_small_yoon(chars[i + 1]) {
                    // 查找拗音组合
                    if let Some(combined) = lookup_yoon(ch, yoon_idx) {
                        result.push_str(combined);
                        last_vowel = combined.chars().last();
                        i += 2; // 跳过当前假名和小や/ゆ/よ
                        continue;
                    }
                }
            }

            // 普通假名
            // ん 的特殊处理：在 b/p/m 前变为 m
            if ch == 'ん' || ch == 'ン' {
                if i + 1 < chars.len() {
                    if let Some(next_romaji) = lookup_kana(chars[i + 1]) {
                        if let Some(first_ch) = next_romaji.chars().next() {
                            if first_ch == 'b' || first_ch == 'p' || first_ch == 'm' {
                                result.push('m');
                                last_vowel = Some('n');
                                i += 1;
                                continue;
                            }
                        }
                    }
                }
            }

            result.push_str(romaji);
            last_vowel = romaji.chars().last();
            i += 1;
            continue;
        }

        // 4. 非假名字符 → 原样保留
        result.push(ch);
        i += 1;
    }

    result
}

/// 查找拗音组合
///
/// `base` 是基础假名（如 き），`yoon_idx` 是 0=ya, 1=yu, 2=yo
fn lookup_yoon(base: char, yoon_idx: usize) -> Option<&'static str> {
    // YOON_COMBINATIONS 的格式是 (char, &str, &str, &str) 但 Rust 不直接支持
    // 这里用硬编码的 match 代替
    let key = (base, yoon_idx);
    match key {
        // き行
        ('き', 0) => Some("kya"), ('き', 1) => Some("kyu"), ('き', 2) => Some("kyo"),
        ('ぎ', 0) => Some("gya"), ('ぎ', 1) => Some("gyu"), ('ぎ', 2) => Some("gyo"),
        // し行
        ('し', 0) => Some("sha"), ('し', 1) => Some("shu"), ('し', 2) => Some("sho"),
        ('じ', 0) => Some("ja"), ('じ', 1) => Some("ju"), ('じ', 2) => Some("jo"),
        // ち行
        ('ち', 0) => Some("cha"), ('ち', 1) => Some("chu"), ('ち', 2) => Some("cho"),
        ('ぢ', 0) => Some("dya"), ('ぢ', 1) => Some("dyu"), ('ぢ', 2) => Some("dyo"),
        // に行
        ('に', 0) => Some("nya"), ('に', 1) => Some("nyu"), ('に', 2) => Some("nyo"),
        // ひ行
        ('ひ', 0) => Some("hya"), ('ひ', 1) => Some("hyu"), ('ひ', 2) => Some("hyo"),
        ('び', 0) => Some("bya"), ('び', 1) => Some("byu"), ('び', 2) => Some("byo"),
        ('ぴ', 0) => Some("pya"), ('ぴ', 1) => Some("pyu"), ('ぴ', 2) => Some("pyo"),
        // み行
        ('み', 0) => Some("mya"), ('み', 1) => Some("myu"), ('み', 2) => Some("myo"),
        // り行
        ('り', 0) => Some("rya"), ('り', 1) => Some("ryu"), ('り', 2) => Some("ryo"),
        // 片假名同样支持
        ('キ', 0) => Some("kya"), ('キ', 1) => Some("kyu"), ('キ', 2) => Some("kyo"),
        ('ギ', 0) => Some("gya"), ('ギ', 1) => Some("gyu"), ('ギ', 2) => Some("gyo"),
        ('シ', 0) => Some("sha"), ('シ', 1) => Some("shu"), ('シ', 2) => Some("sho"),
        ('ジ', 0) => Some("ja"), ('ジ', 1) => Some("ju"), ('ジ', 2) => Some("jo"),
        ('チ', 0) => Some("cha"), ('チ', 1) => Some("chu"), ('チ', 2) => Some("cho"),
        ('ニ', 0) => Some("nya"), ('ニ', 1) => Some("nyu"), ('ニ', 2) => Some("nyo"),
        ('ヒ', 0) => Some("hya"), ('ヒ', 1) => Some("hyu"), ('ヒ', 2) => Some("hyo"),
        ('ビ', 0) => Some("bya"), ('ビ', 1) => Some("byu"), ('ビ', 2) => Some("byo"),
        ('ピ', 0) => Some("pya"), ('ピ', 1) => Some("pyu"), ('ピ', 2) => Some("pyo"),
        ('ミ', 0) => Some("mya"), ('ミ', 1) => Some("myu"), ('ミ', 2) => Some("myo"),
        ('リ', 0) => Some("rya"), ('リ', 1) => Some("ryu"), ('リ', 2) => Some("ryo"),
        _ => None,
    }
}

/// 判断字符串中是否包含假名字符
pub fn contains_kana(s: &str) -> bool {
    s.chars().any(is_kana)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiragana_basic() {
        assert_eq!(kana_to_romaji("あいうえお"), "aiueo");
        assert_eq!(kana_to_romaji("かきくけこ"), "kakikukeko");
        assert_eq!(kana_to_romaji("さしすせそ"), "sashisuseso");
        assert_eq!(kana_to_romaji("たちつてと"), "tachitsuteto");
        assert_eq!(kana_to_romaji("なにぬねの"), "naninuneno");
        assert_eq!(kana_to_romaji("はひふへほ"), "hahifuheho");
        assert_eq!(kana_to_romaji("まみむめも"), "mamimumemo");
        assert_eq!(kana_to_romaji("やゆよ"), "yayuyo");
        assert_eq!(kana_to_romaji("らりるれろ"), "rarirurero");
    }

    #[test]
    fn hiragana_dakuten() {
        assert_eq!(kana_to_romaji("がぎぐげご"), "gagigugego");
        assert_eq!(kana_to_romaji("ざじずぜぞ"), "zajizuzezo");
        assert_eq!(kana_to_romaji("だぢづでど"), "dadidudedo");
        assert_eq!(kana_to_romaji("ばびぶべぼ"), "babibubebo");
        assert_eq!(kana_to_romaji("ぱぴぷぺぽ"), "papipupepo");
    }

    #[test]
    fn katakana_basic() {
        assert_eq!(kana_to_romaji("アイウエオ"), "aiueo");
        assert_eq!(kana_to_romaji("カキクケコ"), "kakikukeko");
        assert_eq!(kana_to_romaji("サシスセソ"), "sashisuseso");
        assert_eq!(kana_to_romaji("タチツテト"), "tachitsuteto");
    }

    #[test]
    fn yoon_combinations() {
        assert_eq!(kana_to_romaji("きゃ"), "kya");
        assert_eq!(kana_to_romaji("きゅ"), "kyu");
        assert_eq!(kana_to_romaji("きょ"), "kyo");
        assert_eq!(kana_to_romaji("しゃ"), "sha");
        assert_eq!(kana_to_romaji("しゅ"), "shu");
        assert_eq!(kana_to_romaji("しょ"), "sho");
        assert_eq!(kana_to_romaji("ちゃ"), "cha");
        assert_eq!(kana_to_romaji("ちゅ"), "chu");
        assert_eq!(kana_to_romaji("ちょ"), "cho");
        assert_eq!(kana_to_romaji("にゃ"), "nya");
        assert_eq!(kana_to_romaji("りゃ"), "rya");
        // 片假名拗音
        assert_eq!(kana_to_romaji("キャ"), "kya");
        assert_eq!(kana_to_romaji("シュ"), "shu");
        assert_eq!(kana_to_romaji("チョ"), "cho");
    }

    #[test]
    fn sokuon() {
        assert_eq!(kana_to_romaji("がっこう"), "gakkou");
        assert_eq!(kana_to_romaji("まった"), "matta");
        assert_eq!(kana_to_romaji("きって"), "kitte");
        assert_eq!(kana_to_romaji("サッカ"), "sakka");
    }

    #[test]
    fn prolonged_mark() {
        assert_eq!(kana_to_romaji("すー"), "suu");
        assert_eq!(kana_to_romaji("かー"), "kaa");
        assert_eq!(kana_to_romaji("コー"), "koo");
    }

    #[test]
    fn n_before_bpm() {
        assert_eq!(kana_to_romaji("さんぽ"), "sampo");
        assert_eq!(kana_to_romaji("しんぶん"), "shimbun");
    }

    #[test]
    fn mixed_kana() {
        assert_eq!(kana_to_romaji("ありがとう"), "arigatou");
        assert_eq!(kana_to_romaji("サクラ"), "sakura");
        assert_eq!(kana_to_romaji("おはよう"), "ohayou");
    }

    #[test]
    fn non_kana_passthrough() {
        assert_eq!(kana_to_romaji("hello"), "hello");
        assert_eq!(kana_to_romaji("test 123"), "test 123");
    }

    #[test]
    fn mixed_text() {
        // 混合假名和非假名
        assert_eq!(kana_to_romaji("Song ありがとう"), "Song arigatou");
    }

    #[test]
    fn contains_kana_check() {
        assert!(contains_kana("ありがとう"));
        assert!(contains_kana("サクラ"));
        assert!(!contains_kana("hello"));
        assert!(!contains_kana("中文"));
    }

    #[test]
    fn empty_string() {
        assert_eq!(kana_to_romaji(""), "");
    }
}
