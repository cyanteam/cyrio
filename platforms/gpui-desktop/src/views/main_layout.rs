//! 主布局 — 1:1 复刻 Tauri 版 CSS 布局
//!
//! 结构：
//! .app-root (padding: 0 4px 4px, bg: #39c5bb 边框效果)
//!   .titlebar (28px, bg: #39c5bb, 标题 + 3 按钮 40px)
//!   .launcher (padding: 12px 20px 14px, bg: #f5f6f8)
//!     .top-bar (gap: 12px, margin-bottom: 10px)
//!       [← back 32px] [虚拟U盘 30px] [.menu-bar flex:1] [paginate 30px]
//!     .content-area (flex:1, gap: 10px)
//!       .content-inner → .pane (padding: 12px 16px, radius: 6px)
//!     .storage-status-bar (min 26px, margin-top: 8px)
//!     .player-bar (48px, absolute bottom)

use crate::state::{format_bytes, CyrioApp, NavPage};
use crate::task::Command;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

/// 渲染主布局
pub fn render_main_layout(
    app: &mut CyrioApp,
    _window: &mut Window,
    cx: &mut Context<CyrioApp>,
) -> impl IntoElement {
    let current_page = app.current_page;
    let paginate = app.paginate;
    let webdav_running = app.webdav_running;
    let has_upload_transfer = app.upload_transfer.is_some();
    let notice = app.notice.clone();
    let internal_mem = app.internal_mem.clone();
    let sd_mem = app.sd_mem.clone();
    let connected = app.connected;

    // .app-root: padding 0 4px 4px + bg #39c5bb（边框效果）
    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        // bg #39c5bb 模拟边框，padding 露出边框色
        .bg(Theme::RIO_BLUE)
        .pt(px(0.0))
        .pl(px(4.0))
        .pr(px(4.0))
        .pb(px(4.0))
        // ---- 自绘标题栏 28px ----
        .child(render_title_bar(app, cx))
        // ---- .launcher 主舞台 ----
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .relative()
                // .launcher padding: 12px 20px 14px, bg: #f5f6f8
                .bg(Theme::BG)
                .pt(px(12.0))
                .pl(px(20.0))
                .pr(px(20.0))
                .pb(px(14.0))
                // ---- .top-bar ----
                .child(render_top_bar(app, cx, current_page, paginate, webdav_running))
                // ---- .content-area ----
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_1()
                        .min_h_0()
                        .gap(px(10.0))
                        .mt(px(10.0))
                        .child(
                            if has_upload_transfer {
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(10.0))
                                    .size_full()
                                    .min_h_0()
                                    .child(crate::views::transfer::render_transfer_sidebar(app, cx))
                                    .child(render_content_pane(app, cx, current_page))
                                    .into_any_element()
                            } else {
                                render_content_pane(app, cx, current_page).into_any_element()
                            }
                        )
                )
                // ---- .storage-status-bar ----
                .child(render_storage_bar(connected, internal_mem.as_ref(), sd_mem.as_ref()))
                // ---- .player-bar (absolute bottom) ----
                .child(crate::views::player::render_player_bar(app, cx))
                // ---- notice toast ----
                .when_some(notice, |this, msg| {
                    this.child(render_notice_toast(msg))
                })
        )
}

/// 自绘标题栏 — .titlebar 28px bg #39c5bb
fn render_title_bar(app: &CyrioApp, cx: &mut Context<CyrioApp>) -> impl IntoElement {
    // 标题文本格式：[deviceLabel] [正在传输] pageLabel Cyrio 开源软件，请勿商用
    let mut parts: Vec<String> = Vec::new();
    if !app.device_name.is_empty() {
        parts.push(format!("[{}]", app.device_name));
    }
    if app.upload_transfer.is_some() {
        parts.push("[正在传输]".into());
    }
    parts.push(app.current_page.label().to_string());
    parts.push("Cyrio".into());
    parts.push("开源软件，请勿商用".into());
    let title_text = parts.join(" ");

    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(28.0))
        .flex_shrink_0()
        .bg(Theme::RIO_BLUE)
        // 标题文字 — flex:1, font 14.5px/700, 白字
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .px(px(12.0))
                .text_color(Theme::WHITE)
                .text_size(px(14.5))
                .font_weight(FontWeight::BOLD)
                .child(title_text)
                .id("titlebar-drag-area")
                .on_mouse_down(MouseButton::Left, cx.listener(|_this, _event: &MouseDownEvent, window, _cx| {
                    window.start_window_move();
                }))
        )
        // 右侧 3 个窗口按钮，各 40px 宽
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                // 最小化按钮
                .child(render_titlebar_btn("tb-min", "−", false, cx.listener(|_this, _event: &ClickEvent, window, _cx| {
                    window.minimize_window();
                })))
                // 最大化按钮
                .child(render_titlebar_btn("tb-max", "□", false, cx.listener(|_this, _event: &ClickEvent, window, _cx| {
                    window.toggle_fullscreen();
                })))
                // 关闭按钮
                .child(render_titlebar_btn("tb-close", "×", true, cx.listener(|_this, _event: &ClickEvent, _window, cx| {
                    cx.quit();
                })))
        )
}

/// 标题栏按钮 — 40px 宽，hover 黑色半透明，关闭按钮 hover 红色
fn render_titlebar_btn(
    id: &str,
    symbol: &str,
    is_close: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id = id.to_string();
    let hover_bg = if is_close {
        // 关闭按钮 hover 红色 #e81123
        Theme::CLOSE_BTN_HOVER
    } else {
        Theme::TITLEBAR_BTN_HOVER
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .w(px(40.0))
        .h(px(28.0))
        .bg(Theme::RIO_BLUE)
        .text_color(Theme::WHITE)
        .text_size(px(12.0))
        .hover(move |this| this.bg(hover_bg))
        .child(symbol.to_string())
        .id(id)
        .on_click(move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
            handler(event, window, cx);
        })
}

/// .top-bar — [← back 32px] [虚拟U盘 30px] [.menu-bar flex:1] [paginate 30px]
fn render_top_bar(
    _app: &mut CyrioApp,
    cx: &mut Context<CyrioApp>,
    current_page: NavPage,
    paginate: bool,
    webdav_running: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .flex_shrink_0()
        // [← back] 32px 高，padding 0 10px
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .h(px(32.0))
                .px(px(10.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_ELEVATED)
                .border_1()
                .border_color(Theme::BG_SUBTLE)
                .text_color(Theme::TEXT)
                .text_size(px(13.5))
                .child("←")
                .id("btn-back")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.send_cmd(Command::CloseDevice);
                    cx.notify();
                }))
        )
        // [虚拟U盘] 30px 高
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(30.0))
                .px(px(12.0))
                .rounded(px(Theme::RADIUS_SM))
                .when(webdav_running, |this| {
                    this.bg(Theme::RIO_BLUE).border_1().border_color(Theme::RIO_BLUE)
                })
                .when(!webdav_running, |this| {
                    this.bg(Theme::BG_ELEVATED).border_1().border_color(Theme::BG_SUBTLE)
                })
                .text_color(if webdav_running { Theme::WHITE } else { Theme::TEXT })
                .text_size(px(13.0))
                .child(if webdav_running { "停止虚拟U盘".to_string() } else { "虚拟U盘".to_string() })
                .id("btn-webdav")
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    this.webdav_running = !this.webdav_running;
                    cx.notify();
                }))
        )
        // [.menu-bar] flex:1, padding 3px, gap 1px
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(1.0))
                .bg(Theme::BG_ELEVATED)
                .border_1()
                .border_color(Theme::BG_SUBTLE)
                .rounded(px(Theme::RADIUS_SM))
                .p(px(3.0))
                .children(NavPage::all().iter().map(|&page| {
                    let is_active = page == current_page;
                    render_menu_item(page, is_active, cx)
                }))
        )
        // [paginate-toggle] 30×30
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(30.0))
                .h(px(30.0))
                .rounded(px(Theme::RADIUS_SM))
                .when(paginate, |this| {
                    this.bg(Theme::RIO_BLUE).border_1().border_color(Theme::RIO_BLUE)
                })
                .when(!paginate, |this| {
                    this.bg(Theme::BG_ELEVATED).border_1().border_color(Theme::BG_SUBTLE)
                })
                .text_color(if paginate { Theme::WHITE } else { Theme::TEXT })
                .text_size(px(14.0))
                .child(if paginate { "▤".to_string() } else { "☰".to_string() })
                .id("btn-paginate")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.paginate = !this.paginate;
                    if this.paginate { this.current_page_num = 0; }
                    cx.notify();
                }))
        )
}

/// .menu-item — padding 6px 12px, font 14px/500, 纯文字无图标
fn render_menu_item(
    page: NavPage,
    is_active: bool,
    cx: &mut Context<CyrioApp>,
) -> impl IntoElement {
    let id = format!("menu-{}", page.path());
    div()
        .flex()
        .items_center()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(Theme::RADIUS_XS))
        .when(is_active, |this| {
            this.bg(Theme::RIO_BLUE)
        })
        .when(!is_active, |this| {
            this.bg(Theme::BG_ELEVATED)
        })
        .text_color(if is_active { Theme::WHITE } else { Theme::TEXT_SECONDARY })
        .text_size(px(14.0))
        .child(page.label().to_string())
        .id(id)
        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.navigate(page, cx);
        }))
}

/// .pane — 白底 6px 圆角 1px 边框, padding 12px 16px
fn render_content_pane(
    app: &mut CyrioApp,
    cx: &mut Context<CyrioApp>,
    current_page: NavPage,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BG_SUBTLE)
        .rounded(px(Theme::RADIUS_MD))
        .py(px(12.0))
        .px(px(16.0))
        .gap(px(8.0))
        .id("content-pane-scroll")
        .overflow_y_scroll()
        .child(render_page_content(app, cx, current_page))
}

/// 根据当前页面调度渲染
fn render_page_content(
    app: &mut CyrioApp,
    cx: &mut Context<CyrioApp>,
    page: NavPage,
) -> AnyElement {
    match page {
        NavPage::Songs => crate::views::songs::render_songs_view(app, cx).into_any_element(),
        NavPage::Playlists => crate::views::playlists::render_playlists_view(app, cx).into_any_element(),
        NavPage::Upload => crate::views::upload::render_upload_view(app, cx).into_any_element(),
        NavPage::Sync => crate::views::sync::render_sync_view(app, cx).into_any_element(),
        NavPage::Transmission => crate::views::transfer::render_transmission_page(app, cx).into_any_element(),
        NavPage::DeviceInfo => crate::views::device_info::render_device_info_view(app, cx).into_any_element(),
        NavPage::Settings => crate::views::settings::render_settings_view(app, cx).into_any_element(),
        NavPage::About => crate::views::about::render_about_view(app, cx).into_any_element(),
    }
}

/// .storage-status-bar — min 26px, padding 4px 10px, margin-top 8px
/// 存储项直接作为子元素（无内层包装），每个 flex_1 均匀分布
fn render_storage_bar(
    connected: bool,
    internal_mem: Option<&cyrio_core::protocol::rio_mem::RioMem>,
    sd_mem: Option<&cyrio_core::protocol::rio_mem::RioMem>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .mt(px(8.0))
        .flex_shrink_0()
        .py(px(4.0))
        .px(px(10.0))
        .min_h(px(26.0))
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BG_SUBTLE)
        .rounded(px(Theme::RADIUS_SM))
        .text_size(px(13.0))
        .when(!connected, |this| {
            this.child(
                div()
                    .w_full()
                    .text_center()
                    .text_color(Theme::TEXT_DIM)
                    .text_size(px(13.0))
                    .child("未连接设备")
            )
        })
        .when(connected, |this| {
            this
                .children(internal_mem.iter().filter(|m| m.is_present()).map(|m| {
                    render_storage_item("内置", m, Theme::RIO_BLUE)
                }))
                .children(sd_mem.iter().filter(|m| m.is_present()).map(|m| {
                    render_storage_item("SD", m, Theme::S30S_ORANGE)
                }))
        })
}

/// 单个存储状态项 — label + free/size + mini-bar(6px)
fn render_storage_item(
    name: &str,
    m: &cyrio_core::protocol::rio_mem::RioMem,
    bar_color: Hsla,
) -> impl IntoElement {
    let used_pct = if m.size > 0 { m.used as f32 / m.size as f32 } else { 0.0 };
    let label = format!("{} / {}", format_bytes(m.free as u64), format_bytes(m.size as u64));

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .flex_1()
        .min_w_0()
        // label 12px/600
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(name.to_string())
        )
        // mini-bar 6px 高, max 160px
        .child(
            div()
                .flex_1()
                .min_w(px(40.0))
                .max_w(px(160.0))
                .h(px(6.0))
                .rounded(px(3.0))
                .bg(Theme::BG_SUBTLE)
                .child(
                    div()
                        .h_full()
                        .w(relative(used_pct))
                        .rounded(px(2.0))
                        .bg(bar_color)
                )
        )
        // value 13px
        .child(
            div()
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(13.0))
                .child(label)
        )
}

/// .notice-toast — 左下角浮层
fn render_notice_toast(msg: String) -> impl IntoElement {
    div()
        .absolute()
        .bottom(px(Theme::PLAYER_H + 8.0))
        .left(px(20.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .py(px(7.0))
        .pl(px(12.0))
        .pr(px(10.0))
        .rounded(px(Theme::RADIUS_SM))
        .bg(Theme::TEXT)
        .child(
            div()
                .w(px(3.0))
                .h(px(16.0))
                .rounded(px(2.0))
                .bg(Theme::RIO_BLUE)
        )
        .child(
            div()
                .text_color(Theme::WHITE)
                .text_size(px(13.0))
                .child(msg)
        )
}
