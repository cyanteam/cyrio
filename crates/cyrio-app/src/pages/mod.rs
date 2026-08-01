//! 页面模块
//!
//! 每个页面是一个函数 `render(ui, state, cmd_tx)`，由 `app.rs` 根据当前
//! `page_path` 调度。页面无内部状态，所有状态在 `AppState` 集中管理。

pub mod about;
pub mod connect;
pub mod device;
pub mod playlists;
pub mod settings;
pub mod songs;
pub mod sync;
pub mod upload;
