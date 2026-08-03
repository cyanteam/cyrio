//! USB Transport trait
//!
//! 抽象 USB 通信层，让 cyrio-core 不直接依赖 nusb / webusb。
//! 平台特定实现：
//! - 桌面（Windows/macOS/Linux）：`cyrio-transport-nusb` crate
//! - Web（WASM）：`cyrio-transport-webusb` crate
//! - Android：后续 JNI 桥接 UsbManager（待实现）
//!
//! ## 设计
//! Transport 是 async trait，所有 USB 操作返回 Future。
//! UI 层用 channel 把 Command 发到后台任务，后台任务持有 Transport 实例执行 USB 操作。

use async_trait::async_trait;

use crate::error::Result;

/// USB 控制传输的方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlDirection {
    /// 主机 → 设备（OUT）
    Out,
    /// 设备 → 主机（IN）
    In,
}

/// USB setup 包字段（对应 libusb 的 control setup）
#[derive(Debug, Clone, Copy)]
pub struct ControlSetup {
    /// bmRequestType 的高 5 位（请求类型：vendor/standard/class）
    pub request_type: u8,
    /// bRequest（操作码，如 0x00 RIO_INIT）
    pub request: u8,
    /// wValue
    pub value: u16,
    /// wIndex
    pub index: u16,
    /// wLength（数据长度）
    pub length: u16,
}

/// USB Transport 抽象
///
/// 实现者负责：
/// - 打开设备（构造函数）
/// - claim interface
/// - 提供 control/bulk transfer
/// - 关闭设备（Drop）
#[async_trait]
pub trait Transport: Send + Sync {
    /// 控制传输（OUT 方向）：主机 → 设备
    ///
    /// 对应 nusb `control_out` / webusb `controlTransferOut`
    async fn control_out(&self, setup: ControlSetup, data: &[u8]) -> Result<()>;

    /// 控制传输（IN 方向）：设备 → 主机
    ///
    /// 对应 nusb `control_in` / webusb `controlTransferIn`
    /// 返回读取的字节数据（长度由 setup.length 决定）
    async fn control_in(&self, setup: ControlSetup) -> Result<Vec<u8>>;

    /// Bulk 传输（OUT 方向）：主机 → 设备（端点方向 OUT）
    ///
    /// 对应 nusb `bulk_out` / webusb `transferOut`
    async fn bulk_out(&self, endpoint: u8, data: &[u8]) -> Result<()>;

    /// Bulk 传输（IN 方向）：设备 → 主机（端点方向 IN）
    ///
    /// 对应 nusb `bulk_in` / webusb `transferIn`
    /// `length` 是期望读取的最大字节数
    async fn bulk_in(&self, endpoint: u8, length: usize) -> Result<Vec<u8>>;

    /// 重置设备（可选实现，USB 重新枚举）
    async fn reset(&self) -> Result<()> {
        Err(crate::error::CyrioError::Other(
            "reset not supported".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock transport，用于测试 API 层逻辑
    #[allow(dead_code)]
    pub struct MockTransport {
        /// 记录所有调用日志
        pub logs: std::sync::Mutex<Vec<String>>,
    }

    impl MockTransport {
        /// 创建空 mock
        #[allow(dead_code)]
        pub fn new() -> Self {
            Self {
                logs: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn control_out(&self, setup: ControlSetup, data: &[u8]) -> Result<()> {
            self.logs.lock().unwrap().push(format!(
                "control_out(req=0x{:02x}, val={}, idx={}, len={})",
                setup.request,
                setup.value,
                setup.index,
                data.len()
            ));
            Ok(())
        }

        async fn control_in(&self, setup: ControlSetup) -> Result<Vec<u8>> {
            self.logs.lock().unwrap().push(format!(
                "control_in(req=0x{:02x}, val={}, idx={}, len={})",
                setup.request, setup.value, setup.index, setup.length
            ));
            Ok(vec![0; setup.length as usize])
        }

        async fn bulk_out(&self, endpoint: u8, data: &[u8]) -> Result<()> {
            self.logs.lock().unwrap().push(format!(
                "bulk_out(ep=0x{:02x}, len={})",
                endpoint,
                data.len()
            ));
            Ok(())
        }

        async fn bulk_in(&self, endpoint: u8, length: usize) -> Result<Vec<u8>> {
            self.logs
                .lock()
                .unwrap()
                .push(format!("bulk_in(ep=0x{:02x}, len={})", endpoint, length));
            Ok(vec![0; length])
        }
    }
}
