//! 连接场景 — 1:1 复刻 Tauri 版 ConnectScene
//!
//! - 自动扫描 Diamond 设备（VID=0x045a），8秒间隔
//! - 有设备：显示大圆球（rio-orb），点击连接
//! - 无设备：显示"未检测到 Rio 设备" + 强制添加按钮
//! - 连接中：显示"正在连接..."

use crate::state::CyrioApp;
use crate::task::Command;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

/// Diamond 厂商 VID
const DIAMOND_VID: u16 = 0x045a;
/// 自动扫描间隔（秒）
const SCAN_INTERVAL_SECS: u64 = 8;

pub fn render_connect_scene(
    app: &mut CyrioApp,
    _window: &mut Window,
    cx: &mut Context<CyrioApp>,
) -> impl IntoElement {
    let scanning = app.scanning;
    let connecting = app.connecting;

    // 自动扫描：初始扫描 + 8秒定时
    let now = std::time::Instant::now();
    let need_scan = !scanning && !connecting && (
        app.last_scan_time.is_none() ||
        now.duration_since(app.last_scan_time.unwrap()).as_secs() >= SCAN_INTERVAL_SECS
    );
    if need_scan {
        app.scanning = true;
        app.last_scan_time = Some(now);
        app.send_cmd(Command::ScanDevices);
    }

    // 过滤 Diamond 设备
    let diamond_devices: Vec<&cyrio_transport_nusb::UsbDeviceInfo> = app.usb_devices.iter()
        .filter(|d| d.vid == DIAMOND_VID)
        .collect();

    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(Theme::BG)
        .items_center()
        .justify_center()
        .gap(px(24.0))
        // ---- 连接中状态 ----
        .when(connecting, |this| {
            this.child(
                div()
                    .text_color(Theme::RIO_BLUE)
                    .text_size(px(Theme::FONT_16))
                    .child("正在连接设备…")
            )
        })
        // ---- 有 Diamond 设备：显示大圆球 ----
        .when(!connecting && !diamond_devices.is_empty(), |this| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(20.0))
                    // 提示文字
                    .child(
                        div()
                            .text_color(Theme::TEXT)
                            .text_size(px(Theme::FONT_14))
                            .child("检测到 Rio 设备，点击连接")
                    )
                    // 大圆球列表
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(20.0))
                            .children(diamond_devices.iter().map(|dev| {
                                render_rio_orb(dev, cx)
                            }))
                    )
            )
        })
        // ---- 无设备：显示提示 ----
        .when(!connecting && diamond_devices.is_empty(), |this| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(16.0))
                    // 设备图标
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(80.0))
                            .h(px(80.0))
                            .rounded_full()
                            .border_2()
                            .border_color(Theme::BORDER)
                            .bg(Theme::BG_ELEVATED)
                            .text_color(Theme::TEXT_DIM)
                            .text_size(px(32.0))
                            .child("♪")
                    )
                    // 状态文字
                    .child(
                        div()
                            .text_color(Theme::TEXT_SECONDARY)
                            .text_size(px(Theme::FONT_14))
                            .child(
                                if scanning { "正在扫描 USB 设备…".to_string() }
                                else { "未检测到 Rio 设备，请连接后自动识别".to_string() }
                            )
                    )
            )
        })
        // ---- 强制添加按钮（始终显示）----
        .when(!connecting, |this| {
            this.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(32.0))
                    .px(px(16.0))
                    .rounded(px(Theme::RADIUS_SM))
                    .border_1()
                    .border_color(Theme::BORDER)
                    .text_color(Theme::TEXT_SECONDARY)
                    .text_size(px(Theme::FONT_12))
                    .child("+ 强制添加任意 USB 设备")
                    .id("btn-force-add")
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.force_add_open = true;
                        cx.notify();
                    }))
            )
        })
        // ---- 强制添加弹窗 ----
        .when(app.force_add_open, |this| {
            this.child(render_force_add_modal(app, cx))
        })
}

/// 大圆球 — rio-orb 140×140
fn render_rio_orb(
    dev: &cyrio_transport_nusb::UsbDeviceInfo,
    cx: &mut Context<CyrioApp>,
) -> impl IntoElement {
    let name = if !dev.name.is_empty() {
        dev.name.clone()
    } else {
        "Rio".to_string()
    };
    let vidpid = format!("{:04x}:{:04x}", dev.vid, dev.pid);
    let vid = dev.vid;
    let pid = dev.pid;
    let title = format!("{} · VID {:04x} PID {:04x}", name, dev.vid, dev.pid);

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .w(px(140.0))
        .h(px(140.0))
        .rounded_full()
        .border_2()
        .border_color(Theme::RIO_BLUE)
        .bg(Theme::BG_ELEVATED)
        // hover 效果
        .hover(|this| this.bg(Theme::RIO_BLUE_SUBTLE))
        // 内容：♪ 图标 + 设备名 + vid:pid
        .gap(px(4.0))
        .child(
            div()
                .text_color(Theme::RIO_BLUE)
                .text_size(px(28.0))
                .child("♪")
        )
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(Theme::FONT_13))
                .font_weight(FontWeight::SEMIBOLD)
                .child(name)
        )
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_10))
                .child(vidpid)
        )
        .id(format!("rio-orb-{}-{}", vid, pid))
        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.connecting = true;
            this.send_cmd(Command::OpenDeviceForce { vid, pid });
            cx.notify();
        }))
}

/// 强制添加设备弹窗
fn render_force_add_modal(
    app: &CyrioApp,
    cx: &mut Context<CyrioApp>,
) -> impl IntoElement {
    let devices = app.usb_devices.clone();

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
                .gap(px(8.0))
                .w(px(420.0))
                .max_h(px(400.0))
                .bg(Theme::BG_ELEVATED)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(Theme::RADIUS_MD))
                .p(px(20.0))
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
                                .text_size(px(Theme::FONT_14))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("强制添加设备")
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(24.0))
                                .h(px(24.0))
                                .rounded(px(Theme::RADIUS_XS))
                                .text_color(Theme::TEXT_DIM)
                                .text_size(px(16.0))
                                .child("×")
                                .id("btn-close-force-add")
                                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                    this.force_add_open = false;
                                    cx.notify();
                                }))
                        )
                )
                // 设备列表
                .child(
                    div()
                        .id("force-add-device-list")
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .children(devices.iter().map(|dev| {
                            let is_diamond = dev.vid == DIAMOND_VID;
                            let label = format!(
                                "{} ({:04x}:{:04x}) — {}",
                                if dev.name.is_empty() { "未知设备" } else { &dev.name },
                                dev.vid, dev.pid,
                                if dev.manufacturer.is_empty() { "未知厂商" } else { &dev.manufacturer }
                            );
                            let vid = dev.vid;
                            let pid = dev.pid;

                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .h(px(36.0))
                                .px(px(8.0))
                                .rounded(px(Theme::RADIUS_SM))
                                .border_1()
                                .border_color(Theme::BORDER)
                                .hover(|this| this.bg(Theme::BG_HOVER))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.0))
                                        .flex_1()
                                        .min_w_0()
                                        .when(is_diamond, |this| {
                                            this.child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .h(px(16.0))
                                                    .px(px(4.0))
                                                    .rounded(px(2.0))
                                                    .bg(Theme::RIO_BLUE_SUBTLE)
                                                    .text_color(Theme::RIO_BLUE_DARK)
                                                    .text_size(px(Theme::FONT_10))
                                                    .child("Diamond")
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_color(Theme::TEXT)
                                                .text_size(px(Theme::FONT_12))
                                                .child(label)
                                        )
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .h(px(24.0))
                                        .px(px(10.0))
                                        .rounded(px(Theme::RADIUS_XS))
                                        .bg(Theme::RIO_BLUE)
                                        .text_color(Theme::WHITE)
                                        .text_size(px(Theme::FONT_11))
                                        .child("连接")
                                        .id(format!("force-add-{}-{}", vid, pid))
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                            this.connecting = true;
                                            this.force_add_open = false;
                                            this.send_cmd(Command::OpenDeviceForce { vid, pid });
                                            cx.notify();
                                        }))
                                )
                                .into_any_element()
                        }))
                )
        )
}
