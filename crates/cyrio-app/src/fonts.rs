//! 中文字体适配
//!
//! egui 默认字体不含 CJK 字形，中文会渲染成豆腐块。本模块负责：
//! 1. 嵌入子集化 CJK 字体（编译时 `include_bytes!`），作为 WASM 和无系统字体环境的保底
//! 2. 启动时尝试加载系统 CJK 字体（更高优先级），让桌面版使用原生 UI 字体
//!
//! ## 嵌入字体
//! `assets/CJKSubset.ttf` 由 Arial Unicode 子集化而来（8MB），覆盖：
//! 基本拉丁、拉丁补充、CJK 基本 + 扩展A、CJK 标点、半角全角、通用标点。
//!
//! ## 系统字体路径
//! - macOS：`/System/Library/Fonts/PingFang.ttc`、`Hiragino Sans GB.ttc`
//! - Windows：`C:\Windows\Fonts\msyh.ttc`（微软雅黑）
//! - Linux：`/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc` 等

use egui::epaint::text::{FontData, FontInsert, FontPriority, InsertFontFamily};
use egui::{Context, FontFamily};
use std::path::Path;
use std::sync::Arc;

/// 嵌入的子集化 CJK 字体（编译时打包进二进制）
const EMBEDDED_CJK_FONT: &[u8] = include_bytes!("../assets/CJKSubset.ttf");

/// 平台候选 CJK 字体路径（按优先级排序）
fn candidate_cjk_font_paths() -> Vec<&'static str> {
    let mut paths: Vec<&'static str> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        paths.push("/System/Library/Fonts/PingFang.ttc");
        paths.push("/System/Library/Fonts/Hiragino Sans GB.ttc");
        paths.push("/System/Library/Fonts/Supplemental/Arial Unicode.ttf");
    }

    #[cfg(target_os = "windows")]
    {
        paths.push("C:\\Windows\\Fonts\\msyh.ttc");
        paths.push("C:\\Windows\\Fonts\\msyh.ttf");
        paths.push("C:\\Windows\\Fonts\\simhei.ttf");
    }

    #[cfg(target_os = "linux")]
    {
        paths.push("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc");
        paths.push("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc");
        paths.push("/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc");
        paths.push("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc");
    }

    paths
}

/// 加载 CJK 字体并注册到 egui Context
///
/// 策略：
/// 1. 先注册嵌入字体（Lowest 优先级）作为保底——WASM 下唯一来源
/// 2. 再尝试加载系统字体（Highest 优先级）覆盖——桌面版使用原生 UI 字体
///
/// 调用时机：`CyrioApp::init_once` 中（在第一个 `ui` 帧之前）。
pub fn install_cjk_font(ctx: &Context) {
    // 1. 注册嵌入字体（保底）
    let embedded = FontData::from_static(EMBEDDED_CJK_FONT);
    ctx.add_font(FontInsert::new(
        "EmbeddedCJK",
        embedded,
        vec![
            InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Lowest,
            },
            InsertFontFamily {
                family: FontFamily::Monospace,
                priority: FontPriority::Lowest,
            },
        ],
    ));
    log::info!("Loaded embedded CJK subset font ({} bytes)", EMBEDDED_CJK_FONT.len());

    // 2. 尝试加载系统字体（更高优先级，覆盖嵌入字体）
    for path_str in candidate_cjk_font_paths() {
        let path = Path::new(path_str);
        if !path.exists() {
            continue;
        }
        match std::fs::read(path) {
            Ok(data) => {
                let font_data = FontData::from_owned(data);
                let insert = FontInsert::new(
                    "SystemCJK",
                    font_data,
                    vec![
                        InsertFontFamily {
                            family: FontFamily::Proportional,
                            priority: FontPriority::Highest,
                        },
                        InsertFontFamily {
                            family: FontFamily::Monospace,
                            priority: FontPriority::Lowest,
                        },
                    ],
                );
                ctx.add_font(insert);
                log::info!("Loaded system CJK font from {}", path_str);
                return;
            }
            Err(e) => {
                log::warn!("Failed to read CJK font {}: {}", path_str, e);
            }
        }
    }
    log::info!("No system CJK font found; using embedded subset font");
}

/// 配置默认字体大小与行距
///
/// 中文需要稍大的字号和更宽松的行距以保证可读性。
pub fn configure_typography(ctx: &Context) {
    ctx.global_style_mut(|style| {
        if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Body) {
            f.size = 13.0;
        }
        if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Button) {
            f.size = 13.0;
        }
        if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Small) {
            f.size = 11.0;
        }
        if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Heading) {
            f.size = 16.0;
        }
        if let Some(f) = style
            .text_styles
            .get_mut(&egui::TextStyle::Name(Arc::<str>::from("Tab")))
        {
            f.size = 13.0;
        }
        // 行距 1.4 倍对中英文混排友好
        style.spacing.item_spacing.y = 5.0;
        style.spacing.item_spacing.x = 7.0;
    });
}
