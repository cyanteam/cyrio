//! 播放器底栏 — 48px
//!
//! 1:1 复刻 Tauri .player-bar：
//! [info title+subtitle] [▶/⏸ 28×28 蓝底白字] [⏹ 28×28 bg-subtle] [time 10px] [progress 4px flex:1] [time 10px] [× 24×24]

use crate::state::{format_time, CyrioApp};
use crate::task::Command;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

pub fn render_player_bar(app: &mut CyrioApp, cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let is_playing = app.playback.is_playing;
    let is_loading = app.playback.is_loading;
    let position = app.playback.position;
    let duration = app.playback.duration;
    let has_track = app.current_playing_file_no.is_some();
    let current_file_no = app.current_playing_file_no;

    // 查找当前播放歌曲标题
    let title_subtitle = current_file_no.and_then(|file_no| {
        app.songs.iter().find(|e| e.file.file_no == file_no).map(|e| {
            let title = if !e.file.title.is_empty() {
                e.file.title.clone()
            } else {
                e.file.name.clone()
            };
            let subtitle = if !e.file.artist.is_empty() {
                e.file.artist.clone()
            } else {
                "未知艺术家".to_string()
            };
            (title, subtitle)
        })
    });

    let play_label = if is_loading { "⏳" } else if is_playing { "⏸" } else { "▶" };
    let pos_str = format_time(position as u32);
    let dur_str = format_time(duration as u32);
    let frac = if duration > 0.0 { (position / duration).clamp(0.0, 1.0) } else { 0.0 };

    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(Theme::PLAYER_H))
        .px(px(16.0))
        .gap(px(10.0))
        .bg(Theme::BG_ELEVATED)
        .border_t_1()
        .border_color(Theme::BORDER)
        // [player-info] title 12px 600 + subtitle 10px dim
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(120.0))
                .child(
                    if let Some((title, subtitle)) = &title_subtitle {
                        div()
                            .text_color(Theme::TEXT)
                            .text_size(px(Theme::FONT_12))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title.clone())
                            .into_any_element()
                    } else {
                        div()
                            .text_color(Theme::TEXT_DIM)
                            .text_size(px(Theme::FONT_12))
                            .child("未播放")
                            .into_any_element()
                    }
                )
                .when_some(title_subtitle, |this, (_, subtitle)| {
                    this.child(
                        div()
                            .text_color(Theme::TEXT_DIM)
                            .text_size(px(Theme::FONT_10))
                            .child(subtitle)
                    )
                })
        )
        // [▶/⏸ 28×28 蓝底白字]
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(28.0))
                .h(px(28.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::RIO_BLUE)
                .text_color(Theme::WHITE)
                .text_size(px(13.0))
                .child(play_label)
                .id("player-play-btn")
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    if has_track {
                        if this.playback.is_playing {
                            this.send_cmd(Command::PauseAudio);
                        } else {
                            this.send_cmd(Command::ResumeAudio);
                        }
                        cx.notify();
                    }
                }))
        )
        // [⏹ 28×28 bg-subtle]
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(28.0))
                .h(px(28.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_SUBTLE)
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(13.0))
                .child("⏹")
                .id("player-stop-btn")
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    if has_track {
                        this.send_cmd(Command::StopAudio);
                        this.current_playing_file_no = None;
                        cx.notify();
                    }
                }))
        )
        // [time 10px]
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_10))
                .child(pos_str)
        )
        // [progress 4px h 蓝色 flex:1]
        .child(
            div()
                .flex_1()
                .h(px(6.0))
                .rounded(px(3.0))
                .bg(Theme::BG_SUBTLE)
                .child(
                    div()
                        .h_full()
                        .w(relative(frac))
                        .rounded(px(3.0))
                        .bg(Theme::RIO_BLUE)
                )
        )
        // [time 10px]
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_10))
                .child(dur_str)
        )
        // [× 24×24]
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(24.0))
                .h(px(24.0))
                .rounded(px(Theme::RADIUS_XS))
                .text_color(Theme::TEXT_DIM)
                .text_size(px(14.0))
                .child("×")
                .id("player-close-btn")
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    if has_track {
                        this.send_cmd(Command::StopAudio);
                        this.current_playing_file_no = None;
                        cx.notify();
                    }
                }))
        )
}
