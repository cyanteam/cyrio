//! 设备连接场景：未连接时显示设备球 + 强制添加
//!
//! 对齐 Tauri ConnectScene：
//! - with-devices 态：connect-text + rio-orbs(140×140) + 强制添加按钮
//! - no-devices 态：connect-illustration(简化) + text + 强制添加按钮
//! - rio-orb：白底 2px rio-blue 边框 圆形，♪ 图标 + 设备名 + vid:pid

use async_channel::Sender;
use egui::{Context, Ui};

use crate::message::Command;
use crate::state::AppState;
use crate::theme;
use cyrio_transport_nusb::UsbDeviceInfo;

/// Diamond 厂商 VID
const DIAMOND_VID: u16 = 0x045a;
/// 扫描间隔（秒）
const SCAN_INTERVAL_SECS: f64 = 8.0;

/// 渲染设备连接场景
pub fn render(ui: &mut Ui, ctx: &Context, state: &mut AppState, cmd_tx: &Sender<Command>) {
    // 连接中状态
    if state.connecting {
        ui.vertical_centered(|ui| {
            ui.add_space(120.0);
            ui.spinner();
            ui.add_space(12.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "正在连接设备…");
        });
        return;
    }

    // 自动扫描（8 秒间隔）
    let now = ctx.input(|i| i.time);
    if !state.scanning
        && (state.usb_devices.is_empty() || now - state.last_scan_time > SCAN_INTERVAL_SECS)
    {
        let _ = cmd_tx.try_send(Command::ScanDevices);
        state.scanning = true;
        state.last_scan_time = now;
    }

    // 过滤 Diamond 设备（clone 避免 borrow 冲突）
    let diamond_devices: Vec<UsbDeviceInfo> = state
        .usb_devices
        .iter()
        .filter(|d| d.vid == DIAMOND_VID)
        .cloned()
        .collect();

    if diamond_devices.is_empty() {
        // ===== no-devices 态 =====
        render_no_devices(ui, ctx, state, cmd_tx);
    } else {
        // ===== with-devices 态 =====
        render_with_devices(ui, state, cmd_tx, &diamond_devices);
    }

    // 强制添加对话框
    if state.show_force_add_dialog {
        render_force_add_dialog(ctx, state, cmd_tx);
    }
}

/// with-devices 态：检测到 Rio 设备，显示大圆球
fn render_with_devices(
    ui: &mut Ui,
    state: &mut AppState,
    cmd_tx: &Sender<Command>,
    devices: &[UsbDeviceInfo],
) {
    // 垂直居中：计算上下 padding 使内容居中
    let available_h = ui.available_height();
    let content_h = 20.0 + 140.0 + 24.0 + 26.0; // text + orb + gap + btn
    let top_pad = ((available_h - content_h) / 2.0).max(20.0);
    ui.add_space(top_pad);
    ui.vertical_centered(|ui| {
        // connect-text
        ui.label(
            egui::RichText::new("检测到 Rio 设备，点击连接")
                .size(13.0)
                .color(theme::RIO_TEXT_SECONDARY),
        );
        ui.add_space(20.0);

        // rio-orbs：horizontal 居中
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 24.0;
            for dev in devices {
                render_rio_orb(ui, dev, state, cmd_tx);
            }
        });
        ui.add_space(24.0);

        // 底部强制添加按钮
        let force_btn = egui::Button::new(
            egui::RichText::new("+ 强制添加任意 USB 设备")
                .size(11.0),
        )
        .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
        .corner_radius(4)
        .min_size(egui::vec2(160.0, 26.0));
        if ui.add(force_btn).clicked() {
            state.show_force_add_dialog = true;
        }
    });
}

/// 渲染单个 rio-orb（140×140 圆形按钮）
fn render_rio_orb(
    ui: &mut Ui,
    dev: &UsbDeviceInfo,
    state: &mut AppState,
    cmd_tx: &Sender<Command>,
) {
    let orb_size = 140.0;
    let orb_id = ui.id().with(("rio_orb", dev.vid, dev.pid));
    let (orb_rect, resp) = ui.allocate_exact_size(
        egui::vec2(orb_size, orb_size),
        egui::Sense::click(),
    );

    // 入场动画（0→1，250ms，对齐 Tauri initial={{ scale: 0.7 }}）
    let entrance = ui
        .ctx()
        .animate_value_with_time(orb_id.with("entrance"), 1.0, 0.25);
    let ent_scale = 0.7 + 0.3 * entrance;

    // hover scale 动画（180ms ease-out，对齐 Tauri transition）
    let hover_target = if resp.hovered() { 1.0 } else { 0.0 };
    let hover_anim = ui
        .ctx()
        .animate_value_with_time(orb_id, hover_target, 0.18);
    let scale = ent_scale * (1.0 + 0.06 * hover_anim);
    let actual_size = orb_size * scale;
    let center = orb_rect.center();
    let scaled_rect = egui::Rect::from_center_size(center, egui::vec2(actual_size, actual_size));

    // 阴影（hover 时加深，动画过渡；入场时淡入）
    let shadow_alpha = (38.0 + 26.0 * hover_anim) * entrance;
    let shadow_color = egui::Color32::from_rgba_premultiplied(
        10,
        108,
        214,
        shadow_alpha as u8,
    );
    ui.painter()
        .rect_filled(scaled_rect, orb_size / 2.0, shadow_color);

    // 圆球主体：白底 + 2px rio-blue 边框（入场淡入）
    let inner_rect = scaled_rect.shrink(4.0);
    let bg_color = theme::RIO_CONTENT_BG.linear_multiply(entrance);
    let border_color = theme::RIO_BLUE.linear_multiply(entrance);
    ui.painter()
        .rect_filled(inner_rect, inner_rect.width() / 2.0, bg_color);
    ui.painter().rect_stroke(
        inner_rect,
        inner_rect.width() / 2.0,
        egui::Stroke::new(2.0, border_color),
        egui::epaint::StrokeKind::Inside,
    );

    // 内容：♪ 图标 + 设备名 + vid:pid（入场淡入）
    let icon_color = theme::RIO_BLUE.linear_multiply(entrance);
    let text_color = theme::RIO_TEXT.linear_multiply(entrance);
    let dim_color = theme::RIO_TEXT_DIM.linear_multiply(entrance);

    let icon_rect = egui::Rect::from_center_size(
        center - egui::vec2(0.0, 28.0),
        egui::vec2(56.0, 28.0),
    );
    ui.painter().text(
        icon_rect.center(),
        egui::Align2::CENTER_CENTER,
        "♪",
        egui::FontId::proportional(28.0),
        icon_color,
    );

    // 设备名（11px 600）
    let name = if dev.name.is_empty() { "Rio" } else { &dev.name };
    let name_rect = egui::Rect::from_center_size(
        center + egui::vec2(0.0, 8.0),
        egui::vec2(inner_rect.width() - 16.0, 16.0),
    );
    ui.painter().text(
        name_rect.center(),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(11.0),
        text_color,
    );

    // vid:pid（10px dim）
    let vidpid = format!("{:04x}:{:04x}", dev.vid, dev.pid);
    let vp_rect = egui::Rect::from_center_size(
        center + egui::vec2(0.0, 26.0),
        egui::vec2(inner_rect.width() - 16.0, 14.0),
    );
    ui.painter().text(
        vp_rect.center(),
        egui::Align2::CENTER_CENTER,
        &vidpid,
        egui::FontId::proportional(10.0),
        dim_color,
    );

    if resp.clicked() {
        state.connecting = true;
        let _ = cmd_tx.try_send(Command::OpenDeviceForce {
            vid: dev.vid,
            pid: dev.pid,
        });
    }
}

/// no-devices 态：简化插图 + 文字
fn render_no_devices(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut AppState,
    cmd_tx: &Sender<Command>,
) {
    // 垂直居中
    let available_h = ui.available_height();
    let content_h = 70.0 + 20.0 + 16.0 + 8.0 + 14.0 + 24.0 + 26.0;
    let top_pad = ((available_h - content_h) / 2.0).max(20.0);
    ui.add_space(top_pad);
    ui.vertical_centered(|ui| {

        // 简化插图：电脑矩形 + 连线 + 设备矩形（painter 绘制）
        let illus_w = 260.0;
        let illus_h = 70.0;
        let (illus_rect, _) = ui.allocate_exact_size(
            egui::vec2(illus_w, illus_h),
            egui::Sense::hover(),
        );
        let painter = ui.painter();
        // 电脑屏幕（90×64 白底 dim 边框）
        let screen_rect = egui::Rect::from_min_size(
            illus_rect.min,
            egui::vec2(90.0, 64.0),
        );
        painter.rect_filled(screen_rect, 4.0, theme::RIO_CONTENT_BG);
        painter.rect_stroke(
            screen_rect,
            4.0,
            egui::Stroke::new(2.0, theme::RIO_TEXT_DIM),
            egui::epaint::StrokeKind::Inside,
        );
        // 屏幕发光（rio-blue-subtle 内框）
        let glow_rect = screen_rect.shrink(4.0);
        painter.rect_filled(glow_rect, 2.0, theme::RIO_SELECTED_BG);
        // 电脑底座
        let stand_rect = egui::Rect::from_min_size(
            screen_rect.center() - egui::vec2(15.0, 0.0),
            egui::vec2(30.0, 4.0),
        );
        painter.rect_filled(stand_rect, 2.0, theme::RIO_TEXT_DIM);
        let base_rect = egui::Rect::from_min_size(
            screen_rect.center() - egui::vec2(25.0, -4.0),
            egui::vec2(50.0, 3.0),
        );
        painter.rect_filled(base_rect, 1.0, theme::RIO_TEXT_DIM);

        // 连线（屏幕右侧到设备）
        let cable_start = screen_rect.right_center();
        let cable_end = cable_start + egui::vec2(80.0, 0.0);
        painter.line_segment(
            [cable_start, cable_end],
            egui::Stroke::new(2.5, theme::RIO_TEXT_DIM),
        );

        // 设备矩形（Rio 播放器，40×56 圆角）
        let device_rect = egui::Rect::from_min_size(
            cable_end + egui::vec2(0.0, -28.0),
            egui::vec2(40.0, 56.0),
        );
        painter.rect_filled(device_rect, 6.0, theme::RIO_CONTENT_BG);
        painter.rect_stroke(
            device_rect,
            6.0,
            egui::Stroke::new(2.0, theme::RIO_BLUE),
            egui::epaint::StrokeKind::Inside,
        );
        // 设备屏幕
        let dev_screen = device_rect.shrink(4.0);
        let dev_screen = egui::Rect::from_min_size(
            dev_screen.min,
            egui::vec2(dev_screen.width(), 24.0),
        );
        painter.rect_filled(dev_screen, 2.0, theme::RIO_SELECTED_BG);

        ui.add_space(20.0);
        // 标题
        ui.label(
            egui::RichText::new("连接 Rio 设备")
                .size(16.0)
                .strong()
                .color(theme::RIO_TEXT),
        );
        ui.add_space(8.0);
        ui.colored_label(
            theme::RIO_TEXT_DIM,
            "将 Rio MP3 播放器通过 USB 连接到电脑",
        );
        ui.add_space(24.0);

        // 强制添加按钮
        let force_btn = egui::Button::new(
            egui::RichText::new("+ 强制添加任意 USB 设备")
                .size(11.0),
        )
        .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
        .corner_radius(4)
        .min_size(egui::vec2(160.0, 26.0));
        if ui.add(force_btn).clicked() {
            state.show_force_add_dialog = true;
        }
    });

    // 连接失败后延迟恢复扫描
    let _ = ctx;
    let _ = cmd_tx;
}

/// 强制添加设备对话框
fn render_force_add_dialog(ctx: &Context, state: &mut AppState, cmd_tx: &Sender<Command>) {
    egui::Window::new("强制添加设备")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            if state.usb_devices.is_empty() {
                ui.colored_label(theme::RIO_TEXT_DIM, "未扫描到任何 USB 设备");
            } else {
                ui.colored_label(theme::RIO_TEXT_DIM, "选择要强制连接的 USB 设备：");
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        let devices = state.usb_devices.clone();
                        for dev in &devices {
                            let label = format!(
                                "{} ({:04x}:{:04x}) — {}",
                                dev.name, dev.vid, dev.pid, dev.manufacturer
                            );
                            if ui.button(label).clicked() {
                                state.connecting = true;
                                state.show_force_add_dialog = false;
                                let _ = cmd_tx.try_send(Command::OpenDeviceForce {
                                    vid: dev.vid,
                                    pid: dev.pid,
                                });
                            }
                        }
                    });
            }
            ui.add_space(8.0);
            ui.separator();
            if ui.button("取消").clicked() {
                state.show_force_add_dialog = false;
            }
        });
}
