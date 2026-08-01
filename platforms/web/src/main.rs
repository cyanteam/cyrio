//! cyrio Web (WASM) 入口
//!
//! 启动 eframe WebRunner，加载 [`cyrio_app::CyrioApp`]。
//! Phase 7 实现 hash 路由同步与 WebUSB 集成。

use eframe::wasm_bindgen::{self, prelude::*};

#[wasm_bindgen]
pub async fn start(canvas_id: String) -> Result<(), JsValue> {
    let _ = console_log::init_with_level(log::Level::Info);

    eframe::WebRunner::new()
        .start(
            canvas_id,
            eframe::WebOptions::default(),
            Box::new(|_cc| Ok(Box::new(cyrio_app::CyrioApp::default()))),
        )
        .await?;

    Ok(())
}
