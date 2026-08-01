//! 设置页面
//!
//! 1:1 复刻 Tauri 设置页面：
//! - 上传设置（转拼音/去词/子选项）
//! - 自定义停用词
//! - WebDAV 设置

use crate::state::CyrioApp;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

pub fn render_settings_view(app: &mut CyrioApp, cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let settings = app.settings.clone();
    let webdav_running = app.webdav_running;

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap(px(12.0))
        .id("settings-scroll")
        .overflow_y_scroll()
        // 标题
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_14))
                .font_weight(FontWeight::SEMIBOLD)
                .child("设置")
        )
        // ---- 上传设置 ----
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(Theme::RADIUS_SM))
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_13))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("上传文本处理")
                )
                .child(render_toggle_row("转拼音", "标题含中文时自动转拼音", settings.upload_apply_slug, cx, |this, v| {
                    this.settings.upload_apply_slug = v;
                }))
                .child(render_toggle_row("去词", "去除括号/引号/质量标签等", settings.upload_apply_strip, cx, |this, v| {
                    this.settings.upload_apply_strip = v;
                }))
                .when(settings.upload_apply_strip, |this| {
                    this.child(
                        div()
                            .pl(px(20.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(render_toggle_row("去除括号内容", "(xxx) 型", settings.strip_parentheses, cx, |this, v| {
                                this.settings.strip_parentheses = v;
                            }))
                            .child(render_toggle_row("去除引号内容", "\"xxx\" 型", settings.strip_quotes, cx, |this, v| {
                                this.settings.strip_quotes = v;
                            }))
                            .child(render_toggle_row("去除质量标签", "320k/HQ/SQ 等", settings.strip_quality_tags, cx, |this, v| {
                                this.settings.strip_quality_tags = v;
                            }))
                    )
                })
        )
        // ---- 自定义停用词 ----
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(Theme::RADIUS_SM))
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_13))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("自定义停用词")
                )
                .child(
                    div()
                        .text_color(Theme::TEXT_DIM)
                        .text_size(px(Theme::FONT_11))
                        .child("每行一个词，去词操作时自动移除这些词")
                )
                .child(
                    div()
                        .w_full()
                        .min_h(px(80.0))
                        .p(px(8.0))
                        .rounded(px(Theme::RADIUS_SM))
                        .bg(Theme::BG_SUBTLE)
                        .border_1()
                        .border_color(Theme::BORDER)
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_12))
                        .child(if settings.custom_stop_words.is_empty() {
                            "输入停用词，每行一个...".to_string()
                        } else {
                            settings.custom_stop_words.clone()
                        })
                )
        )
        // ---- WebDAV 设置 ----
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(Theme::RADIUS_SM))
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_13))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("WebDAV 虚拟U盘")
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_color(Theme::TEXT_SECONDARY)
                                .text_size(px(Theme::FONT_12))
                                .child(if webdav_running { "运行中" } else { "已停止" })
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(28.0))
                                .px(px(16.0))
                                .rounded(px(Theme::RADIUS_SM))
                                .when(webdav_running, |this| {
                                    this.bg(Theme::ERROR).text_color(Theme::WHITE)
                                })
                                .when(!webdav_running, |this| {
                                    this.bg(Theme::RIO_BLUE).text_color(Theme::WHITE)
                                })
                                .text_size(px(Theme::FONT_12))
                                .child(if webdav_running { "停止" } else { "启动" })
                                .id("click-1").on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.webdav_running = !this.webdav_running;
                                    cx.notify();
                                }))
                        )
                )
        )
}

fn render_toggle_row(
    title: &str,
    desc: &str,
    checked: bool,
    cx: &mut Context<CyrioApp>,
    setter: impl Fn(&mut CyrioApp, bool) + 'static + Copy,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .h(px(32.0))
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_12))
                        .child(title.to_string())
                )
                .child(
                    div()
                        .text_color(Theme::TEXT_DIM)
                        .text_size(px(Theme::FONT_10))
                        .child(desc.to_string())
                )
        )
        .child(
            div()
                .w(px(36.0))
                .h(px(20.0))
                .rounded(px(10.0))
                .when(checked, |this| { this.bg(Theme::RIO_BLUE) })
                .when(!checked, |this| { this.bg(Theme::BG_MUTED) })
                .flex()
                .items_center()
                .when(checked, |this| { this.justify_end().pr(px(2.0)) })
                .when(!checked, |this| { this.pl(px(2.0)) })
                .child(
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded_full()
                        .bg(Theme::WHITE)
                )
                .id("click-2").on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    setter(this, !checked);
                    cx.notify();
                }))
        )
}
