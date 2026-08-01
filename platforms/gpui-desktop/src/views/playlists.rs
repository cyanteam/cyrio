//! 歌单页面
//!
//! 1:1 复刻 Tauri 歌单页面：
//! - 歌单列表（名称 + 存储标签）
//! - 创建歌单按钮
//! - 歌单详情（点击查看歌单内歌曲）

use crate::state::CyrioApp;
use crate::task::Command;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

pub fn render_playlists_view(app: &mut CyrioApp, cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let loading = app.loading;
    let playlists = app.playlists.clone();
    let selected_playlist = app.selected_playlist;
    let playlist_songs = app.playlist_songs.clone();
    let create_playlist_open = app.create_playlist_open;
    let create_playlist_name = app.create_playlist_name.clone();

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap(px(8.0))
        // 标题栏
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .h(px(32.0))
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_14))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("歌单")
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(28.0))
                        .px(px(12.0))
                        .rounded(px(Theme::RADIUS_SM))
                        .bg(Theme::RIO_BLUE)
                        .text_color(Theme::WHITE)
                        .text_size(px(Theme::FONT_12))
                        .child("+ 新建歌单")
                        .id("click-1").on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.create_playlist_open = true;
                            cx.notify();
                        }))
                )
        )
        // 内容区
        .child(
            if loading {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Theme::TEXT_DIM)
                    .text_size(px(Theme::FONT_14))
                    .child("加载中...")
                    .into_any_element()
            } else if playlists.is_empty() {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Theme::TEXT_DIM)
                    .text_size(px(Theme::FONT_14))
                    .child("暂无歌单")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .flex_1()
                    .min_h_0()
                    // 左侧：歌单列表
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(200.0))
                            .gap(px(4.0))
                            .border_1()
                            .border_color(Theme::BORDER)
                            .rounded(px(Theme::RADIUS_SM))
                            .p(px(4.0))
                            .id("playlists-list")
                            .overflow_y_scroll()
                            .children(playlists.iter().map(|entry| {
                                let name = if !entry.file.title.is_empty() {
                                    entry.file.title.clone()
                                } else {
                                    entry.file.name.clone()
                                };
                                let is_selected = selected_playlist == Some((entry.file.file_no, entry.mem_unit));
                                let file_no = entry.file.file_no;
                                let mem_unit = entry.mem_unit;
                                let mem_label = if mem_unit == 0 { "内置" } else { "SD" };

                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .h(px(32.0))
                                    .px(px(8.0))
                                    .rounded(px(Theme::RADIUS_XS))
                                    .when(is_selected, |this| {
                                        this.bg(Theme::RIO_BLUE_SUBTLE)
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_color(if is_selected { Theme::RIO_BLUE_DARK } else { Theme::TEXT })
                                            .text_size(px(Theme::FONT_12))
                                            .child(name)
                                    )
                                    .child(
                                        div()
                                            .text_color(Theme::TEXT_DIM)
                                            .text_size(px(Theme::FONT_10))
                                            .child(mem_label.to_string())
                                    )
                                    .id("click-2").on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                        this.selected_playlist = Some((file_no, mem_unit));
                                        this.loading = true;
                                        this.send_cmd(Command::ListPlaylistSongs { playlist_file_no: file_no, mem_unit });
                                        cx.notify();
                                    }))
                                    .into_any_element()
                            }))
                    )
                    // 右侧：歌单详情
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .border_1()
                            .border_color(Theme::BORDER)
                            .rounded(px(Theme::RADIUS_SM))
                            .p(px(4.0))
                            .child(
                                if selected_playlist.is_none() {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(Theme::TEXT_DIM)
                                        .text_size(px(Theme::FONT_13))
                                        .child("← 选择一个歌单查看内容")
                                        .into_any_element()
                                } else if playlist_songs.is_empty() {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(Theme::TEXT_DIM)
                                        .text_size(px(Theme::FONT_13))
                                        .child("歌单为空")
                                        .into_any_element()
                                } else {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .flex_1()
                                        .min_h_0()
                                        .id("playlist-songs")
                                        .overflow_y_scroll()
                                        .children(playlist_songs.iter().enumerate().map(|(i, song)| {
                                            let title = if !song.title.is_empty() {
                                                song.title.clone()
                                            } else if !song.name.is_empty() {
                                                song.name.rsplit(['\\', '/']).next().unwrap_or(&song.name).to_string()
                                            } else {
                                                "(无标题)".to_string()
                                            };
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .h(px(28.0))
                                                .px(px(8.0))
                                                .rounded(px(Theme::RADIUS_XS))
                                                .text_color(Theme::TEXT)
                                                .text_size(px(Theme::FONT_12))
                                                .child(
                                                    div()
                                                        .w(px(28.0))
                                                        .text_color(Theme::TEXT_DIM)
                                                        .text_size(px(Theme::FONT_11))
                                                        .child(format!("{}.", i + 1))
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .child(title)
                                                )
                                                .into_any_element()
                                        }))
                                        .into_any_element()
                                }
                            )
                    )
                    .into_any_element()
            }
        )
        // 创建歌单弹窗
        .when(create_playlist_open, |this| {
            this.child(render_create_playlist_modal(app, cx, &create_playlist_name))
        })
}

/// 创建歌单弹窗
fn render_create_playlist_modal(
    app: &CyrioApp,
    cx: &mut Context<CyrioApp>,
    name: &str,
) -> impl IntoElement {
    let _ = app;
    let name = name.to_string();
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(Theme::MODAL_OVERLAY)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .w(px(320.0))
                .bg(Theme::BG_ELEVATED)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(Theme::RADIUS_MD))
                .p(px(20.0))
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_14))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("新建歌单")
                )
                // 歌单名输入
                .child(
                    div()
                        .w_full()
                        .h(px(32.0))
                        .px(px(8.0))
                        .rounded(px(Theme::RADIUS_SM))
                        .bg(Theme::BG_SUBTLE)
                        .border_1()
                        .border_color(Theme::BORDER)
                        .flex()
                        .items_center()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_13))
                        .child(if name.is_empty() { "输入歌单名称...".to_string() } else { name.clone() })
                )
                // 存储选择
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(28.0))
                                .px(px(12.0))
                                .rounded(px(Theme::RADIUS_SM))
                                .bg(if app.create_playlist_mem == 0 { Theme::RIO_BLUE } else { Theme::BG_SUBTLE })
                                .text_color(if app.create_playlist_mem == 0 { Theme::WHITE } else { Theme::TEXT_SECONDARY })
                                .text_size(px(Theme::FONT_12))
                                .child("内置")
                                .id("click-3").on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.create_playlist_mem = 0;
                                    cx.notify();
                                }))
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(28.0))
                                .px(px(12.0))
                                .rounded(px(Theme::RADIUS_SM))
                                .bg(if app.create_playlist_mem == 1 { Theme::RIO_BLUE } else { Theme::BG_SUBTLE })
                                .text_color(if app.create_playlist_mem == 1 { Theme::WHITE } else { Theme::TEXT_SECONDARY })
                                .text_size(px(Theme::FONT_12))
                                .child("SD 卡")
                                .id("click-4").on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.create_playlist_mem = 1;
                                    cx.notify();
                                }))
                        )
                )
                // 按钮组
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(28.0))
                                .px(px(16.0))
                                .rounded(px(Theme::RADIUS_SM))
                                .bg(Theme::BG_MUTED)
                                .text_color(Theme::TEXT_SECONDARY)
                                .text_size(px(Theme::FONT_12))
                                .child("取消")
                                .id("click-5").on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.create_playlist_open = false;
                                    this.create_playlist_name.clear();
                                    cx.notify();
                                }))
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
                                .child("创建")
                                .id("click-6").on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                    if !this.create_playlist_name.is_empty() {
                                        this.send_cmd(Command::CreatePlaylist {
                                            name: this.create_playlist_name.clone(),
                                            mem_unit: this.create_playlist_mem,
                                        });
                                        cx.notify();
                                    }
                                }))
                        )
                )
        )
}
