//! 歌曲列表页面 — 1:1 复刻 Tauri .songs-page
//!
//! Tauri CSS 参考：
//! - .batch-toolbar: gap 4px, padding 4px 0
//! - .batch-btn: padding 4px 10px, radius 4px, bg-subtle, 13px/500
//! - .filter-bar: gap 10px, padding 4px 0
//! - .seg-control: bg-subtle, padding 2px, gap 1px
//! - .song-table-wrap: flex 1, overflow auto, border 1px, radius 4px
//! - .song-table: width 100%, font 13px
//! - th: padding 6px 8px, 600, text-dim, 13px, uppercase
//! - td: padding 6px 8px, border-bottom 1px border-light
//! - tr.active: bg rio-blue-subtle, border-left 3px rio-blue
//! - tr.checked: bg rgba(57,197,187,0.12), border-left 3px rio-blue-light
//! - .col-title: min 180px (含 row-check + 标题)
//! - .col-artist: min 100px max 220px, text-dim, ellipsis
//! - .col-album: 同上
//! - .col-time: 64px, right, text-dim, tabular-nums
//! - .col-size: 78px, right, text-dim, tabular-nums
//! - .col-bitrate: 78px, right, text-dim, "320kbps"
//! - .col-mem: 78px, center, mem-badge
//! - .row-check: 14×14, border 1.5px, radius 3px, margin-right 8px
//! - .mem-badge: 12px/600, padding 2px 8px, min-width 52px
//!
//! 交互（匹配 Tauri React）：
//! - 单击 = 切换选中 + 设为 active 行
//! - Shift+单击 = 范围选中
//! - 双击 = 播放
//! - 右键 = 上下文菜单

use crate::state::{display_title, format_bytes, format_time, CyrioApp, SortField};
use crate::task::Command;
use crate::theme::Theme;
use gpui::*;
use gpui::prelude::*;

// ---- 列宽常量（匹配 Tauri CSS）----
const COL_TIME_W: f32 = 64.0;
const COL_SIZE_W: f32 = 78.0;
const COL_BITRATE_W: f32 = 78.0;
const COL_MEM_W: f32 = 78.0;
const COL_ARTIST_MIN: f32 = 100.0;
const COL_ARTIST_MAX: f32 = 220.0;
const COL_TITLE_MIN: f32 = 180.0;

pub fn render_songs_view(app: &mut CyrioApp, cx: &mut Context<CyrioApp>) -> impl IntoElement {
    let filtered = app.filtered_songs();
    let total = filtered.len();
    let paged = if app.paginate { app.paged_songs() } else { filtered.clone() };
    let total_pages = app.total_pages();
    let current_page = app.current_page_num;
    let selected_count = app.selected_songs.len();
    let sort_field = app.sort_field;
    let paginate = app.paginate;
    let search_query = app.search_query.clone();
    let loading = app.loading;
    let selected_songs = app.selected_songs.clone();
    let current_playing = app.current_playing_file_no;
    let active_idx = app.active_idx;
    let show_more_menu = app.show_more_menu;
    let context_menu = app.context_menu.clone();

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap(px(0.0))
        // ---- 批量工具栏 ----
        .child(render_batch_toolbar(app, cx, selected_count, total, show_more_menu))
        // ---- 筛选栏 ----
        .child(render_filter_bar(app, cx, sort_field, search_query, total))
        // ---- 歌曲表格 ----
        .child(
            if loading {
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Theme::TEXT_DIM)
                    .text_size(px(Theme::FONT_14))
                    .child("加载中...")
                    .into_any_element()
            } else if paged.is_empty() {
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Theme::TEXT_DIM)
                    .text_size(px(Theme::FONT_14))
                    .child(if total == 0 { "暂无歌曲" } else { "无匹配结果" })
                    .into_any_element()
            } else {
                render_songs_table(
                    cx,
                    &paged,
                    &selected_songs,
                    current_playing,
                    active_idx,
                ).into_any_element()
            }
        )
        // ---- 分页 ----
        .when(paginate && total_pages > 1, |this| {
            this.child(render_pagination(cx, current_page, total_pages, total))
        })
        // ---- 右键菜单 ----
        .when_some(context_menu, |this, menu| {
            this.child(render_context_menu(app, cx, &menu))
        })
}

// ============================================================================
// 批量工具栏 — .batch-toolbar
// ============================================================================

fn render_batch_toolbar(
    app: &mut CyrioApp,
    cx: &mut Context<CyrioApp>,
    selected_count: usize,
    total: usize,
    show_more_menu: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .py(px(4.0))
        .flex_shrink_0()
        // 全选
        .child(render_batch_btn("全选", false, false, cx.listener(|this, _: &ClickEvent, _window, cx| {
            if this.selected_songs.len() == this.songs.len() {
                this.selected_songs.clear();
            } else {
                this.selected_songs = this.songs.iter().map(|e| e.file.file_no).collect();
            }
            cx.notify();
        })))
        // 清空
        .child(render_batch_btn("清空", false, selected_count == 0, cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.selected_songs.clear();
            cx.notify();
        })))
        // 删除
        .child(render_batch_btn(
            &format!("删除{}", if selected_count > 0 { format!(" ({})", selected_count) } else { String::new() }),
            true,
            selected_count == 0,
            cx.listener(move |this, _: &ClickEvent, _window, cx| {
                for &file_no in this.selected_songs.iter() {
                    if let Some(entry) = this.songs.iter().find(|e| e.file.file_no == file_no) {
                        this.send_cmd(Command::DeleteSong {
                            file_no,
                            mem_unit: entry.mem_unit,
                        });
                    }
                }
                this.selected_songs.clear();
                cx.notify();
            }),
        ))
        // 加入歌单
        .child(render_batch_btn(
            &format!("加入歌单{}", if selected_count > 0 { format!(" ({})", selected_count) } else { String::new() }),
            false,
            selected_count == 0,
            cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.show_playlist_picker = true;
                cx.notify();
            }),
        ))
        // 更多（下拉菜单）
        .child(render_more_dropdown(app, cx, selected_count, show_more_menu))
        // 刷新 — margin-left: auto
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(28.0))
                .px(px(10.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_SUBTLE)
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(Theme::FONT_13))
                .ml_auto()
                .child("刷新")
                .id("btn-refresh")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.loading = true;
                    this.pending_song_loads = 2;
                    this.send_cmd(Command::ListSongs(0));
                    this.send_cmd(Command::ListSongs(1));
                    cx.notify();
                }))
        )
}

/// .batch-btn — padding 4px 10px, radius 4px, bg-subtle, 13px/500
fn render_batch_btn(
    label: &str,
    danger: bool,
    disabled: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.to_string();
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(Theme::RADIUS_SM))
        .bg(Theme::BG_SUBTLE)
        .text_color(if disabled { Theme::TEXT_DIM } else if danger { Theme::ERROR } else { Theme::TEXT_SECONDARY })
        .text_size(px(Theme::FONT_13))
        .when(disabled, |this| this.opacity(0.4))
        .hover(move |this| if !disabled && !danger {
            this.bg(Theme::RIO_BLUE_SUBTLE).text_color(Theme::RIO_BLUE)
        } else if !disabled && danger {
            this.bg(Theme::ACCENT_SOFT).text_color(Theme::ERROR)
        } else {
            this
        })
        .child(label)
        .id("batch-btn")
        .on_click(move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
            if !disabled {
                handler(event, window, cx);
            }
        })
}

/// 更多下拉菜单 — .batch-more-wrap + .batch-more-menu
fn render_more_dropdown(
    _app: &CyrioApp,
    cx: &mut Context<CyrioApp>,
    selected_count: usize,
    show_menu: bool,
) -> impl IntoElement {
    div()
        .relative()
        .flex()
        .items_center()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(28.0))
                .px(px(10.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_SUBTLE)
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(Theme::FONT_13))
                .child("更多")
                .id("btn-more")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.show_more_menu = !this.show_more_menu;
                    cx.notify();
                }))
        )
        .when(show_menu, |this| {
            this.child(render_more_menu(cx, selected_count))
        })
}

/// .batch-more-menu — 下拉菜单内容
fn render_more_menu(
    cx: &mut Context<CyrioApp>,
    selected_count: usize,
) -> impl IntoElement {
    div()
        .absolute()
        .top(px(32.0))
        .left(px(0.0))
        .w(px(180.0))
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded(px(Theme::RADIUS_SM))
        .p(px(6.0))
        .flex()
        .flex_col()
        .gap(px(1.0))
        // ---- 仅选中(N) ----
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_12))
                .font_weight(FontWeight::SEMIBOLD)
                .px(px(8.0))
                .py(px(4.0))
                .child(format!("仅选中（{}）", selected_count))
        )
        .child(render_more_item("转拼音", selected_count == 0, cx.listener(|this, _: &ClickEvent, _window, cx| {
            let items: Vec<(u32, u8, String)> = this.selected_songs.iter()
                .filter_map(|&fn_| this.songs.iter().find(|e| e.file.file_no == fn_))
                .map(|e| (e.file.file_no, e.mem_unit, display_title(&e.file)))
                .collect();
            this.send_cmd(Command::BatchSlugSongs { items });
            this.show_more_menu = false;
            cx.notify();
        })))
        .child(render_more_item("去词", selected_count == 0, cx.listener(|this, _: &ClickEvent, _window, cx| {
            let items: Vec<(u32, u8, String)> = this.selected_songs.iter()
                .filter_map(|&fn_| this.songs.iter().find(|e| e.file.file_no == fn_))
                .map(|e| (e.file.file_no, e.mem_unit, display_title(&e.file)))
                .collect();
            let words = this.settings.custom_words_vec();
            this.send_cmd(Command::BatchStripSongs { items, custom_words: words });
            this.show_more_menu = false;
            cx.notify();
        })))
        .child(render_more_item("修复编码", selected_count == 0, cx.listener(|this, _: &ClickEvent, _window, cx| {
            for &fn_ in this.selected_songs.iter() {
                if let Some(e) = this.songs.iter().find(|e| e.file.file_no == fn_) {
                    this.send_cmd(Command::RepairSongEncoding { file_no: fn_, mem_unit: e.mem_unit });
                }
            }
            this.show_more_menu = false;
            cx.notify();
        })))
        // 分隔线
        .child(div().h(px(1.0)).bg(Theme::BG_SUBTLE).my(px(3.0)))
        // ---- 全部歌曲 ----
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_12))
                .font_weight(FontWeight::SEMIBOLD)
                .px(px(8.0))
                .py(px(4.0))
                .child("全部歌曲")
        )
        .child(render_more_item("全部转拼音", false, cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.send_cmd(Command::BatchSlugAllSongs);
            this.show_more_menu = false;
            cx.notify();
        })))
        .child(render_more_item("全部去词", false, cx.listener(|this, _: &ClickEvent, _window, cx| {
            let words = this.settings.custom_words_vec();
            this.send_cmd(Command::BatchStripAllSongs { custom_words: words });
            this.show_more_menu = false;
            cx.notify();
        })))
        // 分隔线
        .child(div().h(px(1.0)).bg(Theme::BG_SUBTLE).my(px(3.0)))
        .child(render_more_item("修复所有编码", false, cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.send_cmd(Command::RepairAllSongsEncoding);
            this.show_more_menu = false;
            cx.notify();
        })))
}

/// .batch-more-item — padding 6px 10px, radius 3px, 13.5px
fn render_more_item(
    label: &str,
    disabled: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.to_string();
    div()
        .text_left()
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(Theme::RADIUS_XS))
        .text_color(if disabled { Theme::TEXT_DIM } else { Theme::TEXT })
        .text_size(px(13.5))
        .when(disabled, |this| this.opacity(0.4))
        .hover(move |this| if !disabled {
            this.bg(Theme::RIO_BLUE_SUBTLE).text_color(Theme::RIO_BLUE)
        } else {
            this
        })
        .child(label)
        .id("more-item")
        .on_click(move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
            if !disabled {
                handler(event, window, cx);
            }
        })
}

// ============================================================================
// 筛选栏 — .filter-bar
// ============================================================================

fn render_filter_bar(
    _app: &CyrioApp,
    cx: &mut Context<CyrioApp>,
    sort_field: SortField,
    search_query: String,
    shown: usize,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .py(px(4.0))
        .flex_shrink_0()
        // 排序组
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(5.0))
                .child(
                    div()
                        .text_color(Theme::TEXT_DIM)
                        .text_size(px(Theme::FONT_12))
                        .font_weight(FontWeight::MEDIUM)
                        .child("排序")
                )
                // seg-control
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .bg(Theme::BG_SUBTLE)
                        .rounded(px(Theme::RADIUS_SM))
                        .p(px(2.0))
                        .gap(px(1.0))
                        .child(render_seg_btn("名称", sort_field == SortField::Name, SortField::Name, cx))
                        .child(render_seg_btn("大小", sort_field == SortField::Size, SortField::Size, cx))
                        .child(render_seg_btn("时间", sort_field == SortField::Time, SortField::Time, cx))
                )
        )
        // 搜索框 — flex 1, max 220px
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .flex_1()
                .min_w(px(100.0))
                .max_w(px(220.0))
                .h(px(28.0))
                .px(px(8.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_SUBTLE)
                .text_color(if search_query.is_empty() { Theme::TEXT_DIM } else { Theme::TEXT })
                .text_size(px(Theme::FONT_13))
                .child(if search_query.is_empty() {
                    "搜索…".to_string()
                } else {
                    search_query
                })
                .id("filter-search")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.search_query.clear();
                    cx.notify();
                }))
        )
        // 计数 — margin-left auto
        .child(
            div()
                .ml_auto()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_12))
                .font_weight(FontWeight::MEDIUM)
                .child(format!("{} 首", shown))
        )
}

/// seg-control button — padding 3px 8px, radius 3px, 13px
fn render_seg_btn(
    label: &str,
    active: bool,
    field: SortField,
    cx: &mut Context<CyrioApp>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .px(px(8.0))
        .py(px(3.0))
        .rounded(px(Theme::RADIUS_XS))
        .when(active, |this| {
            this.bg(Theme::BG_ELEVATED).text_color(Theme::RIO_BLUE)
        })
        .when(!active, |this| {
            this.text_color(Theme::TEXT_SECONDARY)
        })
        .text_size(px(Theme::FONT_13))
        .font_weight(FontWeight::MEDIUM)
        .child(label.to_string())
        .id(format!("seg-{}", label))
        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.sort_field = field;
            cx.notify();
        }))
}

// ============================================================================
// 歌曲表格 — .song-table-wrap > .song-table
// ============================================================================

fn render_songs_table(
    cx: &mut Context<CyrioApp>,
    songs: &[crate::state::SongEntry],
    selected: &std::collections::HashSet<u32>,
    current_playing: Option<u32>,
    active_idx: Option<usize>,
) -> impl IntoElement {
    // .song-table-wrap: flex 1, overflow auto, border 1px, radius 4px, bg-elevated
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .border_1()
        .border_color(Theme::BORDER)
        .rounded(px(Theme::RADIUS_SM))
        .bg(Theme::BG_ELEVATED)
        .overflow_hidden()
        .mt(px(8.0))
        // 表头（sticky 效果用 flex_shrink_0 模拟）
        .child(render_table_header())
        // 表体（可滚动）
        .child(
            div()
                .id("songs-table-body")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(songs.iter().enumerate().map(|(i, entry)| {
                    let is_selected = selected.contains(&entry.file.file_no);
                    let is_playing = current_playing == Some(entry.file.file_no);
                    let is_active = active_idx == Some(i);
                    render_song_row(entry, i, is_selected, is_active, is_playing, cx)
                }))
        )
}

/// 表头 — th: padding 6px 8px, 600, text-dim, 13px, uppercase
/// 列宽必须与 render_song_row 完全一致
fn render_table_header() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(30.0))
        .flex_shrink_0()
        .bg(Theme::BG_SUBTLE)
        .border_b_1()
        .border_color(Theme::BORDER)
        // 左侧留 3px 对齐行指示条 + 8px 内边距
        .pl(px(11.0))
        .pr(px(8.0))
        .text_color(Theme::TEXT_DIM)
        .text_size(px(Theme::FONT_13))
        .font_weight(FontWeight::SEMIBOLD)
        // col-title — flex 1, min 180px
        .child(
            div()
                .flex_1()
                .min_w(px(COL_TITLE_MIN))
                .child("标题")
        )
        // col-artist — flex 1, min 100px, max 220px
        .child(
            div()
                .flex_1()
                .min_w(px(COL_ARTIST_MIN))
                .max_w(px(COL_ARTIST_MAX))
                .child("艺术家")
        )
        // col-album — same
        .child(
            div()
                .flex_1()
                .min_w(px(COL_ARTIST_MIN))
                .max_w(px(COL_ARTIST_MAX))
                .child("专辑")
        )
        // col-time — 64px fixed
        .child(
            div()
                .w(px(COL_TIME_W))
                .flex_shrink_0()
                .text_right()
                .child("时长")
        )
        // col-size — 78px fixed
        .child(
            div()
                .w(px(COL_SIZE_W))
                .flex_shrink_0()
                .text_right()
                .child("大小")
        )
        // col-bitrate — 78px fixed
        .child(
            div()
                .w(px(COL_BITRATE_W))
                .flex_shrink_0()
                .text_right()
                .child("比特率")
        )
        // col-mem — 78px fixed
        .child(
            div()
                .w(px(COL_MEM_W))
                .flex_shrink_0()
                .text_center()
                .child("存储")
        )
}

/// 歌曲行 — 匹配 Tauri tr.active/checked 样式
/// 关键：padding 与表头完全一致，指示条用 absolute 定位不影响布局
fn render_song_row(
    entry: &crate::state::SongEntry,
    index: usize,
    is_selected: bool,
    is_active: bool,
    is_playing: bool,
    cx: &mut Context<CyrioApp>,
) -> impl IntoElement {
    let file = &entry.file;
    let title = display_title(file);
    let artist = if file.artist.is_empty() { "—" } else { &file.artist };
    let album = if file.album.is_empty() { "—" } else { &file.album };
    let time = if file.time > 0 { format_time(file.time) } else { "—".to_string() };
    let size = format_bytes(file.size as u64);
    let bit_rate = if file.bit_rate > 0 { format!("{}kbps", file.bit_rate >> 7) } else { "—".to_string() };
    let mem_label = if entry.mem_unit == 0 { "内置" } else { "SD 卡" };
    let mem_bg = if entry.mem_unit == 0 { Theme::RIO_BLUE_SUBTLE } else { Theme::S30S_ORANGE_SUBTLE };
    let mem_color = if entry.mem_unit == 0 { Theme::RIO_BLUE_DARK } else { Theme::S30S_ORANGE };
    let file_no = file.file_no;
    let mem_unit = entry.mem_unit;

    // 行背景色
    let row_bg = if is_active && is_selected {
        Theme::ROW_ACTIVE_CHECKED
    } else if is_active {
        Theme::RIO_BLUE_SUBTLE
    } else if is_selected {
        Theme::ROW_CHECKED
    } else {
        Theme::BG_ELEVATED
    };

    // 指示条颜色（3px absolute 定位，不影响布局）
    let indicator_color = if is_active {
        Some(Theme::RIO_BLUE)
    } else if is_selected {
        Some(Theme::RIO_BLUE_LIGHT)
    } else {
        None
    };

    div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .min_h(px(34.0))
        // 左 11px = 3px 指示条空间 + 8px 内边距（与表头一致）
        .pl(px(11.0))
        .pr(px(8.0))
        .bg(row_bg)
        .border_b_1()
        .border_color(Theme::BORDER_LIGHT)
        .hover(|this| if !is_active && !is_selected {
            this.bg(Theme::ROW_HOVER)
        } else {
            this
        })
        // ---- 左侧指示条 3px（absolute，不影响列对齐）----
        .when_some(indicator_color, |this, color| {
            this.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .bottom(px(0.0))
                    .w(px(3.0))
                    .bg(color)
            )
        })
        // ---- col-title: row-check + 标题 ----
        .child(
            div()
                .flex_1()
                .min_w(px(COL_TITLE_MIN))
                .flex()
                .flex_row()
                .items_center()
                // row-check: 14×14, border 1.5px, radius 3px, margin-right 8px
                .child(
                    div()
                        .w(px(14.0))
                        .h(px(14.0))
                        .rounded(px(3.0))
                        .border(px(1.5))
                        .border_color(if is_selected { Theme::RIO_BLUE } else { Theme::BORDER })
                        .when(is_selected, |this| {
                            this.bg(Theme::RIO_BLUE)
                        })
                        .mr(px(8.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(is_selected, |this| {
                            this.text_color(Theme::WHITE).text_size(px(10.0)).child("✓")
                        })
                )
                // 标题文字
                .child(
                    div()
                        .min_w_0()
                        .text_color(if is_playing { Theme::RIO_BLUE } else { Theme::TEXT })
                        .text_size(px(Theme::FONT_13))
                        .when(is_playing, |this| {
                            this.font_weight(FontWeight::SEMIBOLD)
                        })
                        .child(title)
                )
        )
        // ---- col-artist — flex 1, min 100, max 220 ----
        .child(
            div()
                .flex_1()
                .min_w(px(COL_ARTIST_MIN))
                .max_w(px(COL_ARTIST_MAX))
                .min_w_0()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_13))
                .child(artist.to_string())
        )
        // ---- col-album — same ----
        .child(
            div()
                .flex_1()
                .min_w(px(COL_ARTIST_MIN))
                .max_w(px(COL_ARTIST_MAX))
                .min_w_0()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_13))
                .child(album.to_string())
        )
        // ---- col-time — 64px fixed ----
        .child(
            div()
                .w(px(COL_TIME_W))
                .flex_shrink_0()
                .text_right()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_13))
                .child(time)
        )
        // ---- col-size — 78px fixed ----
        .child(
            div()
                .w(px(COL_SIZE_W))
                .flex_shrink_0()
                .text_right()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_13))
                .child(size)
        )
        // ---- col-bitrate — 78px fixed ----
        .child(
            div()
                .w(px(COL_BITRATE_W))
                .flex_shrink_0()
                .text_right()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_13))
                .child(bit_rate)
        )
        // ---- col-mem: mem-badge — 78px fixed ----
        .child(
            div()
                .w(px(COL_MEM_W))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(Theme::FONT_12))
                        .font_weight(FontWeight::SEMIBOLD)
                        .py(px(2.0))
                        .px(px(8.0))
                        .min_w(px(52.0))
                        .text_center()
                        .rounded(px(Theme::RADIUS_XS))
                        .bg(mem_bg)
                        .text_color(mem_color)
                        .child(mem_label.to_string())
                )
        )
        // ---- 交互 ----
        .id(format!("song-row-{}", index))
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
            let click_count = event.click_count;
            let shift = event.modifiers.shift;

            if click_count >= 2 {
                this.current_playing_file_no = Some(file_no);
                this.send_cmd(Command::DownloadSongForPlay { file_no, mem_unit });
            } else if shift {
                if let Some(last) = this.last_clicked_idx {
                    let start = last.min(index);
                    let end = last.max(index);
                    let filtered = this.filtered_songs();
                    this.selected_songs.clear();
                    for entry in filtered.iter().skip(start).take(end - start + 1) {
                        this.selected_songs.insert(entry.file.file_no);
                    }
                } else {
                    if this.selected_songs.contains(&file_no) {
                        this.selected_songs.remove(&file_no);
                    } else {
                        this.selected_songs.insert(file_no);
                    }
                }
            } else {
                this.active_idx = Some(index);
                if this.selected_songs.contains(&file_no) {
                    this.selected_songs.remove(&file_no);
                } else {
                    this.selected_songs.insert(file_no);
                }
                this.last_clicked_idx = Some(index);
            }
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
            this.context_menu = Some(crate::state::ContextMenuState {
                x: f32::from(event.position.x),
                y: f32::from(event.position.y),
                file_no,
                mem_unit,
            });
            cx.notify();
        }))
}

// ============================================================================
// 分页 — .pagination
// ============================================================================

fn render_pagination(
    cx: &mut Context<CyrioApp>,
    current_page: usize,
    total_pages: usize,
    total: usize,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .py(px(4.0))
        .flex_shrink_0()
        // 上一页 — page-btn 22×22
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(22.0))
                .h(px(22.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_SUBTLE)
                .text_color(if current_page == 0 { Theme::TEXT_DIM } else { Theme::TEXT_SECONDARY })
                .text_size(px(Theme::FONT_14))
                .when(current_page == 0, |this| this.opacity(0.3))
                .child("‹")
                .id("page-prev")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    if this.current_page_num > 0 {
                        this.current_page_num -= 1;
                        cx.notify();
                    }
                }))
        )
        // 页码 — page-info 13px, min-width 44px
        .child(
            div()
                .text_color(Theme::TEXT_SECONDARY)
                .text_size(px(Theme::FONT_13))
                .font_weight(FontWeight::MEDIUM)
                .min_w(px(44.0))
                .text_center()
                .child(format!("{} / {}", current_page + 1, total_pages))
        )
        // 下一页
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(22.0))
                .h(px(22.0))
                .rounded(px(Theme::RADIUS_SM))
                .bg(Theme::BG_SUBTLE)
                .text_color(if current_page + 1 >= total_pages { Theme::TEXT_DIM } else { Theme::TEXT_SECONDARY })
                .text_size(px(Theme::FONT_14))
                .when(current_page + 1 >= total_pages, |this| this.opacity(0.3))
                .child("›")
                .id("page-next")
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    if this.current_page_num + 1 < total_pages {
                        this.current_page_num += 1;
                        cx.notify();
                    }
                }))
        )
        // 总数
        .child(
            div()
                .text_color(Theme::TEXT_DIM)
                .text_size(px(Theme::FONT_12))
                .child(format!("(共 {} 首)", total))
        )
}

// ============================================================================
// 右键菜单 — .context-menu
// ============================================================================

fn render_context_menu(
    app: &CyrioApp,
    cx: &mut Context<CyrioApp>,
    menu: &crate::state::ContextMenuState,
) -> impl IntoElement {
    let file_no = menu.file_no;
    let mem_unit = menu.mem_unit;
    let x = menu.x;
    let y = menu.y;

    // 找到当前歌曲信息
    let song_title = app.songs.iter()
        .find(|e| e.file.file_no == file_no && e.mem_unit == mem_unit)
        .map(|e| display_title(&e.file))
        .unwrap_or_default();
    let song_title_for_info = song_title.clone();

    div()
        .absolute()
        .top(px(y))
        .left(px(x))
        .min_w(px(160.0))
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BG_SUBTLE)
        .rounded(px(Theme::RADIUS_SM))
        .p(px(4.0))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .text_size(px(Theme::FONT_14))
        // 遮罩层 — 点击关闭菜单
        .child(
            div()
                .absolute()
                .top(px(-9999.0))
                .left(px(-9999.0))
                .w(px(99999.0))
                .h(px(99999.0))
                .id("ctx-menu-overlay")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.context_menu = None;
                    cx.notify();
                }))
        )
        // 播放试听
        .child(render_ctx_item("播放试听", false, cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.current_playing_file_no = Some(file_no);
            this.send_cmd(Command::DownloadSongForPlay { file_no, mem_unit });
            this.context_menu = None;
            cx.notify();
        })))
        // 加入歌单
        .child(render_ctx_item("加入歌单", false, cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.show_playlist_picker = true;
            this.picker_song_file_no = Some(file_no);
            this.context_menu = None;
            cx.notify();
        })))
        // 详细信息
        .child(render_ctx_item("详细信息", false, cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.show_notice(format!("{} — 详细信息", song_title_for_info), cx);
            this.context_menu = None;
        })))
        // 重命名
        .child(render_ctx_item("重命名", false, cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.rename_target = Some((file_no, mem_unit, song_title.clone()));
            this.context_menu = None;
            cx.notify();
        })))
        // 下载到本地
        .child(render_ctx_item("下载到本地", false, cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.send_cmd(Command::DownloadSong {
                file_no,
                mem_unit,
                save_path: std::path::PathBuf::from(format!("/tmp/cyrio_download_{}.mp3", file_no)),
            });
            this.context_menu = None;
            this.show_notice("开始下载…", cx);
        })))
        // 修复编码
        .child(render_ctx_item("修复编码", false, cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.send_cmd(Command::RepairSongEncoding { file_no, mem_unit });
            this.context_menu = None;
            this.show_notice("修复编码中…", cx);
        })))
        // 分隔线
        .child(div().h(px(1.0)).bg(Theme::BG_SUBTLE).my(px(3.0)))
        // 删除 — danger
        .child(render_ctx_item("删除", true, cx.listener(move |this, _: &ClickEvent, _window, cx| {
            this.send_cmd(Command::DeleteSong { file_no, mem_unit });
            this.context_menu = None;
            this.show_notice("删除中…", cx);
        })))
}

/// .context-menu button — padding 6px 10px, radius 3px, 14px
fn render_ctx_item(
    label: &str,
    danger: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(Theme::RADIUS_XS))
        .text_color(if danger { Theme::ERROR } else { Theme::TEXT })
        .text_size(px(Theme::FONT_14))
        .hover(move |this| if danger {
            this.bg(Theme::ACCENT_SOFT).text_color(Theme::ERROR)
        } else {
            this.bg(Theme::RIO_BLUE_SUBTLE).text_color(Theme::RIO_BLUE)
        })
        .child(label)
        .id("ctx-item")
        .on_click(move |event: &ClickEvent, window: &mut Window, cx: &mut App| {
            handler(event, window, cx);
        })
}
