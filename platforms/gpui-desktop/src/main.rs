//! Cyrio GPUI 桌面前端 — 入口
//!
//! 使用 Zed 的 GPUI 引擎渲染原生 GPU 加速界面。
//! 架构参考 cyrio-app：后台 smol 任务处理 USB/IO，前台 GPUI 渲染。
//!
//! 窗口：无边框（appears_transparent + app_owns_titlebar_drag），自绘标题栏。
//! 1:1 复刻 Tauri 版：decorations:false + transparent:true + 自绘 TitleBar。

mod state;
mod task;
mod theme;
mod views;

use crate::state::CyrioApp;
use gpui::*;
use gpui_platform::application;

fn main() {
    env_logger::init();

    // 启动后台任务循环
    let (cmd_tx, event_rx) = task::spawn_task_loop();

    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1024.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // titlebar: None → macOS NSWindowStyleMaskBorderless
                // 完全去除系统标题栏 + 圆角，实现真正的无边框方形窗口
                // 窗口按钮（−/□/×）由自绘标题栏提供
                titlebar: None,
                // app_owns_titlebar_drag 允许通过 start_window_move() 拖拽窗口
                app_owns_titlebar_drag: true,
                is_movable: true,
                is_resizable: true,
                is_minimizable: true,
                window_min_size: Some(size(px(640.0), px(480.0))),
                ..Default::default()
            },
            |_window, cx| {
                cx.new(|_| CyrioApp::new(cmd_tx.clone(), event_rx.clone()))
            },
        )
        .unwrap();
        cx.activate(true);

        // 定时轮询后台事件 — 100ms 间隔，仅在有事件时触发重绘
        cx.spawn(async move |cx| {
            loop {
                let need_notify = cx.update(|cx| {
                    let mut any_changed = false;
                    for entity in cx.windows() {
                        let _ = entity.update(cx, |view, _window, cx| {
                            if let Ok(app) = view.downcast::<CyrioApp>() {
                                app.update(cx, |app, cx| {
                                    let changed = app.poll_events_quiet();
                                    if changed {
                                        any_changed = true;
                                    }
                                    // 仅在播放时请求播放状态
                                    if app.current_playing_file_no.is_some() || app.playback.is_playing {
                                        app.send_cmd(task::Command::GetPlaybackState);
                                    }
                                });
                            }
                        });
                    }
                    any_changed
                });

                if need_notify {
                    cx.update(|cx| {
                        for entity in cx.windows() {
                            let _ = entity.update(cx, |view, _window, cx| {
                                if let Ok(app) = view.downcast::<CyrioApp>() {
                                    app.update(cx, |_, cx| {
                                        cx.notify();
                                    });
                                }
                            });
                        }
                    });
                }

                cx.background_executor().timer(std::time::Duration::from_millis(100)).await;
            }
        }).detach();
    });
}
