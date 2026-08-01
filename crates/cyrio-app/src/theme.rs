//! 主题：颜色、间距、简单动效
//!
//! 配色对齐 Tauri 版 styles.css（白色朴素主题）：
//! - 主背景 #f5f6f8，卡片白底，主色 #39c5bb
//! - 去渐变、减阴影、小圆角

use egui::{Color32, Context, CornerRadius, Stroke, Vec2};

// ===== 背景色 =====

/// 主背景（.launcher bg）
pub const RIO_BG: Color32 = Color32::from_rgb(0xF5, 0xF6, 0xF8);

/// 卡片/弹窗白底（.bg-elevated）
pub const RIO_CONTENT_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);

/// 工具栏/卡片底色（.bg-subtle）
pub const RIO_BG_SUBTLE: Color32 = Color32::from_rgb(0xEC, 0xEE, 0xF2);

/// hover 底色（.bg-hover）
pub const RIO_BG_HOVER: Color32 = Color32::from_rgb(0xF0, 0xF2, 0xF6);

// ===== 文字色 =====

/// 主文字（.text）
pub const RIO_TEXT: Color32 = Color32::from_rgb(0x1A, 0x1F, 0x2E);

/// 次级文字（.text-secondary）
pub const RIO_TEXT_SECONDARY: Color32 = Color32::from_rgb(0x4A, 0x54, 0x68);

/// 暗淡文字（.text-dim）
pub const RIO_TEXT_DIM: Color32 = Color32::from_rgb(0x8A, 0x92, 0xA3);

// ===== 边框 =====

/// 标准边框（.border）
pub const RIO_BORDER: Color32 = Color32::from_rgb(0xE0, 0xE3, 0xEA);

/// 浅边框（.border-light）
pub const RIO_BORDER_LIGHT: Color32 = Color32::from_rgb(0xEC, 0xED, 0xF0);

// ===== Rio 主色 (#39c5bb 青绿色) =====

/// 主色
pub const RIO_BLUE: Color32 = Color32::from_rgb(0x39, 0xC5, 0xBB);

/// 浅主色
pub const RIO_BLUE_LIGHT: Color32 = Color32::from_rgb(0x5D, 0xD6, 0xCD);

/// 主色按下态（更深）
pub const RIO_BLUE_PRESSED: Color32 = Color32::from_rgb(0x2A, 0x9B, 0x92);

/// 选中淡底色
pub const RIO_SELECTED_BG: Color32 = Color32::from_rgb(0xE6, 0xF7, 0xF5);

/// 主色 hover 底色
pub const RIO_BLUE_HOVER: Color32 = Color32::from_rgb(0xD0, 0xF0, 0xEC);

// ===== S30S 橙（SD 卡标识） =====

pub const RIO_S30S_ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x00);
pub const RIO_S30S_ORANGE_SUBTLE: Color32 = Color32::from_rgb(0xFF, 0xF0, 0xE0);

// ===== 强调/状态色 =====

/// 危险色（删除）
pub const RIO_DANGER: Color32 = Color32::from_rgb(0xC9, 0x3B, 0x3B);

/// 危险淡底色
pub const RIO_ACCENT_SOFT: Color32 = Color32::from_rgb(0xFD, 0xE8, 0xE5);

/// 成功色
pub const RIO_SUCCESS: Color32 = Color32::from_rgb(0x1A, 0x9E, 0x5E);

/// 警告色
pub const RIO_WARNING: Color32 = Color32::from_rgb(0xD6, 0x8A, 0x00);

// ===== 选中行半透明色（对齐 Tauri rgba(57,197,187,0.12)） =====

/// 选中行背景（约 12% 透明度青绿）
pub const RIO_CHECKED_BG: Color32 = Color32::from_rgba_premultiplied(57, 197, 187, 31);

/// hover 行背景（约 8% 透明度青绿）
pub const RIO_HOVER_BG: Color32 = Color32::from_rgba_premultiplied(57, 197, 187, 20);

/// 模态遮罩
pub const RIO_OVERLAY: Color32 = Color32::from_rgba_premultiplied(20, 30, 50, 100);

/// 加载遮罩
pub const RIO_LOADING_OVERLAY: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 190);

/// notice toast 深色底
pub const RIO_NOTICE_BG: Color32 = Color32::from_rgb(0x1A, 0x1F, 0x2E);

/// 颜色插值（用于 hover 动画）
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

/// 应用整体主题到 egui Context
pub fn apply_theme(ctx: &Context) {
    // 显式设置主题为 Light，避免 System 主题检测导致使用未修改的 style
    ctx.set_theme(egui::Theme::Light);

    // 使用 all_styles_mut 同时修改 dark 和 light style，
    // 防止系统主题切换时使用未修改的 style
    ctx.all_styles_mut(|style| {
        // 面板背景
        style.visuals.panel_fill = RIO_BG;
        style.visuals.window_fill = RIO_CONTENT_BG;
        style.visuals.extreme_bg_color = RIO_CONTENT_BG;
        style.visuals.faint_bg_color = RIO_BG_SUBTLE;

        // 控件背景（weak_bg_fill 是 Button 等控件实际使用的背景色；
        // bg_fill 是 checkbox 等强背景色。两者都需覆盖，否则会用 Visuals::light() 默认值）
        style.visuals.widgets.noninteractive.weak_bg_fill = RIO_BG_SUBTLE;
        style.visuals.widgets.noninteractive.bg_fill = RIO_BG_SUBTLE;
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, RIO_TEXT);
        style.visuals.widgets.inactive.weak_bg_fill = RIO_BG_SUBTLE;
        style.visuals.widgets.inactive.bg_fill = RIO_BG_SUBTLE;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, RIO_TEXT_SECONDARY);
        style.visuals.widgets.hovered.weak_bg_fill = RIO_BG_HOVER;
        style.visuals.widgets.hovered.bg_fill = RIO_BG_HOVER;
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, RIO_TEXT);
        // 按下态：淡灰底 + 深色文字（避免深青底+白字导致按钮变黑、文字消失）
        style.visuals.widgets.active.weak_bg_fill = RIO_BORDER;
        style.visuals.widgets.active.bg_fill = RIO_BORDER;
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, RIO_TEXT);
        style.visuals.widgets.open.weak_bg_fill = RIO_BG_SUBTLE;
        style.visuals.widgets.open.bg_fill = RIO_BG_SUBTLE;
        style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, RIO_TEXT);

        // 选中态
        style.visuals.selection.bg_fill = RIO_BLUE;
        style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);

        // 圆角（radius-xs=3, radius-sm=4, radius-md=6）
        let cr3 = CornerRadius::same(3);
        let cr4 = CornerRadius::same(4);
        style.visuals.window_corner_radius = cr4;
        style.visuals.menu_corner_radius = cr4;
        style.visuals.widgets.noninteractive.corner_radius = cr3;
        style.visuals.widgets.inactive.corner_radius = cr3;
        style.visuals.widgets.hovered.corner_radius = cr3;
        style.visuals.widgets.active.corner_radius = cr3;
        style.visuals.widgets.open.corner_radius = cr3;

        // 边框
        style.visuals.window_stroke = Stroke::new(1.0, RIO_BORDER);

        // 紧凑间距
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        style.spacing.indent = 16.0;
    });
}

/// 启用简单缓动动画（~180ms，对应 Tauri cubic-bezier(0.25,0.46,0.45,0.94)）
pub fn configure_animation(ctx: &Context) {
    ctx.all_styles_mut(|style| {
        style.animation_time = 0.2;
    });
}
