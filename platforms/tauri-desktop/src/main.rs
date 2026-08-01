//! cyrio Tauri 2.0 桌面二进制入口
//!
//! 仅调用 `cyrio_tauri_app::run()`，实际逻辑在 lib.rs 中。
//! 移动端不需要此文件——Tauri 通过 `mobile_entry_point` 自动调用 lib。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    cyrio_tauri_app::run();
}
