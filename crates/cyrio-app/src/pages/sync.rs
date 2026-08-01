//! 同步页：UI 桩（无后端 sync 命令）
//!
//! 对齐 Tauri SyncPane 布局：
//! - pane-header：h2 "歌曲同步" + count-badge + refresh + 添加规则
//! - sync-empty：空态提示

use async_channel::Sender;
use egui::{Layout, Ui};

use crate::message::Command;
use crate::state::AppState;
use crate::theme;

pub fn render(ui: &mut Ui, _ctx: &egui::Context, state: &mut AppState, _cmd_tx: &Sender<Command>) {
    // ===== pane-header：h2 + count-badge + refresh + 添加规则 =====
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("歌曲同步")
                .size(16.0)
                .color(theme::RIO_TEXT),
        );
        ui.add_space(8.0);
        // count-badge
        let badge_btn = egui::Button::new(
            egui::RichText::new("0 条规则")
                .size(10.0)
                .color(theme::RIO_TEXT_DIM),
        )
        .fill(theme::RIO_BG_SUBTLE)
        .corner_radius(4);
        ui.add(badge_btn);

        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // 添加规则
            let add_btn = egui::Button::new(
                egui::RichText::new("+ 添加规则")
                    .size(11.0),
            )
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4)
            .min_size(egui::vec2(90.0, 26.0));
            if ui.add(add_btn).clicked() {
                state.set_status("同步功能开发中");
            }
            // refresh（disabled）
            let refresh_btn = egui::Button::new(
                egui::RichText::new("↻").size(12.0),
            )
            .min_size(egui::vec2(24.0, 24.0))
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4);
            ui.add_enabled(false, refresh_btn);
        });
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);

    // ===== sync-empty：空态 =====
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.label(
            egui::RichText::new("暂无同步规则，点击\"添加规则\"创建")
                .size(12.0)
                .color(theme::RIO_TEXT_DIM),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("镜像同步：本地文件夹为主，设备完全镜像本地内容")
                .size(11.0)
                .color(theme::RIO_TEXT_DIM),
        );
    });
}
