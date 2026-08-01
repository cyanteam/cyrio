//! 主 App（实现 eframe::App）
//!
//! 布局方案（Phase 4 新设计）：
//! - 顶部栏：cyrio logo + 设备连接按钮 + 内存单元切换 + 存储状态条
//! - 选项卡栏：横排 [歌曲] [歌单] [上传] [设备信息]
//! - 内容区：根据当前 `page_path` 渲染对应页面
//! - Alt+Shift+D：弹出调试窗口（日志 + 路由重置 + 状态查看）

use async_channel::{Receiver, Sender};
use eframe::egui;
use egui::{Context, Panel, Ui};

use crate::fonts;
use crate::message::{Command, Event};
use crate::pages;
use crate::state::{format_bytes, AppState, ConfirmAction, ConfirmDialog, SongEntry, MEM_UNIT_INTERNAL, MEM_UNIT_SD};
use crate::task::spawn_task_loop;
use crate::theme;
use cyrio_webdav::WebDavStatus;

/// 菜单项定义：`(action, 显示名, 图标)` — 对齐 Tauri MENU_ITEMS
const MENU_ITEMS: &[(&str, &str, &str)] = &[
    ("songs", "歌曲", "♪"),
    ("playlists", "歌单", "☰"),
    ("upload", "上传", "↑"),
    ("sync", "同步", "⇅"),
    ("device", "设备", "ℹ"),
    ("settings", "设置", "⚙"),
    ("about", "关于", "⊙"),
];

/// cyrio 主应用
pub struct CyrioApp {
    /// 全局状态
    pub state: AppState,

    /// UI → 后台 命令通道
    cmd_tx: Sender<Command>,
    /// 后台 → UI 事件通道
    event_rx: Receiver<Event>,

    /// 是否已初始化（fonts/theme 只装一次）
    initialized: bool,
}

impl Default for CyrioApp {
    fn default() -> Self {
        let state = AppState::default();
        let (cmd_tx, event_rx) = spawn_task_loop(state.device.clone());
        Self {
            state,
            cmd_tx,
            event_rx,
            initialized: false,
        }
    }
}

impl CyrioApp {
    /// 创建并启动后台任务
    pub fn new() -> Self {
        Self::default()
    }

    /// 一次性初始化：字体、主题、动画
    fn init_once(&mut self, ctx: &Context) {
        if self.initialized {
            return;
        }
        fonts::install_cjk_font(ctx);
        fonts::configure_typography(ctx);
        theme::apply_theme(ctx);
        theme::configure_animation(ctx);
        self.initialized = true;
    }

    /// 处理后台事件队列（每帧调用）
    fn poll_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            self.handle_event(event);
        }
    }

    /// 处理单个事件
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::DeviceOpened(Ok(())) => {
                self.state.connected = true;
                self.state.connecting = false;
                self.state.audio = Some(cyrio_audio::manager::start_audio_thread());
                self.state.page_path = "songs".to_string();
                self.state.set_status("设备已连接");
                self.state.log("设备已连接");
                let _ = self.cmd_tx.try_send(Command::GetStorageStatus);
                // 双存储合并加载：同时请求内置 + SD 的歌曲和歌单
                self.state.pending_song_loads = 2;
                self.state.pending_playlist_loads = 2;
                self.state.loading = true;
                let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_INTERNAL));
                let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_SD));
                let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_INTERNAL));
                let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_SD));
            }
            Event::DeviceOpened(Err(e)) => {
                self.state.connected = false;
                self.state.connecting = false;
                self.state.set_status(format!("连接失败：{}", e));
                self.state.log(format!("连接失败：{}", e));
            }
            Event::DeviceClosed => {
                self.state.connected = false;
                self.state.connecting = false;
                if let Some(audio) = self.state.audio.take() {
                    audio.stop();
                }
                self.state.songs.clear();
                self.state.playlists.clear();
                self.state.internal_mem = None;
                self.state.sd_mem = None;
                self.state.selected_song_ids.clear();
                self.state.last_clicked_song_index = None;
                self.state.current_playing_file_no = None;
                self.state.set_status("设备已断开");
            }
            Event::DevicesScanned(devices) => {
                self.state.usb_devices = devices;
                self.state.scanning = false;
            }
            Event::UploadBatchCompleted(results) => {
                self.state.progress = None;
                self.state.hide_loading();
                let success = results.iter().filter(|r| r.success).count();
                let failed = results.len() - success;
                self.state.set_status(format!(
                    "批量上传完成：成功 {}，失败 {}",
                    success, failed
                ));
                // 标记传输对话框为完成状态，延迟 1.5 秒后清除
                // 哨兵值 f64::MAX：ui() 中替换为真实 ctx.time
                if let Some(ut) = self.state.upload_transfer.as_mut() {
                    ut.done_time = Some(f64::MAX);
                }
                // 上传后重新加载双存储歌曲
                let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_INTERNAL));
                let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_SD));
            }
            Event::SongDownloaded(result) => match result {
                Ok(data) => {
                    if let Some(audio) = &self.state.audio {
                        audio.play(data);
                        audio.set_loading(false);
                    }
                    self.state.set_status("开始播放");
                }
                Err(e) => {
                    self.state.set_status(format!("播放下载失败：{}", e));
                    if let Some(audio) = &self.state.audio {
                        audio.set_loading(false);
                    }
                }
            },
            Event::PlaylistRepaired(result) => match result {
                Ok(()) => {
                    self.state.set_status("歌单编码已修复");
                    let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_INTERNAL));
                    let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_SD));
                }
                Err(e) => self.state.set_status(format!("修复失败：{}", e)),
            },
            Event::RenameCompleted(result) => match result {
                Ok(()) => {
                    self.state.set_status("已重命名");
                    // 关闭重命名对话框
                    self.state.show_rename_dialog = None;
                    self.state.rename_input.clear();
                    // 重新加载双存储歌曲（title 变化需刷新列表）
                    let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_INTERNAL));
                    let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_SD));
                }
                Err(e) => self.state.set_status(format!("重命名失败：{}", e)),
            },
            Event::BatchOperationCompleted { kind, results } => {
                self.state.progress = None;
                self.state.hide_loading();
                let success = results.iter().filter(|r| r.success).count();
                let failed = results.len() - success;
                self.state.set_status(format!(
                    "{} 完成：成功 {}，失败 {}",
                    kind, success, failed
                ));
                // 重新加载双存储歌曲（title 变化需刷新列表）
                let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_INTERNAL));
                let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_SD));
            }
            Event::SongsListedForMem { songs, mem_unit } => {
                // 双存储合并：移除该 mem_unit 的旧项，追加新项（带 mem_unit 标记）
                self.state.songs.retain(|e| e.mem_unit != mem_unit);
                for file in songs {
                    self.state.songs.push(SongEntry { file, mem_unit });
                }
                if self.state.pending_song_loads > 0 {
                    self.state.pending_song_loads -= 1;
                }
                if self.state.pending_song_loads == 0 {
                    self.state.loading = false;
                }
                self.state.set_status(format!("已加载 {} 首歌曲", self.state.songs.len()));
            }
            Event::PlaylistsListedForMem { playlists, mem_unit } => {
                self.state.playlists.retain(|e| e.mem_unit != mem_unit);
                for file in playlists {
                    self.state.playlists.push(SongEntry { file, mem_unit });
                }
                if self.state.pending_playlist_loads > 0 {
                    self.state.pending_playlist_loads -= 1;
                }
                if self.state.pending_playlist_loads == 0 {
                    self.state.loading = false;
                }
                self.state
                    .set_status(format!("已加载 {} 个歌单", self.state.playlists.len()));
            }
            Event::PlaylistSongsListed(result) => {
                self.state.loading_playlist_songs = false;
                match result {
                    Ok(songs) => {
                        self.state.playlist_songs = songs;
                        self.state
                            .set_status(format!("已加载歌单内 {} 首歌曲", self.state.playlist_songs.len()));
                    }
                    Err(e) => {
                        self.state.set_status(format!("加载歌单内容失败：{}", e));
                    }
                }
            }
            Event::UploadProgress {
                sent_bytes,
                total_bytes,
            } => {
                if let Some(p) = self.state.progress.as_mut() {
                    p.current = sent_bytes;
                    p.total = total_bytes;
                }
                // 同步更新上传传输对话框中当前文件的字节进度
                if let Some(ut) = self.state.upload_transfer.as_mut() {
                    if let Some(f) = ut.files.get_mut(ut.current_index) {
                        f.transferred = sent_bytes;
                        f.total = total_bytes;
                    }
                }
            }
            Event::UploadBatchStarted { names } => {
                // 初始化上传传输对话框
                let files = names
                    .iter()
                    .map(|n| crate::state::UploadFileEntry {
                        name: n.clone(),
                        transferred: 0,
                        total: 0,
                        status: crate::state::UploadFileStatus::Pending,
                    })
                    .collect();
                self.state.upload_transfer = Some(crate::state::UploadTransferState {
                    files,
                    current_index: 0,
                    done_time: None,
                });
            }
            Event::UploadFileStarted { index, name: _ } => {
                if let Some(ut) = self.state.upload_transfer.as_mut() {
                    ut.current_index = index;
                    if let Some(f) = ut.files.get_mut(index) {
                        f.status = crate::state::UploadFileStatus::Uploading;
                    }
                }
            }
            Event::UploadFileCompleted { index, success } => {
                if let Some(ut) = self.state.upload_transfer.as_mut() {
                    if let Some(f) = ut.files.get_mut(index) {
                        f.status = if success {
                            crate::state::UploadFileStatus::Done
                        } else {
                            crate::state::UploadFileStatus::Failed
                        };
                        // 完成时确保 transferred == total
                        if success {
                            f.transferred = f.total;
                        }
                    }
                }
            }
            Event::UploadCompleted(result) => {
                self.state.progress = None;
                match result {
                    Ok(file_no) => {
                        self.state.set_status(format!("上传完成，新文件号 {}", file_no));
                        let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_INTERNAL));
                        let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_SD));
                    }
                    Err(e) => self.state.set_status(format!("上传失败：{}", e)),
                }
            }
            Event::DownloadProgress {
                received_bytes,
                total_bytes,
            } => {
                if let Some(p) = self.state.progress.as_mut() {
                    p.current = received_bytes;
                    p.total = total_bytes;
                }
            }
            Event::DownloadCompleted(result) => {
                self.state.progress = None;
                match result {
                    Ok(()) => self.state.set_status("下载完成"),
                    Err(e) => self.state.set_status(format!("下载失败：{}", e)),
                }
            }
            Event::DeleteCompleted(result) => {
                self.state.progress = None;
                match result {
                    Ok(()) => {
                        self.state.set_status("删除完成");
                        let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_INTERNAL));
                        let _ = self.cmd_tx.try_send(Command::ListSongs(MEM_UNIT_SD));
                        let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_INTERNAL));
                        let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_SD));
                    }
                    Err(e) => self.state.set_status(format!("删除失败：{}", e)),
                }
            }
            Event::AddToPlaylistCompleted(result) => match result {
                Ok(()) => self.state.set_status("已加入歌单"),
                Err(e) => self.state.set_status(format!("加入歌单失败：{}", e)),
            },
            Event::CreatePlaylistCompleted(result) => match result {
                Ok(file_no) => {
                    self.state.set_status(format!("已创建歌单，文件号 {}", file_no));
                    let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_INTERNAL));
                    let _ = self.cmd_tx.try_send(Command::ListPlaylists(MEM_UNIT_SD));
                }
                Err(e) => self.state.set_status(format!("创建歌单失败：{}", e)),
            },
            Event::StorageStatusGot(Ok(status)) => {
                self.state.internal_mem = Some(cyrio_core::protocol::rio_mem::RioMem {
                    size: status.internal.size as u32,
                    used: status.internal.used as u32,
                    free: status.internal.free as u32,
                    system: 0,
                    name: status.internal.name,
                    model: status.internal.model,
                });
                self.state.sd_mem = Some(cyrio_core::protocol::rio_mem::RioMem {
                    size: status.sd_card.size as u32,
                    used: status.sd_card.used as u32,
                    free: status.sd_card.free as u32,
                    system: 0,
                    name: status.sd_card.name,
                    model: status.sd_card.model,
                });
            }
            Event::StorageStatusGot(Err(e)) => {
                self.state.set_status(format!("读取存储信息失败：{}", e));
            }
            Event::Log(msg) => self.state.log(msg),
        }
    }

    /// 顶部工具栏
    /// 顶栏：[← back 36×36][虚拟U盘 h30][menu-bar flex:1][paginate 30×30]
    /// 对齐 Tauri .top-bar（gap 12px）
    fn render_top_bar(&mut self, ui: &mut Ui) {
        let webdav_running = matches!(self.state.webdav_status, WebDavStatus::Running { .. });
        let connected = self.state.connected;
        let mut start_webdav = false;
        let mut stop_webdav = false;
        let mut disconnect = false;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            // [← back-btn 36×36] 对齐 .device-circle-mini
            let back_btn = egui::Button::new(
                egui::RichText::new("←").color(theme::RIO_TEXT).size(20.0),
            )
            .min_size(egui::vec2(36.0, 36.0))
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .corner_radius(4);
            let back_resp = ui.add(back_btn);
            if back_resp.clicked() {
                disconnect = true;
            }

            // [虚拟U盘 btn h30] 对齐 .webdav-btn
            ui.add_enabled_ui(connected, |ui| {
                let label = if webdav_running { "停止虚拟U盘" } else { "虚拟U盘" };
                let btn = if webdav_running {
                    // 运行态：rio-blue 底 + 白字
                    egui::Button::new(
                        egui::RichText::new(label).color(egui::Color32::WHITE).size(11.0),
                    )
                    .min_size(egui::vec2(0.0, 30.0))
                    .fill(theme::RIO_BLUE)
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BLUE))
                    .corner_radius(4)
                } else {
                    // 未运行：显式设置文字颜色确保可见
                    egui::Button::new(
                        egui::RichText::new(label).color(theme::RIO_TEXT).size(11.0),
                    )
                    .min_size(egui::vec2(0.0, 30.0))
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                    .corner_radius(4)
                };
                if ui.add(btn).clicked() {
                    if webdav_running {
                        stop_webdav = true;
                    } else {
                        start_webdav = true;
                    }
                }
            });

            // [menu-bar flex:1] 对齐 .menu-bar（白底 3px padding 4px 圆角 1px border）
            let menu_bar_frame = egui::Frame::new()
                .fill(theme::RIO_CONTENT_BG)
                .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                .inner_margin(egui::Margin::same(3))
                .corner_radius(4);
            menu_bar_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 1.0;
                    for (action, label, icon) in MENU_ITEMS {
                        let is_active = self.state.page_path == *action;
                        let item_id = ui.id().with(("menu_item", action));

                        // 用上一帧的 rect 检测 hover（当前帧 rect 尚未分配）
                        let prev_rect = ui.memory(|m| m.data.get_temp::<egui::Rect>(item_id));
                        let hovered = prev_rect
                            .map(|r| ui.rect_contains_pointer(r))
                            .unwrap_or(false);
                        let hover_target = if is_active || hovered { 1.0 } else { 0.0 };
                        let hover_anim =
                            ui.ctx().animate_value_with_time(item_id, hover_target, 0.15);

                        // 动态计算背景色，通过 .fill() 交给 Button 绘制
                        // Button 内部先画 frame（背景）再画 text，不会覆盖文字
                        // 用 RIO_CONTENT_BG（白底）作 lerp 起点，避免 TRANSPARENT 的 RGB=(0,0,0) 导致过渡变黑
                        let bg = if is_active {
                            theme::RIO_BLUE
                        } else {
                            theme::lerp_color(theme::RIO_CONTENT_BG, theme::RIO_BG_HOVER, hover_anim)
                        };

                        let item_btn = egui::Button::new(
                            egui::RichText::new(format!("{} {}", icon, label))
                                .color(if is_active {
                                    egui::Color32::WHITE
                                } else {
                                    theme::RIO_TEXT_SECONDARY
                                })
                                .size(12.0),
                        )
                        .fill(bg)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(3)
                        .min_size(egui::vec2(0.0, 24.0));
                        let resp = ui.add(item_btn);

                        // 保存当前帧 rect 供下一帧 hover 检测
                        ui.memory_mut(|m| m.data.insert_temp(item_id, resp.rect));

                        if resp.clicked() {
                            self.state.page_path = action.to_string();
                            if *action == "device" && self.state.connected {
                                let _ = self.cmd_tx.try_send(Command::GetStorageStatus);
                            }
                        }
                    }
                });
            });

            // [paginate-toggle 30×30] 对齐 .paginate-toggle
            let pag_icon = if self.state.paginate { "▤" } else { "☰" };
            let pag_btn = if self.state.paginate {
                // 开启态：rio-blue 底 + 白字
                egui::Button::new(
                    egui::RichText::new(pag_icon).color(egui::Color32::WHITE).size(14.0),
                )
                .min_size(egui::vec2(30.0, 30.0))
                .fill(theme::RIO_BLUE)
                .stroke(egui::Stroke::new(1.0, theme::RIO_BLUE))
                .corner_radius(4)
            } else {
                // 关闭态：显式设置文字颜色确保可见
                egui::Button::new(
                    egui::RichText::new(pag_icon).color(theme::RIO_TEXT).size(14.0),
                )
                .min_size(egui::vec2(30.0, 30.0))
                .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                .corner_radius(4)
            };
            if ui.add(pag_btn).clicked() {
                self.state.paginate = !self.state.paginate;
            }
        });

        if disconnect {
            let _ = self.cmd_tx.try_send(Command::CloseDevice);
        }
        if start_webdav {
            match self.state.webdav.start(self.state.device.clone()) {
                Ok(addr) => {
                    self.state.webdav_status = WebDavStatus::Running { addr: addr.clone() };
                    self.state.set_status(format!("虚拟U盘已启动：{}", addr));
                    smol::spawn(async move {
                        let _ = smol::unblock(|| cyrio_webdav::mount_webdav()).await;
                    })
                    .detach();
                }
                Err(e) => self.state.set_status(format!("启动失败：{}", e)),
            }
        }
        if stop_webdav {
            match self.state.webdav.stop() {
                Ok(()) => {
                    self.state.webdav_status = WebDavStatus::Stopped;
                    self.state.set_status("虚拟U盘已停止");
                }
                Err(e) => self.state.set_status(format!("停止失败：{}", e)),
            }
        }
    }

    /// 底部存储状态条 对齐 .storage-status-bar（26px，内置蓝/SD橙 mini-bar 3px）
    fn render_storage_bar(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            if !self.state.connected {
                ui.colored_label(
                    theme::RIO_TEXT_DIM,
                    egui::RichText::new("未连接设备").size(11.0),
                );
                return;
            }
            // 内置
            if let Some(m) = self.state.internal_mem.as_ref() {
                if m.is_present() {
                    render_storage_item(ui, "内置", m, theme::RIO_BLUE);
                }
            }
            // SD
            if let Some(m) = self.state.sd_mem.as_ref() {
                if m.is_present() {
                    render_storage_item(ui, "SD", m, theme::RIO_S30S_ORANGE);
                }
            }
            // 右侧状态消息
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(p) = &self.state.progress {
                    ui.colored_label(theme::RIO_BLUE, p.label());
                }
            });
        });
    }

    /// notice toast 左下角提示（深底白字 + 左侧 3px 高亮边条）
    fn render_notice_toast(&mut self, ctx: &Context) {
        if self.state.notice_message.is_none() {
            return;
        }
        let mut should_close = false;
        egui::Area::new(egui::Id::new("notice_toast"))
            .anchor(egui::Align2::LEFT_BOTTOM, [20.0, -20.0])
            .interactable(true)
            .show(ctx, |ui| {
                let frame = egui::Frame::new()
                    .fill(theme::RIO_NOTICE_BG)
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 10,
                        top: 7,
                        bottom: 7,
                    })
                    .corner_radius(4)
                    .shadow(egui::Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(40),
                    });
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // 动态宽度：根据文本测量，范围 [100, 360]
                        let msg = self.state.notice_message.clone().unwrap_or_default();
                        let text_w = ui.painter().fonts_mut(|f| {
                            f.layout_no_wrap(
                                msg.clone(),
                                egui::FontId::proportional(12.0),
                                egui::Color32::WHITE,
                            ).size().x
                        });
                        // bar(3) + gap(8) + text + gap(8) + close(16) = text + 35
                        let dyn_w = (text_w + 35.0).clamp(100.0, 360.0);
                        ui.set_width(dyn_w);
                        // 左侧 3px 高亮边条
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(3.0, 16.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(bar_rect, 2.0, theme::RIO_BLUE);
                        ui.colored_label(
                            egui::Color32::WHITE,
                            egui::RichText::new(&msg).size(12.0),
                        );
                        // 右侧固定 16×16 close 按钮（手动绘制，避免 Button hover 导致布局抖动）
                        let (close_rect, close_resp) = ui.allocate_exact_size(
                            egui::vec2(16.0, 16.0),
                            egui::Sense::click(),
                        );
                        let close_bg = if close_resp.hovered() {
                            egui::Color32::from_white_alpha(50)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(close_rect, 8.0, close_bg);
                        ui.painter().text(
                            close_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "×",
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_white_alpha(180),
                        );
                        if close_resp.clicked() {
                            should_close = true;
                        }
                    });
                });
            });
        if should_close {
            self.state.clear_notice();
        }
    }

    /// 调试窗口
    fn render_debug_window(&mut self, ctx: &Context) {
        if !self.state.debug_window_open {
            return;
        }
        egui::Window::new("调试 (Alt+Shift+D)")
            .default_width(500.0)
            .default_height(360.0)
            .show(ctx, |ui| {
                egui::Grid::new("debug_grid")
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        ui.strong("page_path");
                        let mut path_buf = self.state.page_path.clone();
                        ui.add(egui::TextEdit::singleline(&mut path_buf).desired_width(200.0));
                        if path_buf != self.state.page_path {
                            self.state.page_path = path_buf;
                        }
                        ui.end_row();

                        ui.strong("mem_unit");
                        ui.label(format!("{}", self.state.mem_unit));
                        ui.end_row();

                        ui.strong("connected");
                        ui.label(format!("{}", self.state.connected));
                        ui.end_row();

                        ui.strong("songs.len");
                        ui.label(format!("{}", self.state.songs.len()));
                        ui.end_row();

                        ui.strong("playlists.len");
                        ui.label(format!("{}", self.state.playlists.len()));
                        ui.end_row();

                        ui.strong("selected_songs");
                        ui.label(format!("{}", self.state.selected_song_ids.len()));
                        ui.end_row();
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("重置路由 → songs").clicked() {
                        self.state.page_path = "songs".to_string();
                    }
                    if ui.button("清空选择").clicked() {
                        self.state.selected_song_ids.clear();
                        self.state.selected_playlist_id = None;
                    }
                    if ui.button("清空日志").clicked() {
                        if let Ok(mut logs) = self.state.logs.write() {
                            logs.clear();
                        }
                    }
                });
                ui.separator();
                ui.heading("日志");
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        if let Ok(logs) = self.state.logs.read() {
                            for line in logs.iter().rev() {
                                ui.colored_label(theme::RIO_TEXT_DIM, line);
                            }
                        }
                    });
            });
    }

    /// 渲染内容区（根据 page_path 调度）
    fn render_content(&mut self, ui: &mut Ui, ctx: &Context) {
        // 未连接时显示设备连接场景
        if !self.state.connected {
            pages::connect::render(ui, ctx, &mut self.state, &self.cmd_tx);
            return;
        }
        let page_path = self.state.page_path.clone();
        match page_path.as_str() {
            "songs" => pages::songs::render(ui, ctx, &mut self.state, &self.cmd_tx),
            "playlists" => pages::playlists::render(ui, ctx, &mut self.state, &self.cmd_tx),
            "upload" => pages::upload::render(ui, ctx, &mut self.state, &self.cmd_tx),
            "sync" => pages::sync::render(ui, ctx, &mut self.state, &self.cmd_tx),
            "device" => pages::device::render(ui, ctx, &mut self.state, &self.cmd_tx),
            "settings" => pages::settings::render(ui, ctx, &mut self.state, &self.cmd_tx),
            "about" => pages::about::render(ui, ctx, &mut self.state, &self.cmd_tx),
            other => {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.colored_label(theme::RIO_TEXT_DIM, format!("未知页面：{}", other));
                    ui.add_space(8.0);
                    if ui.button("回到歌曲页").clicked() {
                        self.state.page_path = "songs".to_string();
                    }
                });
            }
        }
    }

    /// 底部音频播放器条 对齐 .player-bar（48px [info][▶/⏸ ⏹][time+progress+time][×]）
    fn render_player_bar(&mut self, ui: &mut Ui, _ctx: &Context) {
        self.state.update_playback_state();

        let is_playing = self.state.playback_state.is_playing;
        let is_loading = self.state.playback_state.is_loading;
        let position = self.state.playback_state.position;
        let duration = self.state.playback_state.duration;
        let has_track = self.state.current_playing_file_no.is_some();
        let current_file_no = self.state.current_playing_file_no;

        // 查找当前播放歌曲标题（songs 现为 Vec<SongEntry>）
        let title_subtitle = current_file_no.and_then(|file_no| {
            self.state.songs.iter().find(|e| e.file.file_no == file_no).map(|e| {
                let title = if !e.file.title.is_empty() {
                    e.file.title.clone()
                } else {
                    e.file.name.clone()
                };
                let subtitle = if !e.file.artist.is_empty() {
                    e.file.artist.clone()
                } else {
                    "未知艺术家".to_string()
                };
                (title, subtitle)
            })
        });

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;

            // [player-info] title 12px 600 + subtitle 10px dim
            ui.vertical(|ui| {
                if let Some((title, subtitle)) = &title_subtitle {
                    ui.label(egui::RichText::new(title).size(12.0).strong().color(theme::RIO_TEXT));
                    ui.label(egui::RichText::new(subtitle).size(10.0).color(theme::RIO_TEXT_DIM));
                } else {
                    ui.label(egui::RichText::new("未播放").size(12.0).color(theme::RIO_TEXT_DIM));
                }
            });

            // [▶/⏸ 28×28 蓝底白字]
            let play_label = if is_loading { "⏳" } else if is_playing { "⏸" } else { "▶" };
            let play_btn = egui::Button::new(
                egui::RichText::new(play_label).size(13.0).color(egui::Color32::WHITE),
            )
            .min_size(egui::vec2(28.0, 28.0))
            .fill(theme::RIO_BLUE)
            .corner_radius(4);
            if ui.add_enabled(has_track, play_btn).clicked() {
                if let Some(audio) = &self.state.audio {
                    if is_playing {
                        audio.pause();
                    } else {
                        audio.resume();
                    }
                }
            }

            // [⏹ 28×28 bg-subtle]
            let stop_btn = egui::Button::new(
                egui::RichText::new("⏹").size(13.0).color(theme::RIO_TEXT_SECONDARY),
            )
            .min_size(egui::vec2(28.0, 28.0))
            .fill(theme::RIO_BG_SUBTLE)
            .corner_radius(4);
            if ui.add_enabled(has_track, stop_btn).clicked() {
                if let Some(audio) = &self.state.audio {
                    audio.stop();
                }
                self.state.current_playing_file_no = None;
            }

            // [time 10px][progress 4px h 蓝色 flex:1][time 10px]
            let pos_str = format_time_sec(position as f64);
            let dur_str = format_time_sec(duration as f64);
            ui.label(egui::RichText::new(pos_str).size(10.0).color(theme::RIO_TEXT_DIM));
            let frac = if duration > 0.0 {
                (position / duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ui.add(
                egui::ProgressBar::new(frac)
                    .desired_height(6.0)
                    .desired_width(ui.available_width().max(60.0))
                    .fill(theme::RIO_BLUE),
            );
            ui.label(egui::RichText::new(dur_str).size(10.0).color(theme::RIO_TEXT_DIM));

            // [× 24×24 hover 红底]
            let close_btn = egui::Button::new(
                egui::RichText::new("×").size(14.0).color(theme::RIO_TEXT_DIM),
            )
            .min_size(egui::vec2(24.0, 24.0))
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .corner_radius(3);
            let close_resp = ui.add_enabled(has_track, close_btn);
            if close_resp.hovered() {
                ui.painter()
                    .rect_filled(close_resp.rect, 3.0, theme::RIO_ACCENT_SOFT);
            }
            if close_resp.clicked() {
                if let Some(audio) = &self.state.audio {
                    audio.stop();
                }
                self.state.current_playing_file_no = None;
            }
        });
    }

    /// 二次确认对话框
    fn render_confirm_dialog(&mut self, ctx: &Context) {
        let dialog = match self.state.confirm_dialog.take() {
            Some(d) => d,
            None => return,
        };
        let action = dialog.action.clone();
        let message = dialog.message.clone();

        let frame = egui::Frame::new()
            .fill(theme::RIO_CONTENT_BG)
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .inner_margin(egui::Margin::same(20))
            .corner_radius(6);

        let mut confirmed = false;
        let resp = egui::Modal::new(egui::Id::new("confirm_dialog"))
            .backdrop_color(theme::RIO_OVERLAY)
            .frame(frame)
            .show(ctx, |ui| {
                ui.colored_label(theme::RIO_DANGER, &message);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        ui.close();
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("确认").color(egui::Color32::WHITE),
                            )
                            .fill(theme::RIO_DANGER),
                        )
                        .clicked()
                    {
                        confirmed = true;
                        ui.close();
                    }
                });
            });

        if confirmed {
            match action {
                ConfirmAction::DeleteSong { file_no, mem_unit } => {
                    let _ = self
                        .cmd_tx
                        .try_send(Command::DeleteSong { file_no, mem_unit });
                }
                ConfirmAction::DeleteSongsBatch { ref file_nos, mem_unit } => {
                    for file_no in file_nos {
                        let _ = self
                            .cmd_tx
                            .try_send(Command::DeleteSong { file_no: *file_no, mem_unit });
                    }
                }
                ConfirmAction::DeletePlaylist { file_no, mem_unit } => {
                    let _ = self
                        .cmd_tx
                        .try_send(Command::DeleteSong { file_no, mem_unit });
                }
                ConfirmAction::Format { mem_unit: _ } => {
                    self.state.set_status("格式化功能暂不支持");
                }
            }
        }

        if !resp.should_close() {
            self.state.confirm_dialog = Some(ConfirmDialog { action, message });
        }
    }

    /// 加载遮罩（居中模态 + 真实进度）
    fn render_loading_modal(&self, ctx: &Context) {
        let msg = match &self.state.show_loading_modal {
            Some(m) => m.clone(),
            None => return,
        };
        let frame = egui::Frame::new()
            .fill(theme::RIO_CONTENT_BG)
            .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
            .inner_margin(egui::Margin::same(24))
            .corner_radius(6);
        egui::Modal::new(egui::Id::new("loading_modal"))
            .backdrop_color(theme::RIO_OVERLAY)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.strong(&msg);
                });
                // 真实进度：进度条 + 字节数
                if let Some(p) = &self.state.progress {
                    ui.add_space(10.0);
                    ui.add(
                        egui::ProgressBar::new(p.fraction())
                            .desired_height(6.0)
                            .desired_width(240.0)
                            .fill(theme::RIO_BLUE),
                    );
                    ui.add_space(4.0);
                    ui.colored_label(
                        theme::RIO_TEXT_DIM,
                        egui::RichText::new(p.label()).size(11.0),
                    );
                }
            });
    }

    /// 上传传输侧栏（现代风格，左侧非模态，对齐 Tauri UploadSidebar）
    /// - 白底圆角，与主页面样式统一
    /// - 文件图标用文字字符（♪ → ✓ × ·），不用 emoji
    /// - 进度条：3px 圆角，RIO_BLUE 填充，RIO_BG_SUBTLE 背景
    /// - 文件列表 hover 为 RIO_BG_HOVER
    fn render_upload_transfer_sidebar(&self, ui: &mut egui::Ui) {
        let ut = match &self.state.upload_transfer {
            Some(u) => u,
            None => return,
        };

        ui.spacing_mut().item_spacing.y = 6.0;
        let sidebar_width = ui.available_width();

        // ===== 标题栏（浅灰底 + 主色文字，4px 圆角） =====
        let all_done = ut.all_done();
        let title = if all_done { "传输完成" } else { "正在传输" };
        let titlebar_height = 28.0;
        let (titlebar_rect, _) = ui.allocate_exact_size(
            egui::vec2(sidebar_width, titlebar_height),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(titlebar_rect, 4.0, theme::RIO_BG_SUBTLE);
        ui.painter().text(
            titlebar_rect.center(),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(12.0),
            theme::RIO_TEXT,
        );

        // ===== 简化动画区：电脑 → 文件飞行 → 设备（现代风格） =====
        let anim_height = 48.0;
        let (anim_rect, _) = ui.allocate_exact_size(
            egui::vec2(sidebar_width, anim_height),
            egui::Sense::hover(),
        );
        let painter = ui.painter();

        // 现代风格：圆角背景 + 两个圆角矩形图标
        painter.rect_filled(anim_rect, 4.0, theme::RIO_BG_SUBTLE);

        // 源图标（左）：电脑符号
        let icon_size = 24.0_f32;
        let src_x = anim_rect.min.x + 16.0;
        let src_y = anim_rect.center().y - icon_size / 2.0;
        let src_rect = egui::Rect::from_min_size(egui::pos2(src_x, src_y), egui::vec2(icon_size, icon_size));
        painter.rect_filled(src_rect, 4.0, theme::RIO_CONTENT_BG);
        painter.text(
            src_rect.center(),
            egui::Align2::CENTER_CENTER,
            "♪",
            egui::FontId::proportional(14.0),
            theme::RIO_TEXT_SECONDARY,
        );
        painter.text(
            egui::pos2(src_x + icon_size / 2.0, src_y + icon_size + 4.0),
            egui::Align2::CENTER_TOP,
            "电脑",
            egui::FontId::proportional(9.0),
            theme::RIO_TEXT_DIM,
        );

        // 目标图标（右）：Rio 设备
        let dst_x = anim_rect.max.x - 16.0 - icon_size;
        let dst_y = src_y;
        let dst_rect = egui::Rect::from_min_size(egui::pos2(dst_x, dst_y), egui::vec2(icon_size, icon_size));
        painter.rect_filled(dst_rect, 4.0, theme::RIO_BLUE);
        painter.text(
            dst_rect.center(),
            egui::Align2::CENTER_CENTER,
            "♪",
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
        painter.text(
            egui::pos2(dst_x + icon_size / 2.0, dst_y + icon_size + 4.0),
            egui::Align2::CENTER_TOP,
            "Rio",
            egui::FontId::proportional(9.0),
            theme::RIO_TEXT_DIM,
        );

        // 飞行文件动画（抛物线运动，现代风格小圆点）
        if !all_done {
            let fly_t = ((ui.input(|i| i.time) % 1.8) / 1.8) as f32; // 0..1
            let fly_start_x = src_x + icon_size;
            let fly_end_x = dst_x;
            let fly_x = fly_start_x + (fly_end_x - fly_start_x) * fly_t;
            // 抛物线：中点最高
            let fly_y = anim_rect.center().y - ((fly_t * std::f32::consts::PI).sin()) * 8.0;
            painter.circle_filled(egui::pos2(fly_x, fly_y), 3.0, theme::RIO_BLUE);
        }

        ui.add_space(4.0);

        // ===== 总进度（现代圆角进度条） =====
        let total_count = ut.files.len();
        let done_count = ut.done_count();
        let failed_count = ut.failed_count();
        let total_bytes = ut.total_bytes();
        let transferred_bytes = ut.transferred_bytes();

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("总进度")
                    .size(11.0)
                    .color(theme::RIO_TEXT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if failed_count > 0 {
                    ui.label(
                        egui::RichText::new(format!(" (失败 {})", failed_count))
                            .size(10.0)
                            .color(theme::RIO_DANGER),
                    );
                }
                ui.label(
                    egui::RichText::new(format!("{} / {}", done_count, total_count))
                        .size(10.0)
                        .color(theme::RIO_TEXT_SECONDARY),
                );
            });
        });

        // 现代进度条：3px 圆角，RIO_BG_SUBTLE 背景 + RIO_BLUE 填充
        let (bar_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 6.0),
            egui::Sense::hover(),
        );
        let painter = ui.painter();
        painter.rect_filled(bar_rect, 3.0, theme::RIO_BG_SUBTLE);
        let fill_w = bar_rect.width() * ut.total_fraction();
        if fill_w > 1.0 {
            let fill_rect = egui::Rect::from_min_size(
                bar_rect.min,
                egui::vec2(fill_w, bar_rect.height()),
            );
            painter.rect_filled(fill_rect, 3.0, theme::RIO_BLUE);
        }

        if total_bytes > 0 {
            ui.label(
                egui::RichText::new(format!(
                    "{} / {}",
                    crate::state::format_bytes(transferred_bytes),
                    crate::state::format_bytes(total_bytes)
                ))
                .size(10.0)
                .color(theme::RIO_TEXT_DIM),
            );
        }

        ui.add_space(4.0);

        // ===== 当前文件进度 =====
        if let Some(cf) = ut.current_file() {
            let cf_frac = if cf.total > 0 {
                cf.transferred as f32 / cf.total as f32
            } else {
                0.0
            };
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&cf.name)
                        .size(11.0)
                        .color(theme::RIO_TEXT)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{}%", (cf_frac * 100.0) as u32))
                            .size(11.0)
                            .color(theme::RIO_BLUE)
                            .strong(),
                    );
                });
            });

            // 现代进度条
            let (bar_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 6.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter();
            painter.rect_filled(bar_rect, 3.0, theme::RIO_BG_SUBTLE);
            let fill_w = bar_rect.width() * cf_frac;
            if fill_w > 1.0 {
                let fill_rect = egui::Rect::from_min_size(
                    bar_rect.min,
                    egui::vec2(fill_w, bar_rect.height()),
                );
                painter.rect_filled(fill_rect, 3.0, theme::RIO_BLUE_LIGHT);
            }

            if cf.total > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "{} / {}",
                        crate::state::format_bytes(cf.transferred),
                        crate::state::format_bytes(cf.total)
                    ))
                    .size(10.0)
                    .color(theme::RIO_TEXT_DIM),
                );
            }
            ui.add_space(4.0);
        }

        // ===== 文件列表（现代风格，hover 为 RIO_BG_HOVER） =====
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .show(ui, |ui| {
                for f in ut.files.iter() {
                    let (icon, icon_color) = match f.status {
                        crate::state::UploadFileStatus::Uploading => ("→", theme::RIO_BLUE),
                        crate::state::UploadFileStatus::Done => ("✓", theme::RIO_SUCCESS),
                        crate::state::UploadFileStatus::Failed => ("×", theme::RIO_DANGER),
                        crate::state::UploadFileStatus::Pending => ("·", theme::RIO_TEXT_DIM),
                    };
                    let name_color = match f.status {
                        crate::state::UploadFileStatus::Uploading => theme::RIO_TEXT,
                        crate::state::UploadFileStatus::Done => theme::RIO_TEXT_DIM,
                        crate::state::UploadFileStatus::Failed => theme::RIO_DANGER,
                        crate::state::UploadFileStatus::Pending => theme::RIO_TEXT_SECONDARY,
                    };

                    let row_resp = ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(icon)
                                .size(11.0)
                                .color(icon_color)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(&f.name)
                                .size(11.0)
                                .color(name_color),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if f.status == crate::state::UploadFileStatus::Uploading
                                    && f.total > 0
                                {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} / {}",
                                            crate::state::format_bytes(f.transferred),
                                            crate::state::format_bytes(f.total)
                                        ))
                                        .size(10.0)
                                        .color(theme::RIO_TEXT_DIM),
                                    );
                                } else if f.status == crate::state::UploadFileStatus::Done
                                    && f.total > 0
                                {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}",
                                            crate::state::format_bytes(f.total)
                                        ))
                                        .size(10.0)
                                        .color(theme::RIO_SUCCESS),
                                    );
                                }
                            },
                        );
                    });
                    // 现代风格：hover 时整行变浅蓝底
                    if row_resp.response.hovered() {
                        let hover_rect = row_resp.response.rect;
                        ui.painter().rect_filled(hover_rect, 3.0, theme::RIO_BG_HOVER);
                    }
                }
            });
    }

    /// 检查路由字符串是否有效（Phase 5a：hash 路由验证）
    fn is_valid_route(&self, route: &str) -> bool {
        MENU_ITEMS.iter().any(|(action, _, _)| *action == route)
    }

    /// 同步 hash 路由（Phase 5a）
    /// - WASM：双向同步 `state.page_path` ↔ `window.location.hash`
    /// - 桌面：no-op（`#[cfg]` 编译期排除）
    ///
    /// 通过 egui memory 记录 `last_synced` 区分变更来源：
    /// - `page_path != last_synced` → UI 发起变更 → 写入 hash
    /// - `hash != last_synced` → 浏览器发起变更（前进/后退/手动改 URL）→ 读入 state
    fn sync_hash_routing(&mut self, ctx: &Context) {
        #[cfg(target_arch = "wasm32")]
        {
            let id = egui::Id::new("hash_route_last_synced");
            let last_synced: String = ctx
                .data_mut(|d| d.get_temp(id))
                .unwrap_or_else(|| self.state.page_path.clone());
            let current_hash = read_web_hash();

            if self.state.page_path != last_synced {
                // UI 发起变更（用户点击菜单项 / 设备连接后切到 songs）→ 写入 hash
                write_web_hash(&self.state.page_path);
                ctx.data_mut(|d| {
                    d.insert_temp(id, self.state.page_path.clone());
                });
            } else if current_hash != last_synced {
                // 浏览器发起变更 → 若有效则读入 state，否则用 state 覆盖 hash
                if self.is_valid_route(&current_hash) {
                    self.state.page_path = current_hash.clone();
                    ctx.data_mut(|d| {
                        d.insert_temp(id, current_hash);
                    });
                } else {
                    write_web_hash(&self.state.page_path);
                    ctx.data_mut(|d| {
                        d.insert_temp(id, self.state.page_path.clone());
                    });
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // 桌面端：no-op
            let _ = ctx;
        }
    }
}

// ============================================================================
// Phase 5a: WASM hash 路由辅助函数
// ============================================================================

/// 读取浏览器 `window.location.hash`，返回去掉 `#` 和 `/` 前缀的路由字符串
/// 例如 `#/songs` → `songs`，`#songs` → `songs`，空 hash → `""`
#[cfg(target_arch = "wasm32")]
fn read_web_hash() -> String {
    use web_sys::window;
    let Some(win) = window() else {
        return String::new();
    };
    let loc = win.location();
    let hash = loc.hash().unwrap_or_default();
    // 去掉前缀 `#`，再去掉前缀 `/`
    let stripped = hash.strip_prefix('#').unwrap_or(&hash);
    stripped.strip_prefix('/').unwrap_or(stripped).to_string()
}

/// 写入浏览器 `window.location.hash`，格式为 `#/<route>`
/// 例如 route=`songs` → URL 变为 `.../#/songs`
#[cfg(target_arch = "wasm32")]
fn write_web_hash(route: &str) {
    use web_sys::window;
    let Some(win) = window() else {
        return;
    };
    let loc = win.location();
    let new_hash = format!("#/{}", route);
    // set_hash 会触发 hashchange 事件，但下一帧 read_web_hash 会读到新值
    let _ = loc.set_hash(&new_hash);
}

/// 渲染存储状态项（对齐 .storage-status-item + .storage-status-mini-bar 3px）
fn render_storage_item(
    ui: &mut Ui,
    name: &str,
    m: &cyrio_core::protocol::rio_mem::RioMem,
    bar_color: egui::Color32,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(egui::RichText::new(name).size(10.0).color(theme::RIO_TEXT_DIM).strong());
        let frac = if m.size > 0 {
            (m.used as f32 / m.size as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ui.add(
            egui::ProgressBar::new(frac)
                .desired_width(120.0)
                .desired_height(6.0)
                .fill(bar_color),
        );
        ui.label(
            egui::RichText::new(format!(
                "{} / {}",
                format_bytes(m.free as u64),
                format_bytes(m.size as u64)
            ))
            .size(11.0)
            .color(theme::RIO_TEXT_DIM),
        );
    });
}

/// 秒数格式化为 mm:ss（播放器时间显示）
fn format_time_sec(sec: f64) -> String {
    if sec.is_finite() && sec >= 0.0 {
        let total = sec as u32;
        format!("{}:{:02}", total / 60, total % 60)
    } else {
        "0:00".to_string()
    }
}

impl eframe::App for CyrioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        self.init_once(&ctx);

        // Alt+Shift+D 切换调试窗口
        let toggle_debug = ctx.input(|i| {
            i.key_pressed(egui::Key::D) && i.modifiers.alt && i.modifiers.shift
        });
        if toggle_debug {
            self.state.debug_window_open = !self.state.debug_window_open;
        }

        // 处理后台事件
        self.poll_events();

        // 同步 hash 路由（Phase 5a：WASM 双向同步 state.page_path ↔ window.location.hash）
        self.sync_hash_routing(&ctx);

        // notice toast 自动消失（3 秒）；哨兵值 f64::MAX 替换为真实时间
        let now = ctx.input(|i| i.time);
        if self.state.notice_message.is_some() {
            if self.state.notice_time == f64::MAX {
                self.state.notice_time = now;
            } else if now - self.state.notice_time > 3.0 {
                self.state.clear_notice();
            }
        }

        // 上传传输对话框：完成后延迟 1.5 秒清除
        if let Some(ut) = self.state.upload_transfer.as_mut() {
            if let Some(t) = ut.done_time {
                if t == f64::MAX {
                    ut.done_time = Some(now);
                } else if now - t > 1.5 {
                    self.state.upload_transfer = None;
                }
            }
        }

        // 顶部栏（40px）
        Panel::top("top_bar")
            .exact_size(40.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::RIO_BG)
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 2,
                        bottom: 8,
                    }),
            )
            .show_separator_line(false)
            .show(ui, |ui| {
                self.render_top_bar(ui);
            });

        // 上传传输侧栏（现代风格，左侧非模态，传输时可浏览其他页面）
        // 仅当 upload_transfer 存在时显示，SidePanel 在 CentralPanel 之前占据左侧空间
        if self.state.upload_transfer.is_some() {
            Panel::left("upload_sidebar")
                .exact_size(260.0)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(theme::RIO_CONTENT_BG)
                        .inner_margin(egui::Margin::same(8))
                        .corner_radius(6)
                        .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER)),
                )
                .show_separator_line(false)
                .show(ui, |ui| {
                    self.render_upload_transfer_sidebar(ui);
                });
        }

        // 内容区（pane 样式：白底 6px 圆角 1px 边框 12/16 padding）
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::RIO_BG)
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                let pane_frame = egui::Frame::new()
                    .fill(theme::RIO_CONTENT_BG)
                    .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 12,
                        bottom: 12,
                    })
                    .corner_radius(6);
                pane_frame.show(ui, |ui| {
                    self.render_content(ui, &ctx);
                });
            });

        // 播放器条（底部 48px，仅 audio 存在时）
        if self.state.audio.is_some() {
            Panel::bottom("player_bar")
                .exact_size(48.0)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(theme::RIO_CONTENT_BG)
                        .stroke(egui::Stroke::new(1.0, theme::RIO_BORDER))
                        .inner_margin(egui::Margin {
                            left: 16,
                            right: 16,
                            top: 0,
                            bottom: 0,
                        }),
                )
                .show_separator_line(false)
                .show(ui, |ui| {
                    self.render_player_bar(ui, &ctx);
                });
        }

        // 存储状态条（底部 26px）
        Panel::bottom("storage_bar")
            .exact_size(26.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::RIO_BG)
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 0,
                        bottom: 8,
                    }),
            )
            .show_separator_line(false)
            .show(ui, |ui| {
                self.render_storage_bar(ui);
            });

        // notice toast
        self.render_notice_toast(&ctx);
        // 二次确认对话框
        self.render_confirm_dialog(&ctx);
        // 加载遮罩（仅在非上传传输时显示）
        if self.state.upload_transfer.is_none() {
            self.render_loading_modal(&ctx);
        }
        // 调试窗口
        self.render_debug_window(&ctx);

        // 持续重绘
        ctx.request_repaint();
    }
}
