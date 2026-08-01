//! # cyrio-app
//!
//! egui 应用层，桌面与 Web 共用同一份代码。
//!
//! ## 架构：消息传递解耦
//! - UI 线程持有 `Sender<Command>` + `Receiver<Event>`
//! - 后台任务（smol runtime）持有 `Receiver<Command>` + `Sender<Event>`
//! - UI 每帧 `try_recv` 检查事件，USB/文件 IO 永不阻塞 UI
//!
//! ## 全局状态
//! - [`state::AppState`]：含 `page_path: String`（WASM 与 hash 同步）
//! - [`message::Command`] / [`message::Event`]：UI ↔ 后台消息
//! - Alt+Shift+D：弹出调试窗口
//!
//! ## 布局（Phase 4 新方案）
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │ 顶部栏: [cyrio] [设备选择▼] [内置/SD切换] [存储状态]    │
//! ├─────────────────────────────────────────────────────┤
//! │ 选项卡栏: [歌曲] [歌单] [上传] [设备信息]              │
//! ├─────────────────────────────────────────────────────┤
//! │                                                     │
//! │ 内容区（根据当前选项卡渲染）                            │
//! │                                                     │
//! └─────────────────────────────────────────────────────┘
//! ```
//! 不再使用左侧侧栏，选项卡横排在内容区顶部，与原厂 Rio 软件一致。

pub mod app;
pub mod fonts;
pub mod message;
pub mod pages;
pub mod state;
pub mod task;
pub mod theme;

pub use app::CyrioApp;
