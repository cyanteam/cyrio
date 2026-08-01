//! 设置页：外观/上传文本处理选项/自定义停用词
//!
//! 对齐 Tauri SettingsPane：
//! - pane-header：h2 "设置"
//! - 外观：现代/经典 单选（经典 1:1 复刻原版样式，暂未实现 → 禁用并标注"敬请期待"）
//! - 上传文本处理：apply_slug / apply_strip 总开关 + strip 子选项
//! - 自定义停用词：多行文本编辑（每行一个词）
//!
//! 任何修改即时调用 `settings.save()` 持久化（不依赖"保存"按钮）。

use async_channel::Sender;
use egui::Ui;

use crate::message::Command;
use crate::state::AppState;
use crate::theme;

/// 渲染设置页
pub fn render(ui: &mut Ui, _ctx: &egui::Context, state: &mut AppState, _cmd_tx: &Sender<Command>) {
    // ===== pane-header =====
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("设置")
                .size(16.0)
                .color(theme::RIO_TEXT),
        );
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);

    ui.vertical(|ui| {
        // ===== 外观 =====
        section_title(ui, "外观");
        ui.add_space(4.0);
        let opts: [(u8, &str, bool); 2] = [
            (0, "现代（默认主题）", true),
            (1, "经典（1:1 复刻 Rio Music Manager 原版样式）", false),
        ];
        for (val, label, enabled) in opts {
            let is_active = state.settings.appearance == val;
            let btn = if is_active {
                egui::Button::new(
                    egui::RichText::new(format!("● {}", label))
                        .size(11.0)
                        .color(egui::Color32::WHITE),
                )
                .fill(theme::RIO_BLUE)
                .stroke(egui::Stroke::new(1.0, theme::RIO_BLUE))
                .corner_radius(4)
                .min_size(egui::vec2(0.0, 26.0))
            } else {
                egui::Button::new(
                    egui::RichText::new(format!("○ {}", label))
                        .size(11.0)
                        .color(theme::RIO_TEXT),
                )
                .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                .corner_radius(4)
                .min_size(egui::vec2(0.0, 26.0))
            };
            if ui.add_enabled(enabled, btn).clicked() {
                state.settings.appearance = val;
                state.settings.save();
                if val == 1 {
                    state.set_status("经典主题尚未实现，敬请期待");
                }
            }
            ui.add_space(2.0);
        }
        ui.colored_label(
            theme::RIO_TEXT_DIM,
            egui::RichText::new("经典主题将 1:1 复刻原版 Rio Music Manager 软件的样式和布局。")
                .size(10.0),
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        // ===== 上传文本处理 =====
        section_title(ui, "上传文本处理");
        ui.add_space(4.0);
        ui.colored_label(
            theme::RIO_TEXT_DIM,
            egui::RichText::new("上传 MP3 时是否对标题应用文本处理（与歌单编码同步）。")
                .size(10.0),
        );
        ui.add_space(6.0);

        // apply_slug 总开关
        if toggle_row(ui, "上传时转拼音（slug）", "把标题中中文转为拼音，便于 Rio 设备字库显示。", &mut state.settings.upload_apply_slug) {
            state.settings.save();
        }
        if toggle_row(ui, "上传时去词（strip）", "移除标题中的 Hi-Res、4K、括号内容等无关词汇。", &mut state.settings.upload_apply_strip) {
            state.settings.save();
        }

        // strip 子选项（仅在 apply_strip 开启时可编辑）
        ui.add_space(4.0);
        ui.indent("strip_sub_opts", |ui| {
            ui.add_enabled_ui(state.settings.upload_apply_strip, |ui| {
                if toggle_row(ui, "去除括号内容", "移除 (...) （...） [...] 【...】 {...} 等括号包裹的内容。", &mut state.settings.strip_parentheses) {
                    state.settings.save();
                }
                if toggle_row(ui, "去除引号内容", "移除 \"...\" “...” '...' 等引号包裹的内容（歌词片段等）。", &mut state.settings.strip_quotes) {
                    state.settings.save();
                }
                if toggle_row(ui, "去除音质/规格停用词", "移除 Hi-Res、无损、4K、高清、bilibili 等内置停用词。", &mut state.settings.strip_quality_tags) {
                    state.settings.save();
                }
            });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        // ===== 自定义停用词 =====
        section_title(ui, "自定义停用词");
        ui.add_space(4.0);
        ui.colored_label(
            theme::RIO_TEXT_DIM,
            egui::RichText::new("每行一个词。strip 启用时，这些词会从标题中移除。")
                .size(10.0),
        );
        ui.add_space(6.0);
        let resp = ui.add_sized(
            egui::vec2(ui.available_width().max(360.0), 120.0),
            egui::TextEdit::multiline(&mut state.settings.custom_stop_words)
                .desired_width(f32::MAX)
                .code_editor()
                .hint_text("例如：\n在百万级播音室大声听\n现场版\n官方MV"),
        );
        if resp.changed() {
            state.settings.save();
        }
        ui.add_space(4.0);
        ui.colored_label(
            theme::RIO_TEXT_DIM,
            egui::RichText::new(format!(
                "当前共 {} 个自定义停用词",
                state.settings.custom_words_vec().len()
            ))
            .size(10.0),
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        // ===== 其他 =====
        section_title(ui, "其他");
        ui.add_space(4.0);
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("重置为默认设置").color(theme::RIO_DANGER),
                )
                .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                .corner_radius(4)
                .min_size(egui::vec2(120.0, 26.0)),
            )
            .clicked()
        {
            state.settings = crate::state::AppSettings::default();
            state.settings.save();
            state.set_status("已重置为默认设置");
        }
    });
}

/// 小节标题（13px 加粗主色）
fn section_title(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .strong()
            .color(theme::RIO_TEXT),
    );
}

/// 单行开关：[checkbox] [标签 + 描述]
///
/// 返回 true 表示值被用户修改（caller 据此调用 settings.save()）。
fn toggle_row(ui: &mut Ui, label: &str, desc: &str, value: &mut bool) -> bool {
    let old = *value;
    ui.horizontal(|ui| {
        ui.add(egui::Checkbox::new(value, ""));
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(11.5)
                    .color(theme::RIO_TEXT),
            );
            ui.colored_label(
                theme::RIO_TEXT_DIM,
                egui::RichText::new(desc).size(10.0),
            );
        });
    });
    let changed = *value != old;
    if changed {
        ui.ctx().request_repaint();
    }
    ui.add_space(2.0);
    changed
}
