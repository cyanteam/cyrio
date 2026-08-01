//! 传输侧栏 — 260px 左侧非模态
//!
//! 1:1 复刻 Tauri UploadSidebar：
//! - 标题栏（传输完成/正在传输）
//! - 总进度条
//! - 当前文件进度
//! - 文件列表

use crate::state::{format_bytes, CyrioApp, UploadFileStatus};
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

pub fn render_transfer_sidebar(app: &CyrioApp, _cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let ut = match &app.upload_transfer {
        Some(u) => u,
        None => return div().into_any_element(),
    };

    let all_done = ut.all_done();
    let title = if all_done { "传输完成" } else { "正在传输" };
    let total_count = ut.files.len();
    let done_count = ut.done_count();
    let failed_count = ut.failed_count();
    let total_fraction = ut.total_fraction();

    let current_file = ut.files.get(ut.current_index);
    let current_name = current_file.map(|f| f.name.clone());
    let current_frac = current_file.and_then(|f| {
        if f.total > 0 { Some(f.transferred as f32 / f.total as f32) } else { None }
    });
    let current_bytes = current_file.map(|f| (f.transferred, f.total));

    div()
        .flex()
        .flex_col()
        .w(px(260.0))
        .h_full()
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded(px(Theme::RADIUS_MD))
        .p(px(8.0))
        .gap(px(6.0))
        // 标题栏
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(28.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_SUBTLE)
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_12))
                .child(title.to_string())
        )
        // 总进度
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_color(Theme::TEXT)
                        .text_size(px(Theme::FONT_11))
                        .child("总进度")
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .when(failed_count > 0, |this| {
                            this.child(
                                div()
                                    .text_color(Theme::ERROR)
                                    .text_size(px(Theme::FONT_10))
                                    .child(format!(" (失败 {})", failed_count))
                            )
                        })
                        .child(
                            div()
                                .text_color(Theme::TEXT_SECONDARY)
                                .text_size(px(Theme::FONT_10))
                                .child(format!("{} / {}", done_count, total_count))
                        )
                )
        )
        // 总进度条 3px
        .child(
            div()
                .w_full()
                .h(px(6.0))
                .rounded(px(3.0))
                .bg(Theme::BG_SUBTLE)
                .child(
                    div()
                        .h_full()
                        .w(relative(total_fraction))
                        .rounded(px(3.0))
                        .bg(Theme::RIO_BLUE)
                )
        )
        // 当前文件
        .when_some(current_name, |this, name| {
            this.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_color(Theme::TEXT)
                            .text_size(px(Theme::FONT_11))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name)
                    )
                    .when_some(current_frac, |this, frac| {
                        this.child(
                            div()
                                .text_color(Theme::RIO_BLUE)
                                .text_size(px(Theme::FONT_11))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("{}%", (frac * 100.0) as u32))
                        )
                    })
            )
            .when_some(current_frac, |this, frac| {
                this.child(
                    div()
                        .w_full()
                        .h(px(6.0))
                        .rounded(px(3.0))
                        .bg(Theme::BG_SUBTLE)
                        .child(
                            div()
                                .h_full()
                                .w(relative(frac))
                                .rounded(px(3.0))
                                .bg(Theme::RIO_BLUE_LIGHT)
                        )
                )
            })
            .when_some(current_bytes, |this, (transferred, total)| {
                this.when(total > 0, |this| {
                    this.child(
                        div()
                            .text_color(Theme::TEXT_DIM)
                            .text_size(px(Theme::FONT_10))
                            .child(format!("{} / {}", format_bytes(transferred), format_bytes(total)))
                    )
                })
            })
        })
        // 文件列表
        .child(
            div()
                .id("transfer-file-list")
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(ut.files.iter().map(|f| {
                    let (icon, icon_color) = match f.status {
                        UploadFileStatus::Uploading => ("→", Theme::RIO_BLUE),
                        UploadFileStatus::Done => ("✓", Theme::SUCCESS),
                        UploadFileStatus::Failed => ("×", Theme::ERROR),
                        UploadFileStatus::Pending => ("·", Theme::TEXT_DIM),
                    };
                    let name_color = match f.status {
                        UploadFileStatus::Uploading => Theme::TEXT,
                        UploadFileStatus::Done => Theme::TEXT_DIM,
                        UploadFileStatus::Failed => Theme::ERROR,
                        UploadFileStatus::Pending => Theme::TEXT_SECONDARY,
                    };

                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .py(px(2.0))
                        .child(
                            div()
                                .text_color(icon_color)
                                .text_size(px(Theme::FONT_11))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(icon.to_string())
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(name_color)
                                .text_size(px(Theme::FONT_11))
                                .child(f.name.clone())
                        )
                        .when(f.status == UploadFileStatus::Uploading && f.total > 0, |this| {
                            this.child(
                                div()
                                    .text_color(Theme::TEXT_DIM)
                                    .text_size(px(Theme::FONT_10))
                                    .child(format_bytes(f.transferred))
                            )
                        })
                        .when(f.status == UploadFileStatus::Done && f.total > 0, |this| {
                            this.child(
                                div()
                                    .text_color(Theme::SUCCESS)
                                    .text_size(px(Theme::FONT_10))
                                    .child(format_bytes(f.total))
                            )
                        })
                }))
        )
        .into_any_element()
}

/// 传输页面（作为独立 tab 内容区）
pub fn render_transmission_page(app: &mut CyrioApp, _cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let has_transfer = app.upload_transfer.is_some();

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap(px(12.0))
        // 标题
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_14))
                .font_weight(FontWeight::SEMIBOLD)
                .child("传输")
        )
        // 内容
        .child(
            if has_transfer {
                // 有传输任务：显示传输详情（全宽版）
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(8.0))
                    .child(render_transfer_detail(app))
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Theme::TEXT_DIM)
                    .text_size(px(Theme::FONT_14))
                    .child("暂无传输任务")
                    .into_any_element()
            }
        )
}

/// 传输详情（全宽版，用于传输页面）
fn render_transfer_detail(app: &CyrioApp) -> impl IntoElement {
    let ut = match &app.upload_transfer {
        Some(u) => u,
        None => return div().into_any_element(),
    };

    let all_done = ut.all_done();
    let title = if all_done { "传输完成" } else { "正在传输" };
    let total_count = ut.files.len();
    let done_count = ut.done_count();
    let failed_count = ut.failed_count();
    let total_fraction = ut.total_fraction();

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(12.0))
        .border_1()
        .border_color(Theme::BORDER)
        .rounded(px(Theme::RADIUS_SM))
        // 标题
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
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
                        .child(format!("{} / {}", done_count, total_count))
                )
        )
        // 总进度条
        .child(
            div()
                .w_full()
                .h(px(8.0))
                .rounded(px(4.0))
                .bg(Theme::BG_SUBTLE)
                .child(
                    div()
                        .h_full()
                        .w(relative(total_fraction))
                        .rounded(px(4.0))
                        .bg(Theme::RIO_BLUE)
                )
        )
        // 文件列表
        .child(
            div()
                .id("transmission-file-list")
                .flex()
                .flex_col()
                .gap(px(2.0))
                .max_h(px(400.0))
                .overflow_y_scroll()
                .children(ut.files.iter().map(|f| {
                    let (icon, icon_color) = match f.status {
                        UploadFileStatus::Uploading => ("→", Theme::RIO_BLUE),
                        UploadFileStatus::Done => ("✓", Theme::SUCCESS),
                        UploadFileStatus::Failed => ("×", Theme::ERROR),
                        UploadFileStatus::Pending => ("·", Theme::TEXT_DIM),
                    };
                    let name_color = match f.status {
                        UploadFileStatus::Uploading => Theme::TEXT,
                        UploadFileStatus::Done => Theme::TEXT_DIM,
                        UploadFileStatus::Failed => Theme::ERROR,
                        UploadFileStatus::Pending => Theme::TEXT_SECONDARY,
                    };

                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .py(px(3.0))
                        .child(
                            div()
                                .text_color(icon_color)
                                .text_size(px(Theme::FONT_11))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(icon.to_string())
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(name_color)
                                .text_size(px(Theme::FONT_12))
                                .child(f.name.clone())
                        )
                        .when(f.status == UploadFileStatus::Uploading && f.total > 0, |this| {
                            this.child(
                                div()
                                    .text_color(Theme::RIO_BLUE)
                                    .text_size(px(Theme::FONT_11))
                                    .child(format!(
                                        "{}%",
                                        (f.transferred as f32 / f.total as f32 * 100.0) as u32
                                    ))
                            )
                        })
                        .when(f.status == UploadFileStatus::Done, |this| {
                            this.child(
                                div()
                                    .text_color(Theme::SUCCESS)
                                    .text_size(px(Theme::FONT_11))
                                    .child("完成")
                            )
                        })
                        .when(f.status == UploadFileStatus::Failed, |this| {
                            this.child(
                                div()
                                    .text_color(Theme::ERROR)
                                    .text_size(px(Theme::FONT_11))
                                    .child("失败")
                            )
                        })
                }))
        )
        .when(failed_count > 0, |this| {
            this.child(
                div()
                    .text_color(Theme::ERROR)
                    .text_size(px(Theme::FONT_11))
                    .child(format!("失败 {} 个文件", failed_count))
            )
        })
        .into_any_element()
}
