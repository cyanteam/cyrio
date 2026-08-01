//! 设备信息页面
//!
//! 1:1 复刻 Tauri 设备信息页面：
//! - 存储卡片（内置/SD 卡）
//! - 设备信息

use crate::state::{format_bytes, CyrioApp};
use crate::task::Command;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

pub fn render_device_info_view(app: &mut CyrioApp, cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let internal = app.internal_mem.clone();
    let sd = app.sd_mem.clone();
    let song_count = app.songs.len();
    let playlist_count = app.playlists.len();

    // 请求存储状态
    if internal.is_none() && sd.is_none() {
        app.send_cmd(Command::GetStorageStatus);
    }

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
                .child("设备信息")
        )
        // 存储卡片网格
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(12.0))
                // 内置闪存
                .child(render_storage_card("内置闪存", internal.as_ref(), Theme::RIO_BLUE))
                // SD 卡
                .child(render_storage_card("SD 卡", sd.as_ref(), Theme::S30S_ORANGE))
        )
        // 统计信息
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(Theme::RADIUS_SM))
                .child(render_info_row("歌曲总数", &format!("{}", song_count)))
                .child(render_info_row("歌单总数", &format!("{}", playlist_count)))
        )
        // 刷新按钮
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(32.0))
                .px(px(20.0))
                .rounded(px(Theme::RADIUS_SM))
                .border_1()
                .border_color(Theme::BORDER)
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(Theme::FONT_12))
                .child("刷新设备信息")
                .id("click-1").on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.send_cmd(Command::GetStorageStatus);
                    this.pending_song_loads = 2;
                    this.send_cmd(Command::ListSongs(0));
                    this.send_cmd(Command::ListSongs(1));
                    cx.notify();
                }))
        )
}

fn render_storage_card(
    name: &str,
    mem: Option<&cyrio_core::protocol::rio_mem::RioMem>,
    color: Hsla,
) -> impl IntoElement {
    let present = mem.map(|m| m.is_present()).unwrap_or(false);
    let used_pct = mem.map(|m| if m.size > 0 { m.used as f32 / m.size as f32 } else { 0.0 }).unwrap_or(0.0);

    div()
        .flex()
        .flex_col()
        .flex_1()
        .gap(px(8.0))
        .p(px(16.0))
        .border_1()
        .border_color(Theme::BORDER)
        .rounded(px(Theme::RADIUS_MD))
        // 标题
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(color)
                )
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_13))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(name.to_string())
                )
        )
        // 状态
        .child(
            if present {
                if let Some(m) = mem {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_color(Theme::TEXT_SECONDARY)
                                .text_size(px(Theme::FONT_12))
                                .child(format!("已用 {} / 共 {}", format_bytes(m.used as u64), format_bytes(m.size as u64)))
                        )
                        // 进度条
                        .child(
                            div()
                                .w_full()
                                .h(px(8.0))
                                .rounded(px(4.0))
                                .bg(Theme::BG_SUBTLE)
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(used_pct))
                                        .rounded(px(4.0))
                                        .bg(color)
                                )
                        )
                        .child(
                            div()
                                .text_color(Theme::TEXT_DIM)
                                .text_size(px(Theme::FONT_11))
                                .child(format!("空闲 {} ({:.0}%)", format_bytes(m.free as u64), (1.0 - used_pct) * 100.0))
                        )
                        .when(!m.name.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_color(Theme::TEXT_DIM)
                                    .text_size(px(Theme::FONT_11))
                                    .child(format!("名称: {}", m.name))
                            )
                        })
                        .when(!m.model.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_color(Theme::TEXT_DIM)
                                    .text_size(px(Theme::FONT_11))
                                    .child(format!("型号: {}", m.model))
                            )
                        })
                        .into_any_element()
                } else {
                    div().into_any_element()
                }
            } else {
                div()
                    .text_color(Theme::TEXT_DIM)
                    .text_size(px(Theme::FONT_12))
                    .child("未插入")
                    .into_any_element()
            }
        )
}

fn render_info_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_12))
                .child(label.to_string())
        )
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_12))
                .font_weight(FontWeight::SEMIBOLD)
                .child(value.to_string())
        )
}
