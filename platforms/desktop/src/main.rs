//! cyrio 桌面入口
//!
//! 启动 eframe native window，加载 [`cyrio_app::CyrioApp`]。
//! Phase 4 起接入后台任务与 USB transport。
//!
//! ## smol 执行器
//! `cyrio_app::task::spawn_task_loop()` 用 `smol::spawn()` 把后台任务投到全局执行器。
//! 但 smol 的全局执行器需要至少一个 `block_on()` 在跑才会被驱动。
//! 这里在独立线程上跑 `smol::block_on(pending)`，让全局执行器常驻后台。

use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 启动 smol 全局执行器（后台线程常驻驱动）
    // 没有这个，smol::spawn() 的任务永远不会执行
    std::thread::Builder::new()
        .name("smol-executor".into())
        .spawn(|| {
            smol::block_on(smol::future::pending::<()>());
        })
        .expect("spawn smol executor thread");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("cyrio - Diamond Rio S-Series Manager"),
        ..Default::default()
    };

    eframe::run_native(
        "cyrio",
        options,
        Box::new(|_cc| Ok(Box::new(cyrio_app::CyrioApp::default()))),
    )
}
