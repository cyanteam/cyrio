//! # cyrio-transport-webusb
//!
//! Web 端 USB transport，基于 [`webusb-web`]（WebUSB API 的 wasm-bindgen 绑定）。
//!
//! 平台：WASM（仅 Chrome/Edge 支持 WebUSB）
//! Phase 7 完整实现。Phase 1 仅占位。

#![warn(missing_docs)]

use async_trait::async_trait;
use cyrio_core::error::{CyrioError, Result};
use cyrio_core::transport::{ControlSetup, Transport};

/// WebUSB 实现的 USB transport
pub struct WebUsbTransport {
    // TODO Phase 7: 持有 webusb_web::Device
    _private: (),
}

impl WebUsbTransport {
    /// 请求用户选择 Rio 设备（弹出浏览器权限对话框）
    pub async fn request_device() -> Result<Self> {
        Err(CyrioError::Other("Phase 7 TODO: WebUsbTransport::request_device".into()))
    }
}

#[async_trait]
impl Transport for WebUsbTransport {
    async fn control_out(&self, _setup: ControlSetup, _data: &[u8]) -> Result<()> {
        Err(CyrioError::Other("Phase 7 TODO".into()))
    }

    async fn control_in(&self, _setup: ControlSetup) -> Result<Vec<u8>> {
        Err(CyrioError::Other("Phase 7 TODO".into()))
    }

    async fn bulk_out(&self, _endpoint: u8, _data: &[u8]) -> Result<()> {
        Err(CyrioError::Other("Phase 7 TODO".into()))
    }

    async fn bulk_in(&self, _endpoint: u8, _length: usize) -> Result<Vec<u8>> {
        Err(CyrioError::Other("Phase 7 TODO".into()))
    }
}
