//! 关于页面
//!
//! 1:1 复刻 Tauri 关于页面：
//! - Cyrio logo + 版本号
//! - 功能简介
//! - 技术栈信息

use crate::state::CyrioApp;
use crate::theme::Theme;
use gpui::*;

pub fn render_about_view(_app: &mut CyrioApp, _cx: &mut Context<CyrioApp>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(16.0))
        // Logo 球
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(80.0))
                .h(px(80.0))
                .rounded_full()
                .bg(Theme::RIO_BLUE)
                .text_color(Theme::WHITE)
                .text_size(px(Theme::FONT_20))
                .font_weight(FontWeight::BOLD)
                .child("♪")
        )
        // 标题
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_20))
                .font_weight(FontWeight::BOLD)
                .child("Cyrio")
        )
        // 版本号
        .child(
            div()
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(Theme::FONT_12))
                .child("版本 0.1.0 (GPUI)")
        )
        // 功能简介
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_12))
                .child("Rio S30S / S35S MP3 播放器管理工具")
        )
        // 技术栈
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .mt(px(16.0))
                .p(px(16.0))
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(Theme::RADIUS_SM))
                .child(render_tech_row("界面引擎", "GPUI (Zed)"))
                .child(render_tech_row("USB 通信", "nusb + cyrio-core"))
                .child(render_tech_row("音频播放", "cyrio-audio (rodio)"))
                .child(render_tech_row("异步运行时", "smol"))
        )
        // GitHub 链接
        .child(
            div()
                .text_color(Theme::RIO_BLUE)
                .text_size(px(Theme::FONT_11))
                .child("Powered by Rust")
        )
}

fn render_tech_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w(px(280.0))
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_11))
                .child(label.to_string())
        )
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_11))
                .child(value.to_string())
        )
}
