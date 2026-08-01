//! cyrio Tauri 2.0 应用库
//!
//! 提供 `run()` 入口，桌面端通过 main.rs 调用，移动端通过
//! `tauri::mobile_entry_point` 自动调用。
//!
//! 桌面端额外启用系统托盘；移动端使用底部选项卡导航。

use cyrio_tauri::audio_commands;
use cyrio_tauri::commands;
use cyrio_tauri::sync_commands;
use cyrio_tauri::webdav_server;
use cyrio_tauri::{start_smol_executor, DeviceState};
// 音频线程仅在桌面端启动（Android 上 cpal/oboe 可能导致 native panic）
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use cyrio_tauri::start_audio_thread;
use tauri::Manager;

/// Tauri 应用入口。
///
/// - 桌面端：由 `main.rs` 直接调用
/// - 移动端：由 `tauri::mobile_entry_point` 宏自动注册
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 日志初始化（移动端也安全）
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();

    // 启动 smol 全局执行器（cyrio-core 的 smol::Timer 需要）
    start_smol_executor();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DeviceState::new())
        .manage(webdav_server::WebDavState::new())
        .setup(|app| {
            // ── 桌面端：启动音频线程 ──────────────────────
            // Android 上跳过：cpal/oboe 初始化可能导致 native panic
            #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
            {
                app.manage(start_audio_thread());
            }
            // ── 桌面端：系统托盘 ──────────────────────────
            // 移动端无系统托盘概念，跳过
            #[cfg(desktop)]
            {
                use tauri::{
                    menu::{Menu, MenuItem},
                    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
                };

                let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "断开并退出", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

                let _tray = TrayIconBuilder::with_id("main")
                    .icon(app.default_window_icon().cloned().unwrap())
                    .menu(&menu)
                    .tooltip("cyrio - 未连接设备")
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // 桌面端：关闭窗口直接退出（不后台运行）
            #[cfg(desktop)]
            {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    let app = window.app_handle();
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // 设备
            commands::open_device,
            commands::close_device,
            commands::is_connected,
            commands::list_usb_devices,
            commands::open_device_force,
            // 列表
            commands::list_songs,
            commands::list_playlists,
            commands::list_playlist_songs,
            commands::get_storage,
            // 上传/下载/删除
            commands::upload_song,
            commands::upload_song_batch,
            commands::expand_paths,
            commands::download_song,
            commands::delete_song,
            // 歌单
            commands::create_playlist,
            commands::add_song_to_playlist,
            commands::repair_playlist_encoding,
            // 详细信息
            commands::get_song_detail,
            // 重命名 / 批量文本处理 / 编码修复
            commands::rename_song,
            commands::batch_slug_songs,
            commands::batch_strip_songs,
            commands::repair_song_encoding,
            commands::repair_all_songs_encoding,
            commands::repair_selected_encoding,
            commands::preview_repair_encoding,
            commands::batch_slug_all_songs,
            commands::batch_strip_all_songs,
            commands::preview_slug,
            commands::preview_strip,
            // 音频播放
            audio_commands::play_song,
            audio_commands::pause_audio,
            audio_commands::resume_audio,
            audio_commands::stop_audio,
            audio_commands::get_playback_state,
            // 同步
            sync_commands::list_sync_rules,
            sync_commands::add_sync_rule,
            sync_commands::delete_sync_rule,
            sync_commands::run_sync,
            // WebDAV 虚拟U盘
            webdav_server::start_webdav,
            webdav_server::stop_webdav,
            webdav_server::get_webdav_status,
            webdav_server::mount_webdav,
            // 系统托盘
            commands::update_tray_tooltip,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
