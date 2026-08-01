//! 主题常量 — 颜色、字体、间距
//!
//! 1:1 复刻 Tauri 版 CSS 变量。所有颜色用 Hsla struct literal（const 上下文）。

use gpui::Hsla;

/// 主题色板（与 Tauri 版 `:root` CSS 变量一致）
pub struct Theme;

impl Theme {
    // ---- Rio 主色 ----
    /// #39c5bb — Rio 青绿（标题栏/边框/主色）
    pub const RIO_BLUE: Hsla = Hsla { h: 176.0 / 360.0, s: 0.55, l: 0.50, a: 1.0 };
    /// #5dd6cd — 浅主色
    pub const RIO_BLUE_LIGHT: Hsla = Hsla { h: 176.0 / 360.0, s: 0.61, l: 0.60, a: 1.0 };
    /// #2a9b92 — 深主色（hover/active）
    pub const RIO_BLUE_DARK: Hsla = Hsla { h: 175.0 / 360.0, s: 0.58, l: 0.39, a: 1.0 };
    /// #e6f7f5 — 极浅主色（hover 底色）
    pub const RIO_BLUE_SUBTLE: Hsla = Hsla { h: 173.0 / 360.0, s: 0.47, l: 0.94, a: 1.0 };

    // ---- 背景色 ----
    /// #f5f6f8 — 主背景
    pub const BG: Hsla = Hsla { h: 220.0 / 360.0, s: 0.18, l: 0.97, a: 1.0 };
    /// #ffffff — 卡片/模态/标题栏按钮 hover
    pub const BG_ELEVATED: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 1.0 };
    /// #eceef2 — 表头/输入框/mem-switch/按钮底
    pub const BG_SUBTLE: Hsla = Hsla { h: 220.0 / 360.0, s: 0.18, l: 0.94, a: 1.0 };
    /// #f0f2f6 — hover 底色
    pub const BG_HOVER: Hsla = Hsla { h: 220.0 / 360.0, s: 0.25, l: 0.95, a: 1.0 };
    /// #eceef2 — confirm cancel 底
    pub const BG_MUTED: Hsla = Hsla { h: 220.0 / 360.0, s: 0.18, l: 0.94, a: 1.0 };

    // ---- 文本色 ----
    /// #1a1f2e — 主文字
    pub const TEXT: Hsla = Hsla { h: 222.0 / 360.0, s: 0.28, l: 0.14, a: 1.0 };
    /// #4a5468 — 次文字
    pub const TEXT_SECONDARY: Hsla = Hsla { h: 220.0 / 360.0, s: 0.18, l: 0.35, a: 1.0 };
    /// #8a92a3 — 暗淡文字
    pub const TEXT_DIM: Hsla = Hsla { h: 220.0 / 360.0, s: 0.11, l: 0.59, a: 1.0 };
    /// #ffffff — 主色上的白字
    pub const WHITE: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 1.0 };

    // ---- 边框 ----
    /// #e0e3ea — 表格/输入框边框
    pub const BORDER: Hsla = Hsla { h: 220.0 / 360.0, s: 0.18, l: 0.90, a: 1.0 };
    /// #ecedf0 — 行分隔/card 分隔
    pub const BORDER_LIGHT: Hsla = Hsla { h: 220.0 / 360.0, s: 0.12, l: 0.93, a: 1.0 };

    // ---- S30S 橙（SD 卡标识）----
    /// #ff6a00
    pub const S30S_ORANGE: Hsla = Hsla { h: 25.0 / 360.0, s: 1.0, l: 0.50, a: 1.0 };
    /// #fff0e0
    pub const S30S_ORANGE_SUBTLE: Hsla = Hsla { h: 30.0 / 360.0, s: 1.0, l: 0.94, a: 1.0 };

    // ---- 状态色 ----
    /// #c93b3b — 错误红
    pub const ERROR: Hsla = Hsla { h: 0.0, s: 0.56, l: 0.51, a: 1.0 };
    /// #fde8e5 — 浅红底
    pub const ACCENT_SOFT: Hsla = Hsla { h: 7.0 / 360.0, s: 0.80, l: 0.94, a: 1.0 };
    /// #1a9e5e — 成功绿
    pub const SUCCESS: Hsla = Hsla { h: 144.0 / 360.0, s: 0.73, l: 0.36, a: 1.0 };
    /// #d68a00 — 警告橙
    pub const WARNING: Hsla = Hsla { h: 39.0 / 360.0, s: 1.0, l: 0.42, a: 1.0 };

    // ---- 半透明覆盖色 ----
    /// rgba(0,0,0,0.18) — 标题栏按钮 hover
    pub const TITLEBAR_BTN_HOVER: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.18 };
    /// rgba(0,0,0,0.28) — 标题栏按钮 active
    pub const TITLEBAR_BTN_ACTIVE: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.28 };
    /// #e81123 — 关闭按钮 hover
    pub const CLOSE_BTN_HOVER: Hsla = Hsla { h: 355.0 / 360.0, s: 0.84, l: 0.48, a: 1.0 };
    /// rgba(57,197,187,0.08) — 表格行 hover
    pub const ROW_HOVER: Hsla = Hsla { h: 176.0 / 360.0, s: 0.55, l: 0.50, a: 0.08 };
    /// rgba(57,197,187,0.12) — 行 checked 底
    pub const ROW_CHECKED: Hsla = Hsla { h: 176.0 / 360.0, s: 0.55, l: 0.50, a: 0.12 };
    /// rgba(57,197,187,0.20) — 行 active.checked 底
    pub const ROW_ACTIVE_CHECKED: Hsla = Hsla { h: 176.0 / 360.0, s: 0.55, l: 0.50, a: 0.20 };
    /// rgba(20,30,50,0.4) — modal 遮罩
    pub const MODAL_OVERLAY: Hsla = Hsla { h: 220.0 / 360.0, s: 0.30, l: 0.14, a: 0.4 };
    /// rgba(0,0,0,0.35) — confirm 遮罩
    pub const CONFIRM_OVERLAY: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.35 };

    // ---- 字号（px）----
    pub const FONT_10: f32 = 10.0;
    pub const FONT_10_5: f32 = 10.5;
    pub const FONT_11: f32 = 11.0;
    pub const FONT_11_5: f32 = 11.5;
    pub const FONT_12: f32 = 12.0;
    pub const FONT_12_5: f32 = 12.5;
    pub const FONT_13: f32 = 13.0;
    pub const FONT_13_5: f32 = 13.5;
    pub const FONT_14: f32 = 14.0;
    pub const FONT_14_5: f32 = 14.5;
    pub const FONT_15: f32 = 15.0;
    pub const FONT_15_5: f32 = 15.5;
    pub const FONT_16: f32 = 16.0;
    pub const FONT_17: f32 = 17.0;
    pub const FONT_20: f32 = 20.0;
    pub const FONT_22: f32 = 22.0;
    pub const FONT_24: f32 = 24.0;

    // ---- 间距（px）----
    pub const SP_1: f32 = 1.0;
    pub const SP_2: f32 = 2.0;
    pub const SP_3: f32 = 3.0;
    pub const SP_4: f32 = 4.0;
    pub const SP_5: f32 = 5.0;
    pub const SP_6: f32 = 6.0;
    pub const SP_8: f32 = 8.0;
    pub const SP_10: f32 = 10.0;
    pub const SP_12: f32 = 12.0;
    pub const SP_14: f32 = 14.0;
    pub const SP_16: f32 = 16.0;
    pub const SP_18: f32 = 18.0;
    pub const SP_20: f32 = 20.0;
    pub const SP_24: f32 = 24.0;
    pub const SP_28: f32 = 28.0;
    pub const SP_32: f32 = 32.0;

    // ---- 圆角（px）----
    pub const RADIUS_XS: f32 = 3.0;
    pub const RADIUS_SM: f32 = 4.0;
    pub const RADIUS_MD: f32 = 6.0;
    pub const RADIUS_LG: f32 = 8.0;

    // ---- 组件高度（px）----
    pub const TITLEBAR_H: f32 = 28.0;
    pub const PLAYER_H: f32 = 48.0;
    pub const STATUS_BAR_H: f32 = 26.0;
    pub const PAGE_SIZE: usize = 10;
}
