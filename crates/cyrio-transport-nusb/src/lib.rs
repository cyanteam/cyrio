//! # cyrio-transport-nusb
//!
//! USB transport 层。
//!
//! - **桌面端** (Windows/macOS/Linux)：基于 [`nusb`]（pure Rust，无 C 依赖）
//! - **Android**：stub 实现，USB 功能暂不可用（需通过 Android USB Host API 实现）
//!
//! ## 设计
//! - [`NusbTransport`] 持有 `nusb::Device` + `nusb::Interface` + 两个 bulk endpoint
//! - bulk endpoint 用 `smol::lock::Mutex` 包装，因为 `Endpoint::submit` 需要 `&mut`
//! - 控制传输直接走 `interface.control_in/out`（不需要 endpoint 句柄）
//!
//! ## smol 集成
//! nusb 0.2 自带异步运行时支持，可直接在 smol executor 上 await。
//! 无需 tokio。

#![warn(missing_docs)]

/// USB 设备简略信息（用于前端列出所有设备供用户强制选择）
#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    /// 厂商 ID
    pub vid: u16,
    /// 产品 ID
    pub pid: u16,
    /// 产品名称（可选，部分设备不提供）
    pub name: String,
    /// 厂商名称（可选）
    pub manufacturer: String,
    /// 序列号（可选）
    pub serial: String,
}

// ============================================================================
// 桌面端实现 — 基于 nusb
// ============================================================================

#[cfg(not(target_os = "android"))]
mod desktop {
    use super::UsbDeviceInfo;
    use cyrio_core::error::{CyrioError, Result};
    use cyrio_core::transport::{ControlSetup, Transport};
    use nusb::transfer::{Buffer, Bulk, Completion, ControlIn, ControlOut, ControlType, In, Out};
    use nusb::{Device, Endpoint, Interface};
    use smol::lock::Mutex;
    use std::time::Duration;

    use cyrio_core::protocol::constants::{
        BULK_TIMEOUT_MS, CONTROL_TIMEOUT_MS, EP_IN, EP_OUT, USB_INTERFACE,
    };

    /// nusb 实现的 USB transport
    pub struct NusbTransport {
        /// 设备句柄（保活用，避免 drop 释放设备）
        _device: Device,
        /// USB 接口（用于 control transfer）
        interface: Interface,
        /// Bulk IN 端点（Mutex 因为 submit 需要 &mut）
        ep_in: Mutex<Endpoint<Bulk, In>>,
        /// Bulk OUT 端点
        ep_out: Mutex<Endpoint<Bulk, Out>>,
    }

    impl NusbTransport {
        /// 打开 Rio S-Series 设备
        ///
        /// 自动扫描 `cyrio_core::protocol::constants::SUPPORTED_PIDS`，找到第一个匹配的设备。
        pub async fn open() -> Result<Self> {
            let device_info = find_rio_device().await?;
            Self::open_from_device_info(device_info).await
        }

        /// 强制以指定 VID/PID 打开任意 USB 设备作为 Rio 设备
        pub async fn open_with_vid_pid(vid: u16, pid: u16) -> Result<Self> {
            let device_info = find_device_by_vid_pid(vid, pid).await?;
            Self::open_from_device_info(device_info).await
        }

        /// 从 `nusb::DeviceInfo` 打开设备并构造 transport
        async fn open_from_device_info(device_info: nusb::DeviceInfo) -> Result<Self> {
            let device = device_info
                .open()
                .await
                .map_err(|e| CyrioError::Transport(format!("open device: {}", e)))?;

            let _ = device.set_configuration(1).await;

            let interface = {
                let mut last_err: Option<String> = None;
                let mut claimed: Option<Interface> = None;
                for attempt in 0..5u32 {
                    match device.claim_interface(USB_INTERFACE).await {
                        Ok(itf) => {
                            claimed = Some(itf);
                            break;
                        }
                        Err(e) => {
                            last_err = Some(format!("claim interface (attempt {}): {}", attempt + 1, e));
                            smol::Timer::after(Duration::from_millis(250)).await;
                        }
                    }
                }
                claimed.ok_or_else(|| {
                    CyrioError::Transport(last_err.unwrap_or_else(|| "claim interface: unknown".into()))
                })?
            };

            let ep_in = interface
                .endpoint::<Bulk, In>(EP_IN)
                .map_err(|e| CyrioError::Transport(format!("get ep_in 0x{:02x}: {}", EP_IN, e)))?;
            let ep_out = interface
                .endpoint::<Bulk, Out>(EP_OUT)
                .map_err(|e| CyrioError::Transport(format!("get ep_out 0x{:02x}: {}", EP_OUT, e)))?;

            Ok(Self {
                _device: device,
                interface,
                ep_in: Mutex::new(ep_in),
                ep_out: Mutex::new(ep_out),
            })
        }
    }

    /// 列出系统中所有 USB 设备
    pub async fn list_all_usb_devices() -> Result<Vec<UsbDeviceInfo>> {
        let devices = nusb::list_devices()
            .await
            .map_err(|e| CyrioError::Transport(format!("list_devices: {}", e)))?;

        let result: Vec<UsbDeviceInfo> = devices
            .map(|d| UsbDeviceInfo {
                vid: d.vendor_id(),
                pid: d.product_id(),
                name: d.product_string().unwrap_or("").to_string(),
                manufacturer: d.manufacturer_string().unwrap_or("").to_string(),
                serial: d.serial_number().unwrap_or("").to_string(),
            })
            .collect();

        Ok(result)
    }

    #[async_trait::async_trait]
    impl Transport for NusbTransport {
        async fn control_out(&self, setup: ControlSetup, data: &[u8]) -> Result<()> {
            let ctrl = ControlOut {
                control_type: ControlType::Vendor,
                recipient: nusb::transfer::Recipient::Device,
                request: setup.request,
                value: setup.value,
                index: setup.index,
                data,
            };
            self.interface
                .control_out(ctrl, Duration::from_millis(CONTROL_TIMEOUT_MS))
                .await
                .map_err(|e| {
                    CyrioError::Transport(format!("control_out(req=0x{:02x}): {}", setup.request, e))
                })
        }

        async fn control_in(&self, setup: ControlSetup) -> Result<Vec<u8>> {
            let ctrl = ControlIn {
                control_type: ControlType::Vendor,
                recipient: nusb::transfer::Recipient::Device,
                request: setup.request,
                value: setup.value,
                index: setup.index,
                length: setup.length,
            };
            self.interface
                .control_in(ctrl, Duration::from_millis(CONTROL_TIMEOUT_MS))
                .await
                .map_err(|e| {
                    CyrioError::Transport(format!("control_in(req=0x{:02x}): {}", setup.request, e))
                })
        }

        async fn bulk_out(&self, _endpoint: u8, data: &[u8]) -> Result<()> {
            let mut ep = self.ep_out.lock().await;
            let mut buf = Buffer::new(data.len());
            buf.extend_from_slice(data);
            ep.submit(buf);
            let completion: Completion = ep.next_complete().await;
            completion
                .into_result()
                .map_err(|e| CyrioError::Transport(format!("bulk_out: {}", e)))?;
            Ok(())
        }

        async fn bulk_in(&self, _endpoint: u8, length: usize) -> Result<Vec<u8>> {
            let mut ep = self.ep_in.lock().await;
            let buf = Buffer::new(length);
            ep.submit(buf);
            let completion: Completion = ep.next_complete().await;
            let buf = completion
                .into_result()
                .map_err(|e| CyrioError::Transport(format!("bulk_in: {}", e)))?;
            Ok(buf.into_vec())
        }

        async fn reset(&self) -> Result<()> {
            self.interface
                .set_alt_setting(0)
                .await
                .map_err(|e| CyrioError::Transport(format!("reset (set_alt_setting): {}", e)))
        }
    }

    /// 扫描 USB 总线，找到 Rio S-Series 设备
    async fn find_rio_device() -> Result<nusb::DeviceInfo> {
        use cyrio_core::protocol::constants::{SUPPORTED_PIDS, VENDOR_DIAMOND};

        let devices = nusb::list_devices()
            .await
            .map_err(|e| CyrioError::Transport(format!("list_devices: {}", e)))?;

        let mut matches: Vec<nusb::DeviceInfo> = devices
            .filter(|d| d.vendor_id() == VENDOR_DIAMOND && SUPPORTED_PIDS.contains(&d.product_id()))
            .collect();

        if matches.is_empty() {
            return Err(CyrioError::Device(format!(
                "no Rio S-Series device found (vid=0x{:04x}, pids={:?})",
                VENDOR_DIAMOND, SUPPORTED_PIDS
            )));
        }

        Ok(matches.remove(0))
    }

    /// 按 VID/PID 查找 USB 设备
    async fn find_device_by_vid_pid(vid: u16, pid: u16) -> Result<nusb::DeviceInfo> {
        let devices = nusb::list_devices()
            .await
            .map_err(|e| CyrioError::Transport(format!("list_devices: {}", e)))?;

        let mut matches: Vec<nusb::DeviceInfo> = devices
            .filter(|d| d.vendor_id() == vid && d.product_id() == pid)
            .collect();

        if matches.is_empty() {
            return Err(CyrioError::Device(format!(
                "no USB device found (vid=0x{:04x}, pid=0x{:04x})",
                vid, pid
            )));
        }

        Ok(matches.remove(0))
    }

    #[allow(dead_code)]
    const _: () = {
        let _ = BULK_TIMEOUT_MS;
    };
}

// ============================================================================
// Android 实现 — 通过 JNI 桥接 Android USB Host API (UsbManager)
// ============================================================================

#[cfg(target_os = "android")]
mod android {
    use super::UsbDeviceInfo;
    use cyrio_core::error::{CyrioError, Result};
    use cyrio_core::protocol::constants::{SUPPORTED_PIDS, VENDOR_DIAMOND};
    use cyrio_core::transport::{ControlSetup, Transport};
    use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
    use jni::{AttachGuard, JNIEnv, JavaVM};
    use std::sync::OnceLock;

    /// 全局 JavaVM 引用（在 nativeInit 时设置）
    static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

    /// 全局 CyrioUsbHelper 类引用（在 nativeInit 时缓存）
    ///
    /// Android JNI 的 FindClass 在后台线程使用系统类加载器，
    /// 找不到应用类（ClassNotFoundException）。
    /// 在主线程 nativeInit 时缓存 GlobalRef，后台线程通过 new_local_ref 使用。
    static HELPER_CLASS_REF: OnceLock<GlobalRef> = OnceLock::new();

    /// JNI 初始化入口 — 由 Kotlin CyrioUsbHelper.nativeInit() 在主线程调用
    ///
    /// 保存 JavaVM 指针和 CyrioUsbHelper 的全局类引用。
    /// 后续后台线程通过 JavaVM.attach_current_thread() 获取 JNIEnv，
    /// 通过全局类引用创建局部引用使用。
    #[no_mangle]
    pub extern "system" fn Java_c_cyrio_android_usb_CyrioUsbHelper_nativeInit(
        mut env: JNIEnv,
        class: JClass,
    ) {
        if let Ok(jvm) = env.get_java_vm() {
            let _ = JAVA_VM.set(jvm);
        } else {
            log::error!("Android USB transport: failed to get JavaVM");
        }
        match env.new_global_ref(class) {
            Ok(global_ref) => {
                let _ = HELPER_CLASS_REF.set(global_ref);
                log::info!("Android USB transport: JavaVM + class ref initialized");
            }
            Err(e) => {
                log::error!("Android USB transport: new_global_ref failed: {}", e);
            }
        }
    }

    /// 获取 JNIEnv（attach 当前线程到 JVM）
    ///
    /// 每次传输操作都调用此函数获取独立的 JNIEnv。
    /// AttachGuard 在 drop 时自动 detach 线程（非主线程）。
    fn get_env() -> Result<AttachGuard<'static>> {
        let jvm = JAVA_VM
            .get()
            .ok_or_else(|| CyrioError::Transport("JavaVM not initialized".into()))?;
        jvm.attach_current_thread()
            .map_err(|e| CyrioError::Transport(format!("attach_current_thread: {}", e)))
    }

    /// 获取 CyrioUsbHelper 的局部类引用
    ///
    /// 从主线程缓存的全局引用创建当前线程的局部引用。
    /// 解决后台线程 FindClass 找不到应用类的问题（Android JNI 限制）。
    fn find_helper_class<'local>(env: &mut JNIEnv<'local>) -> Result<JClass<'local>> {
        let global_ref = HELPER_CLASS_REF.get().ok_or_else(|| {
            CyrioError::Transport("CyrioUsbHelper class not initialized (nativeInit not called)".into())
        })?;
        let local_obj: JObject<'local> = env
            .new_local_ref(global_ref.as_obj())
            .map_err(|e| CyrioError::Transport(format!("new_local_ref for helper class: {}", e)))?;
        Ok(JClass::from(local_obj))
    }

    // ------------------------------------------------------------------
    // Transport struct
    // ------------------------------------------------------------------

    /// Android USB transport
    ///
    /// 所有 USB 状态（UsbDeviceConnection、UsbEndpoint）都在 Kotlin 侧的
    /// CyrioUsbHelper 单例中管理。Rust 侧通过 JNI 调用其静态方法。
    ///
    /// 空结构体满足 `Send + Sync` 约束（Transport trait 要求）。
    pub struct NusbTransport;

    impl NusbTransport {
        /// 打开 Rio S-Series 设备
        ///
        /// 通过 JNI 调用 CyrioUsbHelper.openDevice(vid, pid)：
        /// 1. 枚举 USB 设备找到匹配的 VID/PID
        /// 2. 请求 USB 权限（弹系统对话框，用户授权后继续）
        /// 3. 打开设备、claim interface、获取 bulk endpoints
        pub async fn open() -> Result<Self> {
            smol::unblock(move || {
                let result = call_static_bool("openDevice", "(II)Z", &[
                    JValue::Int(VENDOR_DIAMOND as i32),
                    JValue::Int(0), // pid=0 表示接受任何 PID
                ])?;
                if result {
                    Ok(Self)
                } else {
                    Err(CyrioError::Device(
                        "openDevice returned false (permission denied or device not found)".into(),
                    ))
                }
            })
            .await
        }

        /// 强制以指定 VID/PID 打开设备
        pub async fn open_with_vid_pid(vid: u16, pid: u16) -> Result<Self> {
            smol::unblock(move || {
                let result = call_static_bool("openDevice", "(II)Z", &[
                    JValue::Int(vid as i32),
                    JValue::Int(pid as i32),
                ])?;
                if result {
                    Ok(Self)
                } else {
                    Err(CyrioError::Device(format!(
                        "openDevice(vid=0x{:04x}, pid=0x{:04x}) returned false",
                        vid, pid
                    )))
                }
            })
            .await
        }
    }

    /// 列出所有已连接的 USB 设备
    ///
    /// Kotlin 侧返回 JSON 数组字符串，Rust 侧解析为 Vec<UsbDeviceInfo>
    pub async fn list_all_usb_devices() -> Result<Vec<UsbDeviceInfo>> {
        smol::unblock(move || {
            let json = call_static_string("listDevices", "()Ljava/lang/String;", &[])?;
            parse_device_json(&json)
        })
        .await
    }

    // ------------------------------------------------------------------
    // JNI 调用辅助函数
    // ------------------------------------------------------------------

    /// 调用 CyrioUsbHelper 的静态方法，返回 bool
    fn call_static_bool(name: &str, sig: &str, args: &[JValue]) -> Result<bool> {
        let mut env = get_env()?;
        let class = find_helper_class(&mut env)?;
        let result = env
            .call_static_method(&class, name, sig, args)
            .map_err(|e| CyrioError::Transport(format!("call_static_method {}: {}", name, e)))?;
        result
            .z()
            .map_err(|e| CyrioError::Transport(format!("{}.z(): {}", name, e)))
    }

    /// 调用 CyrioUsbHelper 的静态方法，返回 String
    fn call_static_string(name: &str, sig: &str, args: &[JValue]) -> Result<String> {
        let mut env = get_env()?;
        let class = find_helper_class(&mut env)?;
        let result = env
            .call_static_method(&class, name, sig, args)
            .map_err(|e| CyrioError::Transport(format!("call_static_method {}: {}", name, e)))?;
        let obj = result
            .l()
            .map_err(|e| CyrioError::Transport(format!("{}.l(): {}", name, e)))?;
        let jstr: JString = obj.into();
        let java_str = env.get_string(&jstr)
            .map_err(|e| CyrioError::Transport(format!("get_string: {}", e)))?;
        java_str.to_str()
            .map(|s| s.to_string())
            .map_err(|e| CyrioError::Transport(format!("to_str: {}", e)))
    }

    /// 调用 CyrioUsbHelper 的静态方法，返回 byte[]
    fn call_static_byte_array(name: &str, sig: &str, args: &[JValue]) -> Result<Vec<u8>> {
        let mut env = get_env()?;
        let class = find_helper_class(&mut env)?;
        let result = env
            .call_static_method(&class, name, sig, args)
            .map_err(|e| CyrioError::Transport(format!("call_static_method {}: {}", name, e)))?;
        let obj = result
            .l()
            .map_err(|e| CyrioError::Transport(format!("{}.l(): {}", name, e)))?;
        let byte_array: JByteArray = obj.into();
        env.convert_byte_array(&byte_array)
            .map_err(|e| CyrioError::Transport(format!("convert_byte_array: {}", e)))
    }

    /// 调用 CyrioUsbHelper 的静态方法，返回 int
    #[allow(dead_code)]
    fn call_static_int(name: &str, sig: &str, args: &[JValue]) -> Result<i32> {
        let mut env = get_env()?;
        let class = find_helper_class(&mut env)?;
        let result = env
            .call_static_method(&class, name, sig, args)
            .map_err(|e| CyrioError::Transport(format!("call_static_method {}: {}", name, e)))?;
        result
            .i()
            .map_err(|e| CyrioError::Transport(format!("{}.i(): {}", name, e)))
    }

    // ------------------------------------------------------------------
    // JSON 解析
    // ------------------------------------------------------------------

    /// 解析 Kotlin listDevices() 返回的 JSON 数组
    fn parse_device_json(json: &str) -> Result<Vec<UsbDeviceInfo>> {
        if json.trim() == "[]" || json.trim().is_empty() {
            return Ok(Vec::new());
        }
        // 简易 JSON 解析（避免引入 serde 依赖）
        let mut devices = Vec::new();
        // 按对象分割 {"vid":...}
        for obj_str in extract_json_objects(json) {
            let vid = extract_json_int(&obj_str, "vid").unwrap_or(0) as u16;
            let pid = extract_json_int(&obj_str, "pid").unwrap_or(0) as u16;
            let name = extract_json_string(&obj_str, "name").unwrap_or_default();
            let manufacturer = extract_json_string(&obj_str, "manufacturer").unwrap_or_default();
            let serial = extract_json_string(&obj_str, "serial").unwrap_or_default();
            devices.push(UsbDeviceInfo {
                vid,
                pid,
                name,
                manufacturer,
                serial,
            });
        }
        Ok(devices)
    }

    /// 从 JSON 字符串中提取所有 {} 对象
    fn extract_json_objects(json: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut depth = 0i32;
        let mut start = 0;
        for (i, ch) in json.char_indices() {
            match ch {
                '{' => {
                    if depth == 0 {
                        start = i;
                    }
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        result.push(json[start..=i].to_string());
                    }
                }
                _ => {}
            }
        }
        result
    }

    /// 从 JSON 对象字符串中提取整数字段
    fn extract_json_int(obj: &str, key: &str) -> Option<i64> {
        let pattern = format!("\"{}\":", key);
        let pos = obj.find(&pattern)?;
        let rest = &obj[pos + pattern.len()..];
        let num_str: String = rest.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
        num_str.parse().ok()
    }

    /// 从 JSON 对象字符串中提取字符串字段
    fn extract_json_string(obj: &str, key: &str) -> Option<String> {
        let pattern = format!("\"{}\":\"", key);
        let pos = obj.find(&pattern)?;
        let rest = &obj[pos + pattern.len()..];
        let mut result = String::new();
        let mut escaped = false;
        for ch in rest.chars() {
            if escaped {
                result.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                break;
            } else {
                result.push(ch);
            }
        }
        Some(result)
    }

    // ------------------------------------------------------------------
    // Transport trait 实现
    // ------------------------------------------------------------------

    #[async_trait::async_trait]
    impl Transport for NusbTransport {
        /// 控制传输 OUT：调用 CyrioUsbHelper.controlTransferOut(request, value, index, data)
        async fn control_out(&self, setup: ControlSetup, data: &[u8]) -> Result<()> {
            let request = setup.request as i32;
            let value = setup.value as i32;
            let index = setup.index as i32;
            let data = data.to_vec();
            smol::unblock(move || {
                let mut env = get_env()?;
                let class = find_helper_class(&mut env)?;
                let byte_array = env
                    .byte_array_from_slice(&data)
                    .map_err(|e| CyrioError::Transport(format!("byte_array_from_slice: {}", e)))?;
                let n = env
                    .call_static_method(
                        &class,
                        "controlTransferOut",
                        "(III[B)I",
                        &[
                            JValue::Int(request),
                            JValue::Int(value),
                            JValue::Int(index),
                            JValue::Object(&byte_array),
                        ],
                    )
                    .map_err(|e| {
                        CyrioError::Transport(format!("controlTransferOut: {}", e))
                    })?
                    .i()
                    .map_err(|e| CyrioError::Transport(format!("i(): {}", e)))?;
                if n < 0 {
                    Err(CyrioError::Transport(format!(
                        "controlTransferOut failed (n={})", n
                    )))
                } else {
                    Ok(())
                }
            })
            .await
        }

        /// 控制传输 IN：调用 CyrioUsbHelper.controlTransferIn(request, value, index, length)
        async fn control_in(&self, setup: ControlSetup) -> Result<Vec<u8>> {
            let request = setup.request as i32;
            let value = setup.value as i32;
            let index = setup.index as i32;
            let length = setup.length as i32;
            smol::unblock(move || {
                call_static_byte_array(
                    "controlTransferIn",
                    "(IIII)[B",
                    &[
                        JValue::Int(request),
                        JValue::Int(value),
                        JValue::Int(index),
                        JValue::Int(length),
                    ],
                )
            })
            .await
        }

        /// Bulk 传输 OUT：调用 CyrioUsbHelper.bulkTransferOut(data)
        async fn bulk_out(&self, _endpoint: u8, data: &[u8]) -> Result<()> {
            let data = data.to_vec();
            smol::unblock(move || {
                let mut env = get_env()?;
                let class = find_helper_class(&mut env)?;
                let byte_array = env
                    .byte_array_from_slice(&data)
                    .map_err(|e| CyrioError::Transport(format!("byte_array_from_slice: {}", e)))?;
                let n = env
                    .call_static_method(
                        &class,
                        "bulkTransferOut",
                        "([B)I",
                        &[JValue::Object(&byte_array)],
                    )
                    .map_err(|e| CyrioError::Transport(format!("bulkTransferOut: {}", e)))?
                    .i()
                    .map_err(|e| CyrioError::Transport(format!("i(): {}", e)))?;
                if n < 0 {
                    Err(CyrioError::Transport(format!(
                        "bulkTransferOut failed (n={})", n
                    )))
                } else {
                    Ok(())
                }
            })
            .await
        }

        /// Bulk 传输 IN：调用 CyrioUsbHelper.bulkTransferIn(length)
        async fn bulk_in(&self, _endpoint: u8, length: usize) -> Result<Vec<u8>> {
            let length = length as i32;
            smol::unblock(move || {
                call_static_byte_array("bulkTransferIn", "(I)[B", &[JValue::Int(length)])
            })
            .await
        }

        /// 重置设备：调用 CyrioUsbHelper.closeDevice() + openDevice() 重连
        async fn reset(&self) -> Result<()> {
            smol::unblock(move || {
                call_static_bool("resetDevice", "()Z", &[])?;
                Ok(())
            })
            .await
        }
    }

    /// Drop 时关闭设备连接
    impl Drop for NusbTransport {
        fn drop(&mut self) {
            if JAVA_VM.get().is_some() {
                if let Ok(mut env) = get_env() {
                    if let Ok(class) = find_helper_class(&mut env) {
                        let _ = env.call_static_method(&class, "closeDevice", "()V", &[]);
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    const _: () = {
        let _ = SUPPORTED_PIDS;
    };
}

// 公共 re-export
#[cfg(not(target_os = "android"))]
pub use desktop::{list_all_usb_devices, NusbTransport};

#[cfg(target_os = "android")]
pub use android::{list_all_usb_devices, NusbTransport};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "android"))]
    #[test]
    fn transport_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NusbTransport>();
    }
}
