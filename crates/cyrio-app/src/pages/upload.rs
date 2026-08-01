//! 上传页：选择文件 + 目标内存单元 + 上传进度
//!
//! 对齐 Tauri UploadPane：
//! - pane-header：h2 "上传" + mem-switch（内置/SD卡）
//! - upload-zone：upload-hint + upload-btn "选择文件"
//! - 点击按钮 → rfd 选文件 → 立即上传
//! - 拖拽文件 → 追加 pending_uploads → 自动上传

use async_channel::Sender;
use egui::{Color32, Layout, Ui};

use crate::message::Command;
use crate::state::{AppState, ProgressKind};
use crate::theme;

pub fn render(ui: &mut Ui, ctx: &egui::Context, state: &mut AppState, cmd_tx: &Sender<Command>) {
    // ===== pane-header：h2 "上传" + mem-switch =====
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("上传")
                .size(16.0)
                .color(theme::RIO_TEXT),
        );
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // mem-switch：内置 / SD 卡
            let opts: [(u8, &str); 2] = [(0, "内置"), (1, "SD 卡")];
            for (val, label) in opts {
                let is_active = state.upload_target_mem == val;
                let btn = if is_active {
                    egui::Button::new(
                        egui::RichText::new(label).size(11.0).color(Color32::WHITE),
                    )
                    .fill(theme::RIO_BLUE)
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BLUE))
                    .corner_radius(4)
                    .min_size(egui::vec2(48.0, 24.0))
                } else {
                    egui::Button::new(
                        egui::RichText::new(label).size(11.0),
                    )
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                    .corner_radius(4)
                    .min_size(egui::vec2(48.0, 24.0))
                };
                if ui.add(btn).clicked() {
                    state.upload_target_mem = val;
                }
            }
        });
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ===== upload-zone：居中 hint + btn =====
    let zone_h = ui.available_height() - if state.progress.is_some() { 40.0 } else { 0.0 };
    let zone_h = zone_h.max(120.0);
    let zone_w = ui.available_width();

    ui.allocate_ui_with_layout(
        egui::vec2(zone_w, zone_h),
        Layout::top_down(egui::Align::TOP),
        |ui| {
            let zone_rect = ui.max_rect();
            // 检测拖拽 hover
            let is_hover = ctx.input(|i| {
                i.pointer
                    .interact_pos()
                    .map(|p| zone_rect.contains(p))
                    .unwrap_or(false)
            });

            // 绘制 upload-zone 背景
            let zone_bg = if is_hover {
                theme::RIO_SELECTED_BG
            } else {
                theme::RIO_BG_SUBTLE
            };
            ui.painter().rect_filled(zone_rect, 6.0, zone_bg);
            let zone_stroke = if is_hover {
                egui::Stroke::new(2.0, theme::RIO_BLUE)
            } else {
                egui::Stroke::new(1.0, theme::RIO_BORDER)
            };
            ui.painter().rect_stroke(
                zone_rect,
                6.0,
                zone_stroke,
                egui::epaint::StrokeKind::Inside,
            );

            // 居中内容
            ui.vertical_centered(|ui| {
                ui.add_space((zone_rect.height() / 2.0 - 50.0).max(20.0));

                // upload-hint
                ui.label(
                    egui::RichText::new("选择 MP3 文件，或直接拖拽到任意位置")
                        .size(12.0)
                        .color(theme::RIO_TEXT_DIM),
                );
                ui.add_space(12.0);

                // upload-btn
                let btn_label = if state.progress.is_some() {
                    "上传中…"
                } else {
                    "选择文件"
                };
                let upload_btn = egui::Button::new(
                    egui::RichText::new(btn_label).size(12.0).color(Color32::WHITE),
                )
                .fill(theme::RIO_BLUE)
                .stroke(egui::Stroke::new(1.0, theme::RIO_BLUE))
                .corner_radius(4)
                .min_size(egui::vec2(100.0, 32.0));
                let uploading = state.progress.is_some();
                if ui.add_enabled(state.connected && !uploading, upload_btn).clicked() {
                    if let Some(paths) = pick_mp3_files() {
                        if !paths.is_empty() {
                            start_upload(state, cmd_tx, paths);
                        }
                    }
                }
            });
        },
    );

    // 处理拖拽文件
    let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.clone())
            .collect()
    });
    if !dropped.is_empty() && state.connected && state.progress.is_none() {
        start_upload(state, cmd_tx, dropped);
    }

    // 进度条
    if let Some(p) = &state.progress {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(p.label())
                .size(11.0)
                .color(theme::RIO_TEXT_DIM),
        );
        ui.add(egui::ProgressBar::new(p.fraction()).desired_height(20.0));
    }
}

/// 启动批量上传
///
/// 从 AppSettings 构造 UploadTextOptions 传入，与 playlist 编码同步：
/// 解决"歌曲传输编码没同步"问题（playlist 已能正确写中文，song 上传需同步应用 slug/strip）。
fn start_upload(
    state: &mut AppState,
    cmd_tx: &Sender<Command>,
    paths: Vec<std::path::PathBuf>,
) {
    let total: u64 = paths.iter().map(|p| file_size(p)).sum();
    // 从持久化设置构造文本处理选项（slug/strip 与上传编码同步）
    let text_opts = cyrio_core::api::upload::UploadTextOptions {
        apply_slug: state.settings.upload_apply_slug,
        apply_strip: state.settings.upload_apply_strip,
        strip_parentheses: state.settings.strip_parentheses,
        strip_quotes: state.settings.strip_quotes,
        strip_quality_tags: state.settings.strip_quality_tags,
        custom_stop_words: state.settings.custom_words_vec(),
    };
    let _ = cmd_tx.try_send(Command::UploadSongBatch {
        paths,
        mem_unit: state.upload_target_mem,
        text_opts,
    });
    state.progress = Some(crate::state::ProgressInfo {
        kind: ProgressKind::Upload,
        current: 0,
        total,
    });
    // 不再显示 loading modal——传输对话框 (upload_transfer) 会替代它
}

/// 弹出文件选择对话框
fn pick_mp3_files() -> Option<Vec<std::path::PathBuf>> {
    rfd::FileDialog::new()
        .add_filter("MP3 音频", &["mp3"])
        .set_title("选择 MP3 文件")
        .pick_files()
}

/// 获取文件大小
fn file_size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
