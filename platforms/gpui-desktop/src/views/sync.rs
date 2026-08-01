//! 同步页面
//!
//! 1:1 复刻 Tauri 同步页面：
//! - 批量操作（转拼音全部/去词全部/修复全部编码）
//! - 同步规则列表

use crate::state::CyrioApp;
use crate::task::Command;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

pub fn render_sync_view(app: &mut CyrioApp, cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let song_count = app.songs.len();

    div()
        .flex()
        .flex_col()
        .size_full()
        .gap(px(12.0))
        // 标题
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_14))
                .font_weight(FontWeight::SEMIBOLD)
                .child("批量同步")
        )
        // 说明
        .child(
            div()
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(Theme::FONT_12))
                .child(format!("共 {} 首歌曲可处理", song_count))
        )
        // 操作列表
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(render_sync_item(
                    "全部转拼音",
                    "将所有歌曲标题中的中文转换为拼音",
                    "执行",
                    cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.show_notice("正在批量转拼音...", cx);
                        this.send_cmd(Command::BatchSlugAllSongs);
                    })
                ))
                .child(render_sync_item(
                    "全部去词",
                    "去除所有歌曲标题中的括号/引号/质量标签",
                    "执行",
                    cx.listener(|this, _: &ClickEvent, _window, cx| {
                        let words = this.settings.custom_words_vec();
                        this.show_notice("正在批量去词...", cx);
                        this.send_cmd(Command::BatchStripAllSongs { custom_words: words });
                    })
                ))
                .child(render_sync_item(
                    "修复全部编码",
                    "修复所有歌曲的 ID3 标签编码问题",
                    "执行",
                    cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.show_notice("正在修复编码...", cx);
                        this.send_cmd(Command::RepairAllSongsEncoding);
                    })
                ))
        )
}

fn render_sync_item(
    title: &str,
    desc: &str,
    btn_label: &str,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .h(px(56.0))
        .px(px(12.0))
        .border_1()
        .border_color(Theme::BORDER)
        .rounded(px(Theme::RADIUS_SM))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_13))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string())
                )
                .child(
                    div()
                        .text_color(Theme::TEXT_DIM)
                        .text_size(px(Theme::FONT_11))
                        .child(desc.to_string())
                )
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(28.0))
                .px(px(16.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::RIO_BLUE)
                .text_color(Theme::WHITE)
                .text_size(px(Theme::FONT_12))
                .child(btn_label.to_string())
                .id("click-1").on_click(move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
                    handler(event, window, cx);
                })
        )
}
