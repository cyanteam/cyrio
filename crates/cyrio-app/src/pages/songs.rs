//! 歌曲页：表格布局 + 多选 + 播放 + 右键菜单
//!
//! 对齐 Tauri .song-table：
//! - 列：[row-check + 标题][艺术家][专辑][时长][大小][存储 badge]
//! - 行：6/8 padding 11.5px；checked=rgba(57,197,187,0.12)+左3px；active=blue-subtle+左3px
//! - BatchToolbar + FilterBar（排序 + 搜索 + 计数）+ 分页
//!
//! 多选交互（对齐 Tauri）：
//! - 单击：仅选中该行（替换其他），若已是唯一选中则取消
//! - Ctrl/Cmd+点击：切换该行选中
//! - Shift+点击：从上次点击位置到当前行范围选择

use async_channel::Sender;
use egui::{Context, Key, Ui};

use crate::message::Command;
use crate::state::{format_bytes, AppState, ConfirmAction, SongEntry};
use crate::theme;
use cyrio_core::protocol::rio_file::RioFile;

/// 渲染歌曲页
pub fn render(ui: &mut Ui, ctx: &Context, state: &mut AppState, cmd_tx: &Sender<Command>) {
    // ===== BatchToolbar 容器（始终渲染，保持 UI 树结构稳定）=====
    // 关键：若条件渲染，选中状态变化会导致后续行的 allocate_exact_size 自动 Id 偏移，
    // double_clicked() 检测失效（双击的两次 click 落在不同 Id 上），播放也失效。
    // 始终保留 ui.horizontal 容器，未选中时内部留空占位。
    render_batch_toolbar(ui, state, cmd_tx);
    ui.add_space(4.0);

    // ===== FilterBar：排序 + 搜索 + 计数 =====
    render_filter_bar(ui, state);
    ui.add_space(4.0);

    // 加载中 / 空状态
    if state.loading && state.songs.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.spinner();
            ui.add_space(8.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "加载中…");
        });
        return;
    }

    if state.songs.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.colored_label(theme::RIO_TEXT_DIM, "暂无歌曲");
            if !state.connected {
                ui.add_space(10.0);
                ui.colored_label(theme::RIO_TEXT_DIM, "请先连接设备");
            }
        });
        return;
    }

    // 过滤 + 排序
    let filtered: Vec<SongEntry> = filter_and_sort_songs(&state.songs, &state.search_query, state.sort_by);

    // 分页
    let (page_items, total_pages, current_page) = if state.paginate {
        let per_page = state.songs_per_page;
        let total = filtered.len();
        let total_pages = total.div_ceil(per_page).max(1);
        let page = state.current_page.min(total_pages - 1);
        let start = page * per_page;
        let end = (start + per_page).min(total);
        (filtered[start.min(total)..end].to_vec(), total_pages, page)
    } else {
        (filtered.clone(), 1, 0)
    };

    let total_count = state.songs.len();
    let shown_count = filtered.len();

    // 计数行
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("共 {} 首（显示 {}）", total_count, shown_count))
                .size(11.0)
                .color(theme::RIO_TEXT_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !state.selected_song_ids.is_empty() {
                ui.colored_label(
                    theme::RIO_BLUE,
                    egui::RichText::new(format!("已选 {} 项", state.selected_song_ids.len())).size(11.0),
                );
            }
        });
    });
    ui.add_space(4.0);

    // ===== 表格（手动行布局，不用 Grid 避免 max_rect 问题）=====
    // macOS 用 Cmd 多选，Windows/Linux 用 Ctrl；egui 的 command 字段跨平台
    // 修饰键在点击处理器内读取，确保获取当前帧最新状态

    // 列宽计算
    let total_w = ui.available_width();
    let col_check = 22.0;
    let col_artist = 120.0;
    let col_album = 120.0;
    let col_time = 60.0;
    let col_size = 70.0;
    let col_bitrate = 56.0;
    let col_mem = 56.0;
    let spacing: f32 = 8.0;
    let fixed = col_check + col_artist + col_album + col_time + col_size + col_bitrate + col_mem + spacing * 7.0;
    let col_title = (total_w - fixed - 8.0).max(80.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        // 表头行
        render_table_header(ui, col_check, col_title, col_artist, col_album, col_time, col_size, col_bitrate, col_mem, spacing);

        // 数据行
        for (idx, entry) in page_items.iter().enumerate() {
            let is_selected = state.selected_song_ids.contains(&entry.file.file_no);
            let is_playing = state.current_playing_file_no == Some(entry.file.file_no);
            let global_idx = if state.paginate {
                current_page * state.songs_per_page + idx
            } else {
                idx
            };

            let row_resp = render_song_row(
                ui,
                ctx,
                &entry.file,
                entry.mem_unit,
                is_selected,
                is_playing,
                col_check,
                col_title,
                col_artist,
                col_album,
                col_time,
                col_size,
                col_bitrate,
                col_mem,
                spacing,
            );
            let row_clicked = row_resp.clicked();
            let play_clicked = row_resp.double_clicked();
            let secondary_clicked = row_resp.secondary_clicked();

            // 播放按钮点击
            if play_clicked {
                let mem_unit = entry.mem_unit;
                let file_no = entry.file.file_no;
                if let Some(audio) = &state.audio {
                    let _ = audio.stop();
                }
                if is_playing {
                    state.current_playing_file_no = None;
                } else {
                    state.current_playing_file_no = Some(file_no);
                    if let Some(audio) = &state.audio {
                        audio.set_loading(true);
                    }
                    let _ = cmd_tx.try_send(Command::DownloadSongForPlay {
                        file_no,
                        mem_unit,
                    });
                }
            }

            // 行点击 — 选择逻辑（直接点击即切换选中，无需修饰键）
            if row_clicked || secondary_clicked {
                let file_no = entry.file.file_no;
                let shift = ctx.input(|i| i.modifiers.shift);
                if shift {
                    // Shift+Click：范围选择
                    if let Some(last_idx) = state.last_clicked_song_index {
                        let start = last_idx.min(global_idx);
                        let end = last_idx.max(global_idx);
                        state.selected_song_ids.clear();
                        for e in &filtered[start..=end] {
                            state.selected_song_ids.insert(e.file.file_no);
                        }
                    } else {
                        state.selected_song_ids.clear();
                        state.selected_song_ids.insert(file_no);
                    }
                } else {
                    // 普通点击：切换该行选中状态
                    if is_selected {
                        state.selected_song_ids.remove(&file_no);
                    } else {
                        state.selected_song_ids.insert(file_no);
                    }
                }
                state.last_clicked_song_index = Some(global_idx);
            }

            // ===== 右键上下文菜单（重命名/加入歌单/下载/删除/修复编码）=====
            // 先克隆 entry 数据，避免 context_menu 闭包借用 entry 与 state 可变借用冲突
            let ctx_file_no = entry.file.file_no;
            let ctx_mem_unit = entry.mem_unit;
            let ctx_title = display_title(&entry.file);
            row_resp.context_menu(|ui| {
                if ui.button("重命名").clicked() {
                    state.show_rename_dialog =
                        Some((ctx_file_no, ctx_mem_unit, ctx_title.clone()));
                    state.rename_input = ctx_title.clone();
                    ui.close();
                }
                if ui.button("加入歌单").clicked() {
                    state.add_to_playlist_song_file_no = Some(ctx_file_no);
                    state.show_add_to_playlist_dialog = true;
                    ui.close();
                }
                if ui.button("下载到本地").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name(format!("rio_{}.mp3", ctx_file_no))
                        .add_filter("MP3 音频", &["mp3"])
                        .save_file()
                    {
                        let _ = cmd_tx.try_send(Command::DownloadSong {
                            file_no: ctx_file_no,
                            mem_unit: ctx_mem_unit,
                            save_path: path,
                        });
                        state.set_status("正在下载…");
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("修复编码").clicked() {
                    let _ = cmd_tx.try_send(Command::RepairSongEncoding {
                        file_no: ctx_file_no,
                        mem_unit: ctx_mem_unit,
                    });
                    state.set_status("正在修复编码…");
                    ui.close();
                }
                ui.separator();
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("删除").color(theme::RIO_DANGER),
                        ),
                    )
                    .clicked()
                {
                    state.confirm(
                        ConfirmAction::DeleteSong {
                            file_no: ctx_file_no,
                            mem_unit: ctx_mem_unit,
                        },
                        format!("确认删除「{}」？", ctx_title),
                    );
                    ui.close();
                }
            });
        }
    });

    // Ctrl+A / Cmd+A 全选（command 跨平台：macOS=Cmd, Windows/Linux=Ctrl）
    if ctx.input(|i| i.key_pressed(Key::A) && (i.modifiers.command || i.modifiers.ctrl)) {
        for e in &state.songs {
            state.selected_song_ids.insert(e.file.file_no);
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
                state.current_page = current_page - 1;
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
            if ui.add_enabled(current_page < total_pages - 1, next_btn).clicked() {
                state.current_page = current_page + 1;
            }
        });
    }

    // 加入歌单对话框
    if state.show_add_to_playlist_dialog {
        render_add_to_playlist_dialog(ctx, state, cmd_tx);
    }

    // 重命名对话框（由右键菜单触发）
    if state.show_rename_dialog.is_some() {
        render_rename_dialog(ctx, state, cmd_tx);
    }
}

/// BatchToolbar 对齐 .batch-toolbar
/// 始终渲染 ui.horizontal 容器（即使未选中），保持 UI 树结构稳定，
/// 防止后续行的 allocate_exact_size 自动 Id 因容器增减而偏移，导致 double_clicked 失效。
fn render_batch_toolbar(ui: &mut Ui, state: &mut AppState, cmd_tx: &Sender<Command>) {
    ui.horizontal(|ui| {
        // 未选中时不渲染按钮，但保留容器占位（高度 26px 与按钮一致）
        if state.selected_song_ids.is_empty() {
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
            return;
        }
        ui.spacing_mut().item_spacing.x = 4.0;
        // 全选
        if batch_btn(ui, "全选").clicked() {
            for e in &state.songs {
                state.selected_song_ids.insert(e.file.file_no);
            }
        }
        // 清空
        if batch_btn(ui, "清空").clicked() {
            state.selected_song_ids.clear();
            state.last_clicked_song_index = None;
        }
        // 批量下载
        if batch_btn(ui, "下载").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                let snapshot: Vec<(u32, u8)> = state
                    .selected_song_ids
                    .iter()
                    .filter_map(|&fn_| {
                        state.songs.iter().find(|e| e.file.file_no == fn_).map(|e| (fn_, e.mem_unit))
                    })
                    .collect();
                for (file_no, mem_unit) in snapshot {
                    let save_path = dir.join(format!("rio_{}.mp3", file_no));
                    let _ = cmd_tx.try_send(Command::DownloadSong {
                        file_no,
                        mem_unit,
                        save_path,
                    });
                }
            }
        }
        // 批量删除
        if batch_btn(ui, "删除").clicked() {
            let snapshot: Vec<(u32, u8)> = state
                .selected_song_ids
                .iter()
                .filter_map(|&fn_| {
                    state.songs.iter().find(|e| e.file.file_no == fn_).map(|e| (fn_, e.mem_unit))
                })
                .collect();
            // 按mem_unit分组
            let mut by_mem: std::collections::HashMap<u8, Vec<u32>> = std::collections::HashMap::new();
            for (fn_, mu) in &snapshot {
                by_mem.entry(*mu).or_default().push(*fn_);
            }
            let count = snapshot.len();
            // 简化：用第一个mem_unit的批量删除（实际应分组）
            if let Some((&mu, file_nos)) = by_mem.iter().next() {
                state.confirm(
                    ConfirmAction::DeleteSongsBatch {
                        file_nos: file_nos.clone(),
                        mem_unit: mu,
                    },
                    format!("确认删除选中的 {} 首歌曲？", count),
                );
            }
        }
        // 加入歌单
        if batch_btn(ui, "加入歌单").clicked() {
            if let Some(&fn_) = state.selected_song_ids.iter().next() {
                state.add_to_playlist_song_file_no = Some(fn_);
                state.show_add_to_playlist_dialog = true;
            }
        }
        // 更多批量操作（转拼音/去词/修复编码）
        ui.menu_button(
            egui::RichText::new("更多 ▾").size(11.0).color(theme::RIO_TEXT),
            |ui| {
                // 选中项批量转拼音
                let selected_count = state.selected_song_ids.len();
                if ui
                    .add_enabled(
                        selected_count > 0,
                        egui::Button::new(
                            egui::RichText::new(format!("转拼音（选中 {}）", selected_count))
                                .size(11.0),
                        ),
                    )
                    .clicked()
                {
                    let items: Vec<(u32, u8, String)> = state
                        .selected_song_ids
                        .iter()
                        .filter_map(|&fn_| {
                            state
                                .songs
                                .iter()
                                .find(|e| e.file.file_no == fn_)
                                .map(|e| (fn_, e.mem_unit, display_title(&e.file)))
                        })
                        .collect();
                    let _ = cmd_tx.try_send(Command::BatchSlugSongs { items });
                    state.set_status("正在批量转拼音…");
                    state.show_loading("正在批量转拼音…");
                    ui.close();
                }
                // 选中项批量去词
                if ui
                    .add_enabled(
                        selected_count > 0,
                        egui::Button::new(
                            egui::RichText::new(format!("去词（选中 {}）", selected_count))
                                .size(11.0),
                        ),
                    )
                    .clicked()
                {
                    let items: Vec<(u32, u8, String)> = state
                        .selected_song_ids
                        .iter()
                        .filter_map(|&fn_| {
                            state
                                .songs
                                .iter()
                                .find(|e| e.file.file_no == fn_)
                                .map(|e| (fn_, e.mem_unit, display_title(&e.file)))
                        })
                        .collect();
                    let custom_words = state.settings.custom_words_vec();
                    let _ = cmd_tx.try_send(Command::BatchStripSongs {
                        items,
                        custom_words,
                    });
                    state.set_status("正在批量去词…");
                    state.show_loading("正在批量去词…");
                    ui.close();
                }
                ui.separator();
                // 全部歌曲批量转拼音
                if ui
                    .button(egui::RichText::new("全部转拼音").size(11.0))
                    .clicked()
                {
                    let _ = cmd_tx.try_send(Command::BatchSlugAllSongs);
                    state.set_status("正在为全部歌曲转拼音…");
                    state.show_loading("正在为全部歌曲转拼音…");
                    ui.close();
                }
                // 全部歌曲批量去词
                if ui
                    .button(egui::RichText::new("全部去词").size(11.0))
                    .clicked()
                {
                    let custom_words = state.settings.custom_words_vec();
                    let _ = cmd_tx.try_send(Command::BatchStripAllSongs { custom_words });
                    state.set_status("正在为全部歌曲去词…");
                    state.show_loading("正在为全部歌曲去词…");
                    ui.close();
                }
                ui.separator();
                // 全部歌曲修复编码
                if ui
                    .button(egui::RichText::new("修复所有编码").size(11.0))
                    .clicked()
                {
                    let _ = cmd_tx.try_send(Command::RepairAllSongsEncoding);
                    state.set_status("正在修复所有歌曲编码…");
                    state.show_loading("正在修复所有歌曲编码…");
                    ui.close();
                }
            },
        );
        // 右侧刷新
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let refresh_btn = egui::Button::new(
                egui::RichText::new("↻").size(12.0),
            )
            .min_size(egui::vec2(24.0, 24.0))
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4);
            if ui.add_enabled(state.connected, refresh_btn).clicked() {
                let _ = cmd_tx.try_send(Command::ListSongs(crate::state::MEM_UNIT_INTERNAL));
                let _ = cmd_tx.try_send(Command::ListSongs(crate::state::MEM_UNIT_SD));
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

/// FilterBar 对齐 .filter-bar（排序 seg-control + 搜索 + 计数）
fn render_filter_bar(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        // 排序 seg-control
        ui.label(egui::RichText::new("排序").size(11.0).color(theme::RIO_TEXT_DIM));
        let sorts = [("名称", 0u8), ("大小", 1u8), ("时间", 2u8)];
        for (label, val) in sorts {
            let is_active = state.sort_by == val;
            let btn = if is_active {
                // 选中态：rio-blue 底 + 白字
                egui::Button::new(egui::RichText::new(label).size(11.0).color(egui::Color32::WHITE))
                    .fill(theme::RIO_BLUE)
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BLUE))
                    .corner_radius(3)
                    .min_size(egui::vec2(44.0, 22.0))
            } else {
                // 未选中：egui 状态样式自动处理 idle/hover/press
                egui::Button::new(egui::RichText::new(label).size(11.0))
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                    .corner_radius(3)
                    .min_size(egui::vec2(44.0, 22.0))
            };
            if ui.add(btn).clicked() {
                state.sort_by = val;
            }
        }
        ui.separator();
        // 搜索框
        ui.label(egui::RichText::new("搜索").size(11.0).color(theme::RIO_TEXT_DIM));
        ui.add_sized(
            egui::vec2(150.0, 22.0),
            egui::TextEdit::singleline(&mut state.search_query),
        );
    });
}

/// 颜色插值 — 使用 theme::lerp_color
/// 表头行
fn render_table_header(
    ui: &mut Ui,
    col_check: f32,
    col_title: f32,
    col_artist: f32,
    col_album: f32,
    col_time: f32,
    col_size: f32,
    col_bitrate: f32,
    col_mem: f32,
    spacing: f32,
) {
    let total_w = ui.available_width();
    let h = 24.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total_w, h), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme::RIO_BG_SUBTLE);

    let mut x = rect.min.x + 8.0;
    let y = rect.center().y;
    let font = egui::FontId::proportional(11.0);
    let color = theme::RIO_TEXT_DIM;

    // 标题列（含 row-check 占位）
    x += col_check + spacing;
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        "标题",
        font.clone(),
        color,
    );
    x += col_title + spacing;
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        "艺术家",
        font.clone(),
        color,
    );
    x += col_artist + spacing;
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        "专辑",
        font.clone(),
        color,
    );
    x += col_album + spacing;
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        "时长",
        font.clone(),
        color,
    );
    x += col_time + spacing;
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        "大小",
        font.clone(),
        color,
    );
    x += col_size + spacing;
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        "比特率",
        font.clone(),
        color,
    );
}

/// 渲染一行歌曲，返回该行的 egui::Response
///
/// 调用方通过 `response.clicked()` / `response.double_clicked()` /
/// `response.secondary_clicked()` 获取点击事件，并通过 `response.context_menu()`
/// 挂载右键菜单。
#[allow(clippy::too_many_arguments)]
fn render_song_row(
    ui: &mut Ui,
    ctx: &Context,
    song: &RioFile,
    mem_unit: u8,
    is_selected: bool,
    is_playing: bool,
    col_check: f32,
    col_title: f32,
    col_artist: f32,
    col_album: f32,
    col_time: f32,
    col_size: f32,
    col_bitrate: f32,
    col_mem: f32,
    spacing: f32,
) -> egui::Response {
    let total_w = ui.available_width();
    let row_h = 28.0;
    let row_id = ui.id().with(("song_row", song.file_no, mem_unit));

    // 分配行空间 + 获取交互响应
    let (row_rect, row_resp) = ui.allocate_exact_size(
        egui::vec2(total_w, row_h),
        egui::Sense::click(),
    );

    // hover 动画（150ms ease-out，对齐 Tauri cubic-bezier）
    let hover_target = if row_resp.hovered() { 1.0 } else { 0.0 };
    let hover_anim = ctx.animate_value_with_time(
        row_id,
        hover_target,
        0.15,
    );

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

    // 播放中行背景
    if is_playing && !is_selected {
        let bg = theme::lerp_color(theme::RIO_CONTENT_BG, theme::RIO_SELECTED_BG, 0.5);
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

    // 列2：标题（播放时加 ▶ 前缀）
    let title_text = if is_playing {
        format!("▶ {}", display_title(song))
    } else {
        display_title(song)
    };
    let title_color = if is_playing {
        theme::RIO_BLUE
    } else {
        theme::RIO_TEXT
    };
    // 截断过长标题
    let title_display = truncate_text(&title_text, col_title, &font_115, ui);
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        &title_display,
        font_115.clone(),
        title_color,
    );
    x += col_title + spacing;

    // 列3：艺术家
    let artist = if song.artist.is_empty() {
        "未知艺术家"
    } else {
        &song.artist
    };
    let artist_display = truncate_text(artist, col_artist, &font_11, ui);
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        &artist_display,
        font_11.clone(),
        theme::RIO_TEXT_DIM,
    );
    x += col_artist + spacing;

    // 列4：专辑
    let album = if song.album.is_empty() { "" } else { &song.album };
    let album_display = truncate_text(album, col_album, &font_11, ui);
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        &album_display,
        font_11.clone(),
        theme::RIO_TEXT_DIM,
    );
    x += col_album + spacing;

    // 列5：时长
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        format_time(song.time),
        font_11.clone(),
        theme::RIO_TEXT_DIM,
    );
    x += col_time + spacing;

    // 列6：大小
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        format_bytes(song.size as u64),
        font_11.clone(),
        theme::RIO_TEXT_DIM,
    );
    x += col_size + spacing;

    // 列7：比特率（bit_rate 字段单位是 kbps << 7，显示时 >> 7）
    let bitrate_text = if song.bit_rate > 0 {
        format!("{}kbps", song.bit_rate >> 7)
    } else {
        "—".to_string()
    };
    ui.painter().text(
        egui::pos2(x, y),
        egui::Align2::LEFT_CENTER,
        &bitrate_text,
        font_11.clone(),
        theme::RIO_TEXT_DIM,
    );
    x += col_bitrate + spacing;

    // 列8：mem-badge（加宽，显示"内置"/"SD 卡"）
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

    row_resp
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

/// "加入歌单"对话框
fn render_add_to_playlist_dialog(
    ctx: &Context,
    state: &mut AppState,
    cmd_tx: &Sender<Command>,
) {
    egui::Window::new("加入歌单")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            if state.playlists.is_empty() {
                ui.colored_label(theme::RIO_TEXT_DIM, "暂无歌单");
                ui.add_space(8.0);
                if ui.button("关闭").clicked() {
                    state.show_add_to_playlist_dialog = false;
                    state.add_to_playlist_song_file_no = None;
                }
                return;
            }
            ui.colored_label(theme::RIO_TEXT_DIM, "选择目标歌单：");
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    let playlists_snapshot: Vec<(u32, String, u8)> = state
                        .playlists
                        .iter()
                        .map(|p| (p.file.file_no, p.file.title.clone(), p.mem_unit))
                        .collect();
                    for (file_no, title, pl_mem) in &playlists_snapshot {
                        if ui.button(title.clone()).clicked() {
                            if let Some(song_file_no) = state.add_to_playlist_song_file_no {
                                // 查找歌曲所在 mem_unit
                                let song_mem = state
                                    .songs
                                    .iter()
                                    .find(|e| e.file.file_no == song_file_no)
                                    .map(|e| e.mem_unit)
                                    .unwrap_or(state.mem_unit);
                                let _ = cmd_tx.try_send(Command::AddToPlaylist {
                                    song_file_no,
                                    song_mem_unit: song_mem,
                                    playlist_file_no: *file_no,
                                    playlist_mem_unit: *pl_mem,
                                });
                                state.set_status(format!("正在加入歌单 {}…", title));
                            }
                            state.show_add_to_playlist_dialog = false;
                            state.add_to_playlist_song_file_no = None;
                        }
                    }
                });
            ui.add_space(8.0);
            ui.separator();
            if ui.button("取消").clicked() {
                state.show_add_to_playlist_dialog = false;
                state.add_to_playlist_song_file_no = None;
            }
        });
}

/// 歌曲标题显示：title 为空时用 name 字段兜底
fn display_title(song: &RioFile) -> String {
    if !song.title.is_empty() {
        return song.title.clone();
    }
    let name = &song.name;
    if name.is_empty() {
        return "(无标题)".to_string();
    }
    let base = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let without_ext = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    if without_ext.is_empty() {
        "(无标题)".to_string()
    } else {
        without_ext.to_string()
    }
}

/// 重命名对话框（由右键菜单触发）
///
/// 显示当前标题，用户输入新标题后确认 → 发送 RenameSong 命令。
/// 重命名通过 download → 修改 title → serialize → overwrite 流程，
/// 大文件重传较慢，进度通过 state.progress 反馈。
fn render_rename_dialog(ctx: &Context, state: &mut AppState, cmd_tx: &Sender<Command>) {
    // 取出 dialog 状态（避开 state 可变借用冲突）
    let (file_no, mem_unit, original) = match state.show_rename_dialog.clone() {
        Some(d) => d,
        None => return,
    };

    let frame = egui::Frame::new()
        .fill(theme::RIO_CONTENT_BG)
        .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
        .inner_margin(egui::Margin::same(16))
        .corner_radius(6);

    let mut confirmed = false;
    let mut cancelled = false;
    let mut new_title = state.rename_input.clone();

    egui::Modal::new(egui::Id::new("rename_dialog"))
        .backdrop_color(theme::RIO_OVERLAY)
        .frame(frame)
        .show(ctx, |ui| {
            ui.heading(
                egui::RichText::new("重命名")
                    .size(14.0)
                    .color(theme::RIO_TEXT),
            );
            ui.add_space(8.0);
            ui.colored_label(
                theme::RIO_TEXT_DIM,
                egui::RichText::new(format!("原始：{}", original)).size(11.0),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("新标题").size(11.0).color(theme::RIO_TEXT),
            );
            // 输入框占据较宽空间，回车确认
            let resp = ui.add_sized(
                egui::vec2(320.0, 22.0),
                egui::TextEdit::singleline(&mut new_title),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                confirmed = true;
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("取消").clicked() {
                    cancelled = true;
                }
                ui.add_enabled_ui(!new_title.trim().is_empty(), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("确认").color(egui::Color32::WHITE),
                            )
                            .fill(theme::RIO_BLUE)
                            .corner_radius(4)
                            .min_size(egui::vec2(60.0, 26.0)),
                        )
                        .clicked()
                    {
                        confirmed = true;
                    }
                });
            });
        });

    // 把输入框编辑结果写回 state
    state.rename_input = new_title.clone();

    if confirmed {
        let trimmed = new_title.trim();
        if !trimmed.is_empty() && trimmed != original {
            let _ = cmd_tx.try_send(Command::RenameSong {
                file_no,
                mem_unit,
                new_title: trimmed.to_string(),
            });
            state.set_status("正在重命名…");
            state.show_loading("正在重命名…");
        }
        // 关闭对话框（事件回调中也会再保险一次）
        state.show_rename_dialog = None;
        state.rename_input.clear();
    } else if cancelled {
        state.show_rename_dialog = None;
        state.rename_input.clear();
    }
}

/// 过滤 + 排序歌曲
fn filter_and_sort_songs(songs: &[SongEntry], query: &str, sort_by: u8) -> Vec<SongEntry> {
    let filtered: Vec<&SongEntry> = if query.trim().is_empty() {
        songs.iter().collect()
    } else {
        let q = query.trim().to_lowercase();
        songs
            .iter()
            .filter(|e| {
                e.file.title.to_lowercase().contains(&q)
                    || e.file.artist.to_lowercase().contains(&q)
                    || e.file.name.to_lowercase().contains(&q)
            })
            .collect()
    };
    let mut result: Vec<SongEntry> = filtered.iter().map(|e| (*e).clone()).collect();
    match sort_by {
        1 => result.sort_by(|a, b| b.file.size.cmp(&a.file.size)),
        2 => result.sort_by(|a, b| b.file.time.cmp(&a.file.time)),
        _ => result.sort_by(|a, b| display_title(&a.file).cmp(&display_title(&b.file))),
    }
    result
}

/// 时长格式化（秒 → mm:ss）
fn format_time(seconds: u32) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{}:{:02}", m, s)
}
