//! UI 视图模块 — 1:1 复刻 Tauri 版 CyrioLauncher
//!
//! 布局结构（对齐 Tauri CSS + cyrio-app egui）：
//! - 标题栏 28px（teal #39c5bb + 3 个窗口按钮）
//! - 顶栏 40px（← back | 虚拟U盘 | menu-bar flex:1 | 分页切换）
//! - 传输侧栏 260px（仅上传传输时显示，左侧非模态）
//! - 内容区（白底 6px 圆角 1px 边框，pane 样式）
//! - 播放器条 48px（info | ▶/⏸ | ⏹ | time | progress | time | ×）
//! - 存储状态条 26px（内置蓝/SD橙 mini-bar 3px）

pub mod about;
pub mod connect;
pub mod device_info;
pub mod main_layout;
pub mod player;
pub mod playlists;
pub mod songs;
pub mod sync;
pub mod transfer;
pub mod upload;
pub mod settings;

use crate::state::CyrioApp;
use gpui::*;

impl Render for CyrioApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.connected {
            // 未连接：显示连接页面
            connect::render_connect_scene(self, window, cx).into_any_element()
        } else {
            // 已连接：显示主布局
            main_layout::render_main_layout(self, window, cx).into_any_element()
        }
    }
}
