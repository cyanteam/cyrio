//! 歌单页：表格布局 + 多选 + 双击进入详情
//!
//! 对齐 Tauri PlaylistsPane：
//! - 列表视图：BatchToolbar(无加入歌单) + FilterBar(名称/大小) + song-table 3 列
//!   [row-check+歌单名][大小][mem-badge] + 分页(10/页) + 新建歌单按钮
//! - 详情视图：← 返回 + 歌单名 + song-table(#/标题/艺术家/时长)
//! - 双击行进入详情；多选复用 songs.rs 的 row-check + mem-badge 样式

use async_channel::Sender;
use egui::{Context, Key, Ui};

use crate::message::Command;
use crate::state::{format_bytes, AppState, ConfirmAction, SongEntry};
use crate::theme;
use cyrio_core::protocol::rio_file::RioFile;

/// 每页歌单数（对齐 Tauri paginate=10）
const PLAYLISTS_PER_PAGE: usize = 10;

/// 渲染歌单页
pub fn render(ui: &mut Ui, ctx: &Context, state: &mut AppState, cmd_tx: &Sender<Command>) {
    // 详情视图
    if state.active_playlist.is_some() {
        render_detail_view(ui, state, cmd_tx);
        return;
    }

    // ===== 列表视图 =====
    // BatchToolbar 容器（始终渲染，保持 UI 树结构稳定）
    // 若条件渲染，选中状态变化会导致后续行 allocate_exact_size 自动 Id 偏移，
    // double_clicked() 失效（双击进入详情无法触发）。
    render_batch_toolbar(ui, state, cmd_tx);
    ui.add_space(4.0);

    // FilterBar
    render_filter_bar(ui, state);
    ui.add_space(4.0);

    // 加载中 / 空状态
    if state.loading && state.playlists.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.spinner();
            ui.add_space(8.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "加载中…");
        });
        return;
    }

    if state.playlists.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "暂无歌单");
            if !state.connected {
                ui.add_space(10.0);
                ui.colored_label(theme::RIO_TEXT_DIM, "请先连接设备");
            }
        });
        return;
    }

    // 过滤 + 排序
    let filtered: Vec<SongEntry> = filter_and_sort_playlists(
        &state.playlists,
        &state.playlist_search_query,
        state.playlist_sort_by,
    );

    // 分页
    let (page_items, total_pages, current_page) = if state.paginate {
        let per_page = PLAYLISTS_PER_PAGE;
        let total = filtered.len();
        let total_pages = total.div_ceil(per_page).max(1);
        let page = state.playlist_current_page.min(total_pages - 1);
        let start = page * per_page;
        let end = (start + per_page).min(total);
        (filtered[start.min(total)..end].to_vec(), total_pages, page)
    } else {
        (filtered.clone(), 1, 0)
    };

    let total_count = state.playlists.len();
    let shown_count = filtered.len();

    // 计数行
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("共 {} 个（显示 {}）", total_count, shown_count))
                .size(11.0)
                .color(theme::RIO_TEXT_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !state.selected_playlist_keys.is_empty() {
                ui.colored_label(
                    theme::RIO_BLUE,
                    egui::RichText::new(format!("已选 {} 项", state.selected_playlist_keys.len()))
                        .size(11.0),
                );
            }
        });
    });
    ui.add_space(4.0);

    // ===== 表格（手动行布局，不用 Grid 避免 max_rect 问题）=====
    // macOS 用 Cmd 多选，Windows/Linux 用 Ctrl；egui 的 command 字段跨平台
    // 修饰键在点击处理器内读取，确保获取当前帧最新状态

    let total_w = ui.available_width();
    let col_check = 22.0;
    let col_size = 80.0;
    let col_mem = 32.0;
    let spacing: f32 = 8.0;
    let col_name = (total_w - col_check - col_size - col_mem - spacing * 3.0 - 8.0).max(100.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        // 表头
        let h = 24.0;
        let (hdr_rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
        ui.painter().rect_filled(hdr_rect, 0.0, theme::RIO_BG_SUBTLE);
        let font_h = egui::FontId::proportional(11.0);
        let mut xh = hdr_rect.min.x + 8.0 + col_check + spacing;
        let yh = hdr_rect.center().y;
        ui.painter().text(egui::pos2(xh, yh), egui::Align2::LEFT_CENTER, "歌单名", font_h.clone(), theme::RIO_TEXT_DIM);
        xh += col_name + spacing;
        ui.painter().text(egui::pos2(xh, yh), egui::Align2::LEFT_CENTER, "大小", font_h.clone(), theme::RIO_TEXT_DIM);

        // 数据行
        for (idx, entry) in page_items.iter().enumerate() {
            let key = (entry.file.file_no, entry.mem_unit);
            let is_selected = state.selected_playlist_keys.contains(&key);
            let global_idx = if state.paginate {
                current_page * PLAYLISTS_PER_PAGE + idx
            } else {
                idx
            };

            let (row_clicked, double_clicked, secondary_clicked) = render_playlist_row(
                ui, ctx, &entry.file, entry.mem_unit, is_selected,
                col_check, col_name, col_size, col_mem, spacing,
            );

            // 双击进入详情
            if double_clicked {
                state.active_playlist = Some(key);
                state.loading_playlist_songs = true;
                state.playlist_songs.clear();
                let _ = cmd_tx.try_send(Command::ListPlaylistSongs {
                    playlist_file_no: entry.file.file_no,
                    mem_unit: entry.mem_unit,
                });
            }

            // 行点击 — 选择逻辑（直接点击即切换选中，无需修饰键）
            if row_clicked || secondary_clicked {
                let shift = ctx.input(|i| i.modifiers.shift);
                if shift {
                    // Shift+Click：范围选择
                    if let Some(last_idx) = state.last_clicked_song_index {
                        let start = last_idx.min(global_idx);
                        let end = last_idx.max(global_idx);
                        state.selected_playlist_keys.clear();
                        for e in &filtered[start..=end] {
                            state.selected_playlist_keys.insert((e.file.file_no, e.mem_unit));
                        }
                    } else {
                        state.selected_playlist_keys.clear();
                        state.selected_playlist_keys.insert(key);
                    }
                } else {
                    // 普通点击：切换该行选中状态
                    if is_selected {
                        state.selected_playlist_keys.remove(&key);
                    } else {
                        state.selected_playlist_keys.insert(key);
                    }
                }
                state.last_clicked_song_index = Some(global_idx);
            }
        }
    });

    // Ctrl+A / Cmd+A 全选（command 跨平台：macOS=Cmd, Windows/Linux=Ctrl）
    if ctx.input(|i| i.key_pressed(Key::A) && (i.modifiers.command || i.modifiers.ctrl)) {
        for e in &state.playlists {
            state
                .selected_playlist_keys
                .insert((e.file.file_no, e.mem_unit));
        }
    }

    // 分页栏
    if state.paginate && total_pages > 1 {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let prev_btn = egui::Button::new(
                egui::RichText::new("◀ 上一页").size(11.0),
            )
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4)
            .min_size(egui::vec2(80.0, 24.0));
            if ui.add_enabled(current_page > 0, prev_btn).clicked() {
                state.playlist_current_page = current_page - 1;
            }
            ui.label(
                egui::RichText::new(format!("第 {} / {} 页", current_page + 1, total_pages))
                    .size(11.0)
                    .color(theme::RIO_TEXT_DIM),
            );
            let next_btn = egui::Button::new(
                egui::RichText::new("下一页 ▶").size(11.0),
            )
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4)
            .min_size(egui::vec2(80.0, 24.0));
            if ui.add_enabled(current_page < total_pages - 1, next_btn)
                .clicked()
            {
                state.playlist_current_page = current_page + 1;
            }
        });
    }

    // 底部新建歌单按钮
    ui.add_space(8.0);
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("+ 新建歌单").size(11.0),
            )
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4)
            .min_size(egui::vec2(100.0, 26.0)),
        )
        .clicked()
        && state.connected
    {
        state.show_create_playlist_dialog = true;
    }

    // 新建歌单对话框
    if state.show_create_playlist_dialog {
        render_create_playlist_dialog(ctx, state, cmd_tx);
    }
}

/// 详情视图：← 返回 + 歌单名 + 歌曲表格
fn render_detail_view(ui: &mut Ui, state: &mut AppState, _cmd_tx: &Sender<Command>) {
    let active = state.active_playlist;
    // 查找歌单名
    let playlist_name = active
        .and_then(|(fn_, mu)| {
            state
                .playlists
                .iter()
                .find(|e| e.file.file_no == fn_ && e.mem_unit == mu)
                .map(|e| display_playlist_name(&e.file))
        })
        .unwrap_or_default();

    // pane-header：← 返回 + 歌单名
    ui.horizontal(|ui| {
        let back_btn = egui::Button::new(
            egui::RichText::new("← 返回").size(11.0),
        )
        .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
        .corner_radius(4)
        .min_size(egui::vec2(72.0, 26.0));
        if ui.add(back_btn).clicked() {
            state.active_playlist = None;
            state.playlist_songs.clear();
        }
        ui.heading(
            egui::RichText::new(playlist_name.clone())
                .size(16.0)
                .color(theme::RIO_TEXT),
        );
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    if state.loading_playlist_songs {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.spinner();
            ui.add_space(8.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "加载中…");
        });
        return;
    }

    if state.playlist_songs.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "歌单为空");
        });
        return;
    }

    // 手动行布局（与列表视图一致，避免 Grid 对齐问题）
    let total_w = ui.available_width();
    let col_idx = 36.0;
    let col_time = 50.0;
    let spacing: f32 = 8.0;
    let col_artist = 120.0;
    let col_title = (total_w - col_idx - col_artist - col_time - spacing * 3.0 - 8.0).max(100.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        // 表头
        let h = 24.0;
        let (hdr_rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
        ui.painter().rect_filled(hdr_rect, 0.0, theme::RIO_BG_SUBTLE);
        let font_h = egui::FontId::proportional(11.0);
        let mut xh = hdr_rect.min.x + 8.0;
        let yh = hdr_rect.center().y;
        ui.painter().text(egui::pos2(xh, yh), egui::Align2::LEFT_CENTER, "#", font_h.clone(), theme::RIO_TEXT_DIM);
        xh += col_idx + spacing;
        ui.painter().text(egui::pos2(xh, yh), egui::Align2::LEFT_CENTER, "标题", font_h.clone(), theme::RIO_TEXT_DIM);
        xh += col_title + spacing;
        ui.painter().text(egui::pos2(xh, yh), egui::Align2::LEFT_CENTER, "艺术家", font_h.clone(), theme::RIO_TEXT_DIM);
        xh += col_artist + spacing;
        ui.painter().text(egui::pos2(xh, yh), egui::Align2::LEFT_CENTER, "时长", font_h.clone(), theme::RIO_TEXT_DIM);

        // 数据行
        let row_h = 28.0;
        let font_11 = egui::FontId::proportional(11.0);
        let font_115 = egui::FontId::proportional(11.5);
        for ps in &state.playlist_songs {
            let (row_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), row_h),
                egui::Sense::hover(),
            );

            // hover 背景
            let row_id = ui.id().with(("detail_row", ps.index));
            let hover_target = if row_rect.is_positive() && ui.rect_contains_pointer(row_rect) { 1.0 } else { 0.0 };
            let hover_anim = ui.ctx().animate_value_with_time(row_id, hover_target, 0.15);
            if hover_anim > 0.001 {
                let bg = theme::lerp_color(theme::RIO_CONTENT_BG, theme::RIO_HOVER_BG, hover_anim);
                ui.painter().rect_filled(row_rect, 0.0, bg);
            }

            let mut x = row_rect.min.x + 8.0;
            let y = row_rect.center().y;

            // 列1: #
            ui.painter().text(
                egui::pos2(x, y),
                egui::Align2::LEFT_CENTER,
                format!("{}", ps.index),
                font_11.clone(),
                theme::RIO_TEXT_DIM,
            );
            x += col_idx + spacing;

            // 列2: 标题（截断）
            let title_text = display_song_title(&ps.song);
            let title_display = truncate_text(&title_text, col_title, &font_115, ui);
            ui.painter().text(
                egui::pos2(x, y),
                egui::Align2::LEFT_CENTER,
                &title_display,
                font_115.clone(),
                theme::RIO_TEXT,
            );
            x += col_title + spacing;

            // 列3: 艺术家
            let artist = if ps.song.artist.is_empty() { "未知艺术家" } else { &ps.song.artist };
            let artist_display = truncate_text(artist, col_artist, &font_11, ui);
            ui.painter().text(
                egui::pos2(x, y),
                egui::Align2::LEFT_CENTER,
                &artist_display,
                font_11.clone(),
                theme::RIO_TEXT_DIM,
            );
            x += col_artist + spacing;

            // 列4: 时长
            ui.painter().text(
                egui::pos2(x, y),
                egui::Align2::LEFT_CENTER,
                format_time(ps.song.time),
                font_11.clone(),
                theme::RIO_TEXT_DIM,
            );
        }
    });
}

/// BatchToolbar（对齐 Tauri，无"加入歌单"）
/// 始终渲染 ui.horizontal 容器（即使未选中），保持 UI 树结构稳定，
/// 防止后续行的 allocate_exact_size 自动 Id 因容器增减而偏移，导致 double_clicked 失效。
fn render_batch_toolbar(ui: &mut Ui, state: &mut AppState, cmd_tx: &Sender<Command>) {
    ui.horizontal(|ui| {
        // 未选中时不渲染按钮，但保留容器占位（高度 26px 与按钮一致）
        if state.selected_playlist_keys.is_empty() {
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
            return;
        }
        ui.spacing_mut().item_spacing.x = 4.0;
        // 全选
        if batch_btn(ui, "全选").clicked() {
            for e in &state.playlists {
                state
                    .selected_playlist_keys
                    .insert((e.file.file_no, e.mem_unit));
            }
        }
        // 清空
        if batch_btn(ui, "清空").clicked() {
            state.selected_playlist_keys.clear();
            state.last_clicked_song_index = None;
        }
        // 批量删除
        if batch_btn(ui, "删除").clicked() {
            let snapshot: Vec<(u32, u8)> = state.selected_playlist_keys.iter().cloned().collect();
            let count = snapshot.len();
            // 按 mem_unit 分组，取第一组做确认（简化）
            let mut by_mem: std::collections::HashMap<u8, Vec<u32>> = std::collections::HashMap::new();
            for (fn_, mu) in &snapshot {
                by_mem.entry(*mu).or_default().push(*fn_);
            }
            if let Some((&mu, file_nos)) = by_mem.iter().next() {
                state.confirm(
                    ConfirmAction::DeleteSongsBatch {
                        file_nos: file_nos.clone(),
                        mem_unit: mu,
                    },
                    format!("确认删除选中的 {} 个歌单？", count),
                );
            }
        }
        // 右侧刷新
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let refresh_btn = egui::Button::new(
                egui::RichText::new("↻").size(12.0),
            )
            .min_size(egui::vec2(24.0, 24.0))
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4);
            if ui.add_enabled(state.connected, refresh_btn).clicked() {
                let _ = cmd_tx.try_send(Command::ListPlaylists(crate::state::MEM_UNIT_INTERNAL));
                let _ = cmd_tx.try_send(Command::ListPlaylists(crate::state::MEM_UNIT_SD));
                state.loading = true;
            }
        });
    });
}

/// batch-btn 样式（11px，4px 圆角，固定尺寸防止位移）
/// 不设显式 fill/color — 让 egui 状态样式处理 hover/press 配色
fn batch_btn(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(theme::RIO_TEXT).size(11.0))
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4)
            .min_size(egui::vec2(label.len() as f32 * 9.0 + 16.0, 26.0)),
    )
}

/// FilterBar（排序 名称/大小 + 搜索）
fn render_filter_bar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(egui::RichText::new("排序").size(11.0).color(theme::RIO_TEXT_DIM));
        let sorts = [("名称", 0u8), ("大小", 1u8)];
        for (label, val) in sorts {
            let is_active = state.playlist_sort_by == val;
            let btn = if is_active {
                egui::Button::new(egui::RichText::new(label).size(11.0).color(egui::Color32::WHITE))
                    .fill(theme::RIO_BLUE)
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BLUE))
                    .corner_radius(3)
                    .min_size(egui::vec2(44.0, 22.0))
            } else {
                egui::Button::new(egui::RichText::new(label).size(11.0))
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                    .corner_radius(3)
                    .min_size(egui::vec2(44.0, 22.0))
            };
            if ui.add(btn).clicked() {
                state.playlist_sort_by = val;
            }
        }
        ui.separator();
        ui.label(egui::RichText::new("搜索").size(11.0).color(theme::RIO_TEXT_DIM));
        ui.add_sized(
            egui::vec2(150.0, 22.0),
            egui::TextEdit::singleline(&mut state.playlist_search_query),
        );
    });
}

/// 渲染一行歌单，返回 (行是否点击, 是否双击)
/// 手动行布局：allocate_exact_size 获取正确行 rect，painter 逐列绘制
#[allow(clippy::too_many_arguments)]
fn render_playlist_row(
    ui: &mut Ui,
    ctx: &Context,
    pl: &RioFile,
    mem_unit: u8,
    is_selected: bool,
    col_check: f32,
    col_name: f32,
    col_size: f32,
    col_mem: f32,
    spacing: f32,
) -> (bool, bool, bool) {
    let total_w = ui.available_width();
    let row_h = 28.0;
    let row_id = ui.id().with(("playlist_row", pl.file_no, mem_unit));

    // 分配行空间 + 获取交互响应（关键：用 allocate_exact_size 而非 max_rect）
    let (row_rect, row_resp) =
        ui.allocate_exact_size(egui::vec2(total_w, row_h), egui::Sense::click());

    // hover 动画（150ms，对齐 Tauri cubic-bezier）
    let hover_target = if row_resp.hovered() { 1.0 } else { 0.0 };
    let hover_anim = ctx.animate_value_with_time(row_id, hover_target, 0.15);

    // 绘制行背景
    if is_selected {
        ui.painter().rect_filled(row_rect, 0.0, theme::RIO_CHECKED_BG);
        let indicator =
            egui::Rect::from_min_size(row_rect.min, egui::vec2(3.0, row_rect.height()));
        ui.painter().rect_filled(indicator, 0.0, theme::RIO_BLUE_LIGHT);
    } else if hover_anim > 0.001 {
        let bg = theme::lerp_color(theme::RIO_CONTENT_BG, theme::RIO_HOVER_BG, hover_anim);
        ui.painter().rect_filled(row_rect, 0.0, bg);
    }

    // ===== 在行 rect 上绘制内容 =====
    let mut x = row_rect.min.x + 8.0;
    let y = row_rect.center().y;
    let font_11 = egui::FontId::proportional(11.0);
    let font_115 = egui::FontId::proportional(11.5);

    // 列1：row-check
    let check_center = egui::pos2(x + 7.0, y);
    let check_rect = egui::Rect::from_center_size(check_center, egui::vec2(14.0, 14.0));
    if is_selected {
        ui.painter().rect_filled(check_rect, 3.0, theme::RIO_BLUE);
        let p1 = check_rect.min + egui::vec2(3.0, 7.0);
        let p2 = check_rect.min + egui::vec2(6.0, 10.0);
        let p3 = check_rect.min + egui::vec2(11.0, 4.0);
        ui.painter()
            .line_segment([p1, p2], egui::Stroke::new(1.5, egui::Color32::WHITE));
        ui.painter()
            .line_segment([p2, p3], egui::Stroke::new(1.5, egui::Color32::WHITE));
    } else {
        ui.painter().rect_stroke(
            check_rect,
            3.0,
            egui::Stroke::new(1.5, theme::RIO_BORDER),
            egui::epaint::StrokeKind::Inside,
        );
    }
    x += col_check + spacing;

    // 列2：歌单名（截断过长文本）
    let name_text = display_playlist_name(pl);
    let name_display = truncate_text(&name_text, col_name, &font_115, ui);
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        &name_display,
        font_115.clone(),
        theme::RIO_TEXT,
    );
    x += col_name + spacing;

    // 列3：大小
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        format_bytes(pl.size as u64),
        font_11.clone(),
        theme::RIO_TEXT_DIM,
    );
    x += col_size + spacing;

    // 列4：mem-badge（加宽，显示"内置"/"SD 卡"）
    let (badge_text, badge_bg, badge_fg) = if mem_unit == 0 {
        ("内置", theme::RIO_SELECTED_BG, theme::RIO_BLUE_PRESSED)
    } else {
        ("SD 卡", theme::RIO_S30S_ORANGE_SUBTLE, theme::RIO_S30S_ORANGE)
    };
    let badge_rect =
        egui::Rect::from_center_size(egui::pos2(x + 18.0, y), egui::vec2(44.0, 18.0));
    ui.painter().rect_filled(badge_rect, 3.0, badge_bg);
    ui.painter().text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        badge_text,
        egui::FontId::proportional(10.0),
        badge_fg,
    );

    (row_resp.clicked(), row_resp.double_clicked(), row_resp.secondary_clicked())
}

/// 截断过长文本（用 fonts 测量宽度，超出加省略号）
fn truncate_text(text: &str, max_w: f32, font: &egui::FontId, ui: &Ui) -> String {
    let measure = |s: &str| -> f32 {
        ui.painter()
            .fonts_mut(|f| f.layout_no_wrap(s.to_string(), font.clone(), egui::Color32::BLACK).size().x)
    };
    let full_w = measure(text);
    if full_w <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 1usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let candidate: String = chars[..mid].iter().collect();
        let w = measure(&format!("{}…", candidate));
        if w <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let truncated: String = chars[..lo].iter().collect();
    format!("{}…", truncated)
}

/// 新建歌单对话框
fn render_create_playlist_dialog(
    ctx: &Context,
    state: &mut AppState,
    cmd_tx: &Sender<Command>,
) {
    egui::Window::new("新建歌单")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("歌单名称:");
            ui.text_edit_singleline(&mut state.new_playlist_name);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("取消").clicked() {
                    state.show_create_playlist_dialog = false;
                    state.new_playlist_name.clear();
                }
                if ui
                    .add_enabled(
                        !state.new_playlist_name.trim().is_empty(),
                        egui::Button::new("创建"),
                    )
                    .clicked()
                {
                    let name = state.new_playlist_name.trim().to_string();
                    let _ = cmd_tx.try_send(Command::CreatePlaylist {
                        name,
                        mem_unit: state.mem_unit,
                    });
                    state.show_create_playlist_dialog = false;
                    state.new_playlist_name.clear();
                }
            });
        });
}

/// 过滤 + 排序歌单
fn filter_and_sort_playlists(
    playlists: &[SongEntry],
    query: &str,
    sort_by: u8,
) -> Vec<SongEntry> {
    let filtered: Vec<&SongEntry> = if query.trim().is_empty() {
        playlists.iter().collect()
    } else {
        let q = query.trim().to_lowercase();
        playlists
            .iter()
            .filter(|e| {
                e.file.title.to_lowercase().contains(&q)
                    || e.file.name.to_lowercase().contains(&q)
            })
            .collect()
    };
    let mut result: Vec<SongEntry> = filtered.iter().map(|e| (*e).clone()).collect();
    match sort_by {
        1 => result.sort_by(|a, b| b.file.size.cmp(&a.file.size)),
        _ => result.sort_by(|a, b| display_playlist_name(&a.file).cmp(&display_playlist_name(&b.file))),
    }
    result
}

/// 歌单名显示：title 为空时用 name 兜底
fn display_playlist_name(pl: &RioFile) -> String {
    if !pl.title.is_empty() {
        return pl.title.clone();
    }
    if pl.name.is_empty() {
        return "(无标题)".to_string();
    }
    let base = pl.name.rsplit(['\\', '/']).next().unwrap_or(&pl.name);
    let without_ext = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    if without_ext.is_empty() {
        "(无标题)".to_string()
    } else {
        without_ext.to_string()
    }
}

/// 歌曲标题显示
fn display_song_title(song: &cyrio_core::api::types::Song) -> String {
    if !song.title.is_empty() {
        return song.title.clone();
    }
    if song.name.is_empty() {
        return "(无标题)".to_string();
    }
    let base = song.name.rsplit(['\\', '/']).next().unwrap_or(&song.name);
    let without_ext = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    if without_ext.is_empty() {
        "(无标题)".to_string()
    } else {
        without_ext.to_string()
    }
}

/// 时长格式化
fn format_time(seconds: u32) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{}:{:02}", m, s)
}
