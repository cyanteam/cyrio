//! 设备信息页：存储卡片
//!
//! 对齐 Tauri DeviceInfoPane：
//! - pane-header：h2 "设备信息" + refresh↻
//! - storage-grid：两列 StorageCard（内置 + SD）
//! - StorageCard：storage-title + storage-size(20px) + storage-bar(5px) + storage-detail

use async_channel::Sender;
use egui::{Layout, Ui};

use crate::message::Command;
use crate::state::{format_bytes, AppState};
use crate::theme;
use cyrio_core::protocol::rio_mem::RioMem;

pub fn render(
    ui: &mut Ui,
    _ctx: &egui::Context,
    state: &mut AppState,
    cmd_tx: &Sender<Command>,
) {
    // ===== pane-header：h2 + refresh =====
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("设备信息")
                .size(16.0)
                .color(theme::RIO_TEXT),
        );
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            let refresh_btn = egui::Button::new(
                egui::RichText::new("↻").size(12.0),
            )
            .min_size(egui::vec2(28.0, 28.0))
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4);
            if ui.add_enabled(state.connected, refresh_btn).clicked() {
                let _ = cmd_tx.try_send(Command::GetStorageStatus);
            }
        });
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);

    if !state.connected {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "设备未连接");
            ui.add_space(8.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "请先连接设备");
        });
        return;
    }

    // ===== storage-grid：两列 =====
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        // 内置存储
        let half_w = (ui.available_width() - 10.0) / 2.0;
        ui.allocate_ui_with_layout(
            egui::vec2(half_w, 120.0),
            Layout::top_down(egui::Align::LEFT),
            |ui| {
                render_storage_card(ui, "内置存储", state.internal_mem.as_ref());
            },
        );
        // SD 卡
        ui.allocate_ui_with_layout(
            egui::vec2(half_w, 120.0),
            Layout::top_down(egui::Align::LEFT),
            |ui| {
                render_storage_card(ui, "SD 卡", state.sd_mem.as_ref());
            },
        );
    });
}

/// 渲染单个 StorageCard（对齐 .storage-card）
fn render_storage_card(ui: &mut Ui, title: &str, mem: Option<&RioMem>) {
    let is_present = mem.map(|m| m.is_present()).unwrap_or(false);
    let card_w = ui.available_width();
    let card_h = 120.0;

    ui.allocate_ui_with_layout(
        egui::vec2(card_w, card_h),
        Layout::top_down(egui::Align::LEFT),
        |ui| {
            let rect = ui.max_rect();
            // 卡片背景
            let card_bg = if is_present {
                theme::RIO_BG_SUBTLE
            } else {
                egui::Color32::from_rgba_premultiplied(
                    theme::RIO_BG_SUBTLE.r(),
                    theme::RIO_BG_SUBTLE.g(),
                    theme::RIO_BG_SUBTLE.b(),
                    128,
                )
            };
            ui.painter().rect_filled(rect, 6.0, card_bg);
            ui.painter().rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.0, theme::RIO_BORDER_LIGHT),
                egui::epaint::StrokeKind::Inside,
            );

            ui.spacing_mut().item_spacing.y = 6.0;
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(title)
                        .size(11.0)
                        .color(theme::RIO_TEXT_DIM),
                );
            });

            if is_present {
                if let Some(m) = mem {
                    // storage-size
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(format_bytes(m.size as u64))
                                .size(20.0)
                                .strong()
                                .color(theme::RIO_TEXT),
                        );
                    });

                    // storage-bar
                    let used_pct = if m.size > 0 {
                        (m.used as f32 / m.size as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let bar_w = ui.available_width() - 24.0;
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, 5.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(bar_rect, 2.0, theme::RIO_CONTENT_BG);
                        let fill_w = bar_rect.width() * used_pct;
                        let fill_rect = egui::Rect::from_min_size(
                            bar_rect.min,
                            egui::vec2(fill_w, bar_rect.height()),
                        );
                        ui.painter().rect_filled(fill_rect, 2.0, theme::RIO_BLUE);
                    });

                    // storage-detail
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "已用 {} / 可用 {}",
                                format_bytes(m.used as u64),
                                format_bytes(m.free as u64)
                            ))
                            .size(10.0)
                            .color(theme::RIO_TEXT_DIM),
                        );
                    });
                }
            } else {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("未插入")
                            .size(12.0)
                            .color(theme::RIO_TEXT_DIM),
                    );
                });
            }
        },
    );
}
