//! 关于页：应用信息（左对齐，去 AI 味）
//!
//! 对齐 Tauri AboutPane：
//! - pane-header：h2 "关于"
//! - about-content：左对齐，logo 行水平排列 + 信息列表 + credits

use async_channel::Sender;
use egui::Ui;

use crate::message::Command;
use crate::state::AppState;
use crate::theme;

pub fn render(ui: &mut Ui, _ctx: &egui::Context, _state: &mut AppState, _cmd_tx: &Sender<Command>) {
    // ===== pane-header =====
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("关于")
                .size(16.0)
                .color(theme::RIO_TEXT),
        );
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);

    // ===== about-content：左对齐 =====
    ui.vertical(|ui| {
        ui.add_space(8.0);

        // logo 行：24×24 小图标 + 名称 + 版本号（水平排列）
        ui.horizontal(|ui| {
            let (icon_rect, _) = ui.allocate_exact_size(
                egui::vec2(24.0, 24.0),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(icon_rect, 4.0, theme::RIO_BLUE);
            ui.painter().text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                "♪",
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );

            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("cyrio")
                    .size(14.0)
                    .strong()
                    .color(theme::RIO_TEXT),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("v0.1.0")
                    .size(11.0)
                    .color(theme::RIO_TEXT_DIM),
            );
        });

        ui.add_space(16.0);

        // 信息列表：左对齐 [label 70px][value]
        let rows: [(&str, &str); 3] = [
            ("作者", "cyanteam"),
            ("GitHub", "github.com/cyanteam"),
            ("邮箱", "qtof@qq.com"),
        ];
        for (label, value) in rows {
            ui.horizontal(|ui| {
                let (label_rect, _) = ui.allocate_exact_size(
                    egui::vec2(70.0, 18.0),
                    egui::Sense::hover(),
                );
                ui.painter().text(
                    label_rect.min,
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(11.0),
                    theme::RIO_TEXT_DIM,
                );
                ui.label(
                    egui::RichText::new(value)
                        .size(12.0)
                        .color(theme::RIO_TEXT),
                );
            });
            ui.add_space(4.0);
        }

        ui.add_space(16.0);

        // credits：左对齐 bullet 列表
        ui.label(
            egui::RichText::new("· 基于 Rio Receiver USB 协议逆向工程实现")
                .size(10.0)
                .color(theme::RIO_TEXT_DIM),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("· 支持 Rio S50 / S30S · 跨存储歌单 · GBK/UTF-8 智能解码")
                .size(10.0)
                .color(theme::RIO_TEXT_DIM),
        );
    });
}
