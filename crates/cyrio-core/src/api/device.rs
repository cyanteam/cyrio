//! RioDevice：设备连接 + 协议初始化 + 完整读写 API
//!
//! 包装 [`crate::transport::Transport`]，提供高层方法。
//!
//! ## 实现状态
//! - `open()`：init 序列（7 步，PROTOCOL.md §6）
//! - `send_command()`：带重试，状态字节校验
//! - `get_memory_info()`：读 rio_mem_t（256B）
//! - `get_file_info()`：读 rio_file_t（2048B），slot index 查询
//! - `find_file_info_by_file_no()`：通过 slot 迭代查找真实 file_no
//! - `list_files()`：列举内存单元所有文件
//! - `upload_file()`：上传新文件（OP_RIO_WRITE 0x6c）
//! - `overwrite_file()`：覆盖已存在文件（OP_RIO_OVWRT 0x88，S-Series 播放列表修改）
//! - `download_file()`：下载文件（OP_RIO_READF 0x70，含 header_buffer 保留）
//! - `delete_file()`：删除文件（OP_RIO_DELET 0x78）
//! - `abort_transfer()`：发 CRIOABRT 中止包
//!
//! ## 关键陷阱
//! 1. init 序列中 0x61/0x65 的 status[0] 不稳定，**不**走 send_command 校验，
//!    直接 control_in 绕过校验
//! 2. RIO_FILEI 的 wIndex 是 0-based slot index，不是 rio_file_t.file_no
//! 3. file_no=0 表示空槽
//! 4. upload/overwrite 的 CRIOINFO 包无 CRC（4 字节字段必须为 0）
//! 5. delete 不前置 CRIODATA，直接写裸 2048B
//! 6. upload 末尾的 0x60 收尾不能漏
//! 7. download 的 fileNo 写在 2048B 头的 OFF_FILE_NO 字段
//! 8. 任何异常：尽力发 64B "CRIOABRT" 中止包，再抛原始错误
//! 9. S-Series 播放列表覆盖时必须保留原始 2048B 头中的未知字段（unk1[4] 等）

use std::time::Duration;

use crate::error::{CyrioError, Result};
use crate::protocol::constants::{
    COMMAND_MAX_RETRIES, COMMAND_RETRY_DELAY_MS, COMMAND_SUCCESS_BYTE, CONTROL_STATUS_LENGTH,
    EP_IN, EP_OUT, FILE_NO_MAX, FILE_NO_MIN, LIST_FILES_EMPTY_GAP, MAGIC_SRIONOFL, MAGIC_SRIODATA,
    MAGIC_SRIODELD, MAGIC_SRIODELS, MAGIC_SRIODONE, MAGIC_SRIORDY, OP_RIO_DELET, OP_RIO_FILEI,
    OP_RIO_MEMRI, OP_RIO_OVWRT, OP_RIO_POLLD, OP_RIO_READF, OP_RIO_TIMES, OP_RIO_TYPEQ,
    OP_RIO_WRITE, OP_UNKNOWN00, OP_UNKNOWN65, PKT_BLOCK, PKT_HANDSHAKE, PKT_HEADER, RIO_FILE_SIZE,
    RIO_MEM_SIZE,
};
use crate::protocol::packets::{
    build_crio_abort, build_crio_data, build_crio_info, expect_magic, parse_magic,
};
use crate::protocol::rio_file::{
    overwrite_rio_file_fields, parse_rio_file, serialize_rio_file, RioFile, RioFileUpdates,
};
use crate::protocol::rio_mem::{parse_rio_mem, RioMem};
use crate::transport::{ControlSetup, Transport};

/// 上传/下载进度回调参数
#[derive(Debug, Clone, Copy)]
pub struct UploadProgress {
    /// 已传输字节数（实际有效字节，不含补零）
    pub transferred: u32,
    /// 总字节数
    pub total: u32,
}

/// `download_file` 的返回结果
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// 文件元数据（rio_file_t）
    pub header: RioFile,
    /// 原始 2048B header buffer（保留所有未知字段，用于 `overwrite_file` 时回传）
    pub header_buffer: Box<[u8; RIO_FILE_SIZE]>,
    /// 文件内容（纯音频字节，长度 == header.size）
    pub data: Vec<u8>,
}

/// Diamond Rio S-Series 设备
///
/// 持有 Transport 实例，提供高层 API（list_songs、upload_song 等）。
/// 构造后需调 [`open`](Self::open) 完成 USB 协议握手。
///
/// 一次只能执行一个操作；并发调用会破坏 USB 时序。
pub struct RioDevice {
    /// USB transport
    pub transport: Box<dyn Transport>,
}

impl RioDevice {
    /// 创建设备实例（不打开 USB）
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self { transport }
    }

    /// 协议初始化（PROTOCOL.md §6）
    ///
    /// 流程：
    /// 1. `control_in(0x60, 0, 0)` 握手
    /// 2. `control_in(0x7b, time_hi, time_lo)` 设置时钟（本地 Unix 时间戳）
    /// 3. `control_in(0x61, 0, 0)` × 2 poll
    /// 4. `control_in(0x65, 0, 0)` unknown but required
    /// 5. for i in 0..3: `control_in(0x60)` + `control_in(0x63, i, 0)` + 2× `bulk_in(64)` 丢弃
    /// 6. `control_in(0x60, 0, 0)` unlock
    ///
    /// 重要：整个 init 序列都不校验 status[0]（rioutil 行为）。
    /// 0x61/0x65 真机 status[0] 不稳定（0x00 或 0x01），无意义重试会误判失败。
    pub async fn open(&mut self) -> Result<()> {
        // 1. 握手 0x60
        self.raw_control_in(OP_UNKNOWN00, 0, 0).await?;

        // 2. 设置时钟（本地 Unix 时间戳）
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| CyrioError::Other(format!("system time: {}", e)))?
            .as_secs() as u32;
        self.raw_control_in(OP_RIO_TIMES, (now >> 16) as u16, (now & 0xffff) as u16)
            .await?;

        // 3. Poll × 2
        self.raw_control_in(OP_RIO_POLLD, 0, 0).await?;
        self.raw_control_in(OP_RIO_POLLD, 0, 0).await?;

        // 4. unknown 0x65
        self.raw_control_in(OP_UNKNOWN65, 0, 0).await?;

        // 5. 查询 3 种文件类型
        for i in 0..3u16 {
            self.raw_control_in(OP_UNKNOWN00, 0, 0).await?;
            self.raw_control_in(OP_RIO_TYPEQ, i, 0).await?;
            // 丢弃 2 × 64B bulk 响应
            self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
        }

        // 6. unlock
        self.raw_control_in(OP_UNKNOWN00, 0, 0).await?;

        Ok(())
    }

    /// 关闭设备（transport 的 Drop 会清理 USB 资源）
    pub async fn close(&mut self) -> Result<()> {
        Ok(())
    }

    /// 发送命令（带重试 + status[0] 校验）
    ///
    /// 成功判定：返回的 12 字节状态缓冲区首字节 == `COMMAND_SUCCESS_BYTE` (0x01)。
    /// 否则重试最多 `COMMAND_MAX_RETRIES` 次，每次间隔 50ms。
    pub async fn send_command(&self, opcode: u8, arg1: u16, arg2: u16) -> Result<Vec<u8>> {
        let mut last_err: Option<CyrioError> = None;
        for _ in 0..=COMMAND_MAX_RETRIES {
            match self.raw_control_in(opcode, arg1, arg2).await {
                Ok(status) => {
                    if !status.is_empty() && status[0] == COMMAND_SUCCESS_BYTE {
                        return Ok(status);
                    }
                    last_err = Some(CyrioError::Device(format!(
                        "send_command(0x{:02x}, {}, {}): status[0]=0x{:02x} (expected 0x{:02x})",
                        opcode,
                        arg1,
                        arg2,
                        status.first().copied().unwrap_or(0),
                        COMMAND_SUCCESS_BYTE
                    )));
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
            // 重试前等待
            smol::Timer::after(Duration::from_millis(COMMAND_RETRY_DELAY_MS)).await;
        }
        Err(last_err.unwrap_or_else(|| CyrioError::Other("send_command: unreachable".into())))
    }

    /// 读取内存单元信息（OP_RIO_MEMRI 0x68）
    ///
    /// # 参数
    /// - `mem_unit`：0=内置闪存, 1=SD 卡
    ///
    /// # 返回
    /// `RioMem`（size=0 表示该单元不存在，如未插 SD 卡）
    pub async fn get_memory_info(&self, mem_unit: u8) -> Result<RioMem> {
        self.send_command(OP_RIO_MEMRI, mem_unit as u16, 0).await?;
        let buf = self.transport.bulk_in(EP_IN, RIO_MEM_SIZE).await?;
        parse_rio_mem(&buf)
    }

    /// 读取文件信息（OP_RIO_FILEI 0x69）
    ///
    /// # 注意
    /// `wIndex` 是 **0-based slot index**，不是 `rio_file_t.file_no`。
    /// 当 file_no 与 slot 一致时（内置存储常见）可直接用此方法。
    /// 不一致时（SD 卡 fileNo=16416 但 slot=1）应使用
    /// [`find_file_info_by_file_no`](Self::find_file_info_by_file_no)。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `file_no`：文件号 / slot index（1-based）
    ///
    /// # 返回
    /// `Some(RioFile)` 或 `None`（空槽，文件不存在）
    pub async fn get_file_info(&self, mem_unit: u8, file_no: u32) -> Result<Option<RioFile>> {
        self.send_command(OP_RIO_FILEI, mem_unit as u16, file_no as u16)
            .await?;
        let buf = self.transport.bulk_in(EP_IN, PKT_HEADER).await?;
        let file = parse_rio_file(&buf)?;
        if file.file_no == 0 {
            Ok(None)
        } else {
            Ok(Some(file))
        }
    }

    /// 通过 file_no 查找文件信息（slot 迭代查找）
    ///
    /// RIO_FILEI 的 wIndex 是 0-based slot index，不是真实 file_no。
    /// 此方法迭代所有 slot，找到 `rio_file_t.file_no == target_file_no` 的文件。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `target_file_no`：目标文件号
    ///
    /// # 返回
    /// `Some(RioFile)` 或 `None`（未找到）
    pub async fn find_file_info_by_file_no(
        &self,
        mem_unit: u8,
        target_file_no: u32,
    ) -> Result<Option<RioFile>> {
        for slot in 0..FILE_NO_MAX {
            self.send_command(OP_RIO_FILEI, mem_unit as u16, slot as u16)
                .await?;
            let buf = self.transport.bulk_in(EP_IN, PKT_HEADER).await?;
            let file = parse_rio_file(&buf)?;
            if file.file_no == 0 {
                return Ok(None); // 空槽，目标不存在
            }
            if file.file_no == target_file_no {
                return Ok(Some(file));
            }
        }
        Ok(None)
    }

    /// 列出内存单元上的所有文件
    ///
    /// 从 file_no=1 起逐个查询，遇到空槽时**不立即停止**，连续遇到
    /// `LIST_FILES_EMPTY_GAP`（200）个空槽才停止。
    ///
    /// 原因：删除操作后 slot 表可能出现大段连续空槽，后面的 slot 仍有歌曲。
    /// 如果遇第一个空槽就 break，会漏掉空槽后面的所有歌曲。
    /// 连续 200 个空槽才停止可覆盖绝大部分场景，
    /// 同时避免无意义扫描整个 3000 slot 表（USB 慢）。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `on_progress`：可选进度回调（每个 file_no 一次）
    pub async fn list_files(
        &self,
        mem_unit: u8,
        mut on_progress: impl FnMut(u32),
    ) -> Result<Vec<RioFile>> {
        let mut result = Vec::new();
        let mut consecutive_empty: u32 = 0;
        for file_no in FILE_NO_MIN..=FILE_NO_MAX {
            match self.get_file_info(mem_unit, file_no).await? {
                Some(file) => {
                    consecutive_empty = 0;
                    on_progress(file_no);
                    result.push(file);
                }
                None => {
                    consecutive_empty += 1;
                    if consecutive_empty >= LIST_FILES_EMPTY_GAP {
                        break; // 连续足够多空槽，停止
                    }
                }
            }
        }
        Ok(result)
    }

    // ========================================================================
    // 写操作（Phase 5.1）
    // ========================================================================

    /// 上传新文件（OP_RIO_WRITE 0x6c）
    ///
    /// 时序（PROTOCOL.md §7）：
    /// 1. `send_command(0x6c, mem_unit, 0)` 上传命令（wIndex=0 让设备分配新 fileNo）
    /// 2. `bulk_in` 期望 `SRIORDY`（设备就绪）
    /// 3. `bulk_in` 期望 `SRIODATA`（设备准备好接收数据）
    /// 4. 循环发送 16384B 数据块：
    ///    - `bulk_out` CRIODATA 握手包（含 CRC32）
    ///    - `bulk_out` 16384B 数据块（最后一块补 0）
    ///    - `bulk_in` 期望 SRIODATA 确认
    /// 5. `bulk_out` CRIOINFO 握手包（**无 CRC**）
    /// 6. `bulk_out` 2048B 文件头
    /// 7. `bulk_in` 64B 头确认响应（不校验 magic）
    /// 8. `send_command(0x60, 0, 0)` 收尾（关键，不能漏）
    /// 9. 扫描所有 slot 找新分配的 fileNo（设备分配的 fileNo 与 slot index 无关）
    ///
    /// # 参数
    /// - `mem_unit`：目标内存单元
    /// - `header`：文件元数据（fileNo 字段被忽略，由设备分配）
    /// - `data`：文件内容（纯音频字节，已跳过 ID3v2）
    /// - `on_progress`：进度回调
    /// - `header_buffer`：可选原始 2048B header buffer（保留所有未知字段）。
    ///   若传入，则用它作为 CRIOINFO 后的 header（仅更新已知字段），保留所有未知字段；
    ///   否则用 `serialize_rio_file(header)`。用于 rename 的 delete + upload 场景，
    ///   保留原文件的未知字段（如 unk1[4] 等）。
    ///
    /// # 返回
    /// 设备分配的新 fileNo
    pub async fn upload_file(
        &self,
        mem_unit: u8,
        header: &RioFile,
        data: &[u8],
        mut on_progress: impl FnMut(UploadProgress),
        header_buffer: Option<&[u8; RIO_FILE_SIZE]>,
    ) -> Result<u32> {
        let outcome = async {
            // 1. 上传命令（wIndex=0 让设备分配新 fileNo）
            log::info!(
                "upload_file: OP_RIO_WRITE mem_unit={} data_len={} header_buffer={}",
                mem_unit, data.len(), header_buffer.is_some()
            );
            self.send_command(OP_RIO_WRITE, mem_unit as u16, 0).await?;

            // 2. SRIORDY
            let rdy = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            expect_magic(&rdy, MAGIC_SRIORDY, "upload init")?;

            // 3. SRIODATA
            let ready = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            expect_magic(&ready, MAGIC_SRIODATA, "upload ready")?;
            log::info!("upload_file: device ready, starting data transfer");

            // 4. 数据块循环
            let total = data.len() as u32;
            let mut sent: u32 = 0;
            let mut chunk_buf = [0u8; PKT_BLOCK];
            let block_count = (data.len() + PKT_BLOCK - 1) / PKT_BLOCK;
            for (i, off) in (0..data.len()).step_by(PKT_BLOCK).enumerate() {
                let end = (off + PKT_BLOCK).min(data.len());
                let chunk = &data[off..end];
                let block: &[u8] = if chunk.len() == PKT_BLOCK {
                    chunk
                } else {
                    // 最后一块补 0 到 16384B
                    chunk_buf[..chunk.len()].copy_from_slice(chunk);
                    for b in &mut chunk_buf[chunk.len()..] {
                        *b = 0;
                    }
                    &chunk_buf
                };

                // 4a. CRIODATA 握手包（含 CRC32）
                self.transport.bulk_out(EP_OUT, &build_crio_data(block)).await?;
                // 4b. 16384B 数据块
                self.transport.bulk_out(EP_OUT, block).await?;
                // 4c. 期望 SRIODATA 确认
                let ack = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
                if let Err(e) = expect_magic(&ack, MAGIC_SRIODATA, "upload block") {
                    log::error!(
                        "upload_file: block {}/{} at offset {} failed: {}",
                        i + 1, block_count, off, e
                    );
                    return Err(CyrioError::Device(format!(
                        "upload block at offset {}: {}",
                        off, e
                    )));
                }

                sent = sent.saturating_add(chunk.len() as u32);
                on_progress(UploadProgress { transferred: sent, total });
            }
            log::info!(
                "upload_file: data transfer complete ({} blocks, {} bytes), sending header",
                block_count, sent
            );

            // 5. CRIOINFO 握手包（无 CRC）
            self.transport.bulk_out(EP_OUT, &build_crio_info()).await?;
            // 6. 2048B 文件头
            //    若传入 header_buffer，用原始 buffer（保留所有未知字段），更新已知字段。
            //    用于 rename 的 delete + upload 场景，保留原文件的未知字段。
            let header_buf: [u8; RIO_FILE_SIZE] = if let Some(orig) = header_buffer {
                let mut buf = *orig;
                let updates = RioFileUpdates {
                    file_no: Some(header.file_no),
                    size: Some(header.size),
                    mod_date: Some(header.mod_date),
                    name: Some(header.name.clone()),
                    title: Some(header.title.clone()),
                    artist: Some(header.artist.clone()),
                    album: Some(header.album.clone()),
                    bits: Some(header.bits),
                    file_type: Some(header.file_type),
                    sample_rate: Some(header.sample_rate),
                    bit_rate: Some(header.bit_rate),
                    time: Some(header.time),
                    start: Some(header.start),
                    ..Default::default()
                };
                overwrite_rio_file_fields(&mut buf, &updates);
                buf
            } else {
                serialize_rio_file(header)
            };
            self.transport.bulk_out(EP_OUT, &header_buf).await?;
            // 7. 读 64B 头确认响应（不校验 magic，rioutil 行为）
            self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            log::info!("upload_file: header accepted, finalizing");

            // 8. 收尾（关键，不能漏！否则设备状态异常）
            self.send_command(OP_UNKNOWN00, 0, 0).await?;

            // 9. 扫描所有 slot 找新分配的 fileNo
            self.find_uploaded_file_no(mem_unit, header).await
        }
        .await;

        if outcome.is_err() {
            self.abort_transfer().await;
        }
        outcome
    }

    /// 覆盖已存在文件（OP_RIO_OVWRT 0x88）
    ///
    /// 与 `upload_file` 时序相同，仅 opcode 改为 0x88。
    ///
    /// # 关键：wIndex = 已存在 file_no（PROTOCOL.md §16.10）
    /// `RIO_OVWRT (0x88)` 的 wIndex 传**已存在文件号**，不是 0。
    /// `RIO_WRITE (0x6c)` 的 wIndex 才是 0（让设备分配新号）。
    /// 混淆会导致设备状态异常（看似成功但实际未覆盖，旧文件不变）。
    ///
    /// # 关键：S-Series 播放列表兼容性
    /// `rio_file_t` 结构含许多未知字段（如 0x78 `unk1[4]`，注释说
    /// "Associated with S-Series playlists"）。`parse_rio_file` → `serialize_rio_file`
    /// 会清零这些字段，导致设备无法识别目标歌单。因此覆盖歌单时应传入 `download_file`
    /// 返回的原始 `header_buffer`，仅修改 `file_no`/`size`/`mod_date` 字段。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `file_no`：已存在的文件号（写入 wIndex 和 header.fileNo，设备据此定位文件）
    /// - `header`：新文件元数据（fileNo 字段必须已设置为要覆盖的文件号）
    /// - `data`：新文件内容
    /// - `on_progress`：进度回调
    /// - `header_buffer`：可选原始 2048B header buffer（保留所有未知字段）。
    ///   若传入，则用它作为 CRIOINFO 后的 header（仅更新 file_no/size/mod_date 字段）；
    ///   否则用 `serialize_rio_file(header)`。
    pub async fn overwrite_file(
        &self,
        mem_unit: u8,
        file_no: u32,
        header: &RioFile,
        data: &[u8],
        mut on_progress: impl FnMut(UploadProgress),
        header_buffer: Option<&[u8; RIO_FILE_SIZE]>,
    ) -> Result<()> {
        let outcome = async {
            // 1. 覆盖命令（wIndex=已存在 file_no，PROTOCOL.md §16.10）
            //    设备通过 wIndex 识别要覆盖的文件，不是 0。
            //    RIO_WRITE (0x6c) 的 wIndex 才是 0（让设备分配新号）。
            //    file_no 不会超过 u16 范围（SD 卡 0x4020=16416，内置存储 < 256）
            log::info!(
                "overwrite_file: OP_RIO_OVWRT mem_unit={} wIndex(file_no)={} data_len={} header_buffer={}",
                mem_unit, file_no, data.len(), header_buffer.is_some()
            );
            self.send_command(OP_RIO_OVWRT, mem_unit as u16, file_no as u16)
                .await?;

            // 2. SRIORDY
            let rdy = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            expect_magic(&rdy, MAGIC_SRIORDY, "overwrite init")?;

            // 3. SRIODATA
            let ready = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            expect_magic(&ready, MAGIC_SRIODATA, "overwrite ready")?;
            log::info!("overwrite_file: device ready, starting data transfer");

            // 4. 数据块循环
            let total = data.len() as u32;
            let mut sent: u32 = 0;
            let mut chunk_buf = [0u8; PKT_BLOCK];
            for off in (0..data.len()).step_by(PKT_BLOCK) {
                let end = (off + PKT_BLOCK).min(data.len());
                let chunk = &data[off..end];
                let block: &[u8] = if chunk.len() == PKT_BLOCK {
                    chunk
                } else {
                    chunk_buf[..chunk.len()].copy_from_slice(chunk);
                    for b in &mut chunk_buf[chunk.len()..] {
                        *b = 0;
                    }
                    &chunk_buf
                };

                self.transport.bulk_out(EP_OUT, &build_crio_data(block)).await?;
                self.transport.bulk_out(EP_OUT, block).await?;
                let ack = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
                if let Err(e) = expect_magic(&ack, MAGIC_SRIODATA, "overwrite block") {
                    return Err(CyrioError::Device(format!(
                        "overwrite block at offset {}: {}",
                        off, e
                    )));
                }

                sent = sent.saturating_add(chunk.len() as u32);
                on_progress(UploadProgress { transferred: sent, total });
            }

            // 5. CRIOINFO + 2048B 头
            self.transport.bulk_out(EP_OUT, &build_crio_info()).await?;

            // 准备最终 header buffer：
            // 若传入 header_buffer，用原始 buffer（保留所有未知字段），更新
            // file_no/size/modDate 以及 name/title/bits 字段。
            // 关键：设备在 bit 0=1 时会双重编码返回 name/title，header_buffer 里的
            // name/title 字节可能是双重编码的。用 parse_rio_file 解析后的正确 name
            // 覆盖，同时修正 bits（PLS 清除 bit 0），防止双重编码字节被写回设备。
            let final_header_buf: [u8; RIO_FILE_SIZE] = if let Some(orig) = header_buffer {
                let mut buf = *orig;
                let updates = RioFileUpdates {
                    file_no: Some(file_no),
                    size: Some(header.size),
                    mod_date: Some(header.mod_date),
                    name: Some(header.name.clone()),
                    title: Some(header.title.clone()),
                    artist: Some(header.artist.clone()),
                    album: Some(header.album.clone()),
                    bits: Some(header.bits),
                    ..Default::default()
                };
                overwrite_rio_file_fields(&mut buf, &updates);
                buf
            } else {
                serialize_rio_file(header)
            };
            self.transport.bulk_out(EP_OUT, &final_header_buf).await?;

            // 6. 读 64B 头确认响应（不校验 magic，rioutil 行为）
            self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;

            // 7. 收尾
            self.send_command(OP_UNKNOWN00, 0, 0).await?;

            log::info!("overwrite_file: completed successfully");
            Ok(())
        }
        .await;

        if outcome.is_err() {
            self.abort_transfer().await;
        }
        outcome
    }

    /// 下载文件（OP_RIO_READF 0x70，S-Series 原生支持）
    ///
    /// 时序（参考 rioutil song_management.c download_file_rio）：
    /// 1. 迭代 RIO_FILEI 槽位（wIndex 是 0-based slot index），找到 fileNo 匹配的文件，
    ///    获取完整 2048B 文件头 buffer
    /// 2. `send_command(0x70, mem_unit, 0)`
    /// 3. `bulk_in` 64B 初始响应
    /// 4. `bulk_out` 2048B **完整**文件头（来自步骤 1，非空 reqHeader）
    /// 5. `bulk_in` 64B 响应，检查 SRIONOFL（文件不存在）
    /// 6. 循环 `blocks = ceil(size / 16384)` 次：
    ///    - `bulk_out` CRIODATA（空 16384B 块，CRC=0）
    ///    - `bulk_in` 64B 响应（SRIODONE 则提前结束）
    ///    - `bulk_in` 16384B 数据块
    /// 7. 循环结束后若未收到 SRIODONE，再发一个 CRIODATA（gen4+ 不读响应）
    ///
    /// 下载不会删除原文件（与老机型 gen3 不同）。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `file_no`：要下载的文件号（rio_file_t.file_no，非槽位索引）
    /// - `on_progress`：进度回调
    pub async fn download_file(
        &self,
        mem_unit: u8,
        file_no: u32,
        mut on_progress: impl FnMut(UploadProgress),
    ) -> Result<DownloadResult> {
        let outcome = async {
            // 1. 迭代 RIO_FILEI 槽位，找到 fileNo 匹配的完整 2048B 文件头
            let full_header_buf = self
                .find_slot_buffer_by_file_no(mem_unit, file_no)
                .await?
                .ok_or_else(|| {
                    CyrioError::Device(format!(
                        "download_file: file {} not found on memUnit {} (slot iteration hit empty slot)",
                        file_no, mem_unit
                    ))
                })?;
            let header = parse_rio_file(&full_header_buf)?;

            // 2. 发送下载命令
            self.send_command(OP_RIO_READF, mem_unit as u16, 0).await?;

            // 3. 读 64B 初始响应（rioutil 未严格校验此包内容）
            self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;

            // 4. 写 2048B 完整文件头（来自 RIO_FILEI，非空 reqHeader）
            //    rioutil: write_block_rio(rio, file, RIO_HEADER_SIZE, NULL)
            //    cksum_hdr=NULL 表示不发 CRIODATA 前导包，直接写 2048B
            self.transport.bulk_out(EP_OUT, &full_header_buf[..]).await?;

            // 5. 读 64B 响应：检查 SRIONOFL（文件不存在）
            let fl_resp = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            if parse_magic(&fl_resp).as_bytes() == MAGIC_SRIONOFL {
                return Err(CyrioError::Device(format!(
                    "download_file: file {} not found on memUnit {} (SRIONOFL)",
                    file_no, mem_unit
                )));
            }

            // 6. 数据块循环
            let total = header.size;
            let blocks = Self::download_block_count(total);
            let mut chunks: Vec<u8> = Vec::with_capacity(total as usize);
            let mut received: u32 = 0;
            let mut download_complete = false;

            let empty_block = [0u8; PKT_BLOCK];
            for _ in 0..blocks {
                // 6a. 发 CRIODATA 包（空 16384B 块，CRC=0）
                self.transport
                    .bulk_out(EP_OUT, &build_crio_data(&empty_block))
                    .await?;

                // 6b. 读 64B 响应：SRIODONE 则提前完成
                let resp = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
                if parse_magic(&resp).as_bytes() == MAGIC_SRIODONE {
                    download_complete = true;
                    break;
                }

                // 6c. 读 16384B 数据块
                let block = self.transport.bulk_in(EP_IN, PKT_BLOCK).await?;
                let take = block.len();
                chunks.extend_from_slice(&block);
                received = received.saturating_add(take as u32);
                on_progress(UploadProgress { transferred: received, total });
            }

            // 7. 循环结束后若未收到 SRIODONE，再发一个 CRIODATA
            //    rioutil: gen4+ (S-Series) 不读响应，只发 CRIODATA
            if !download_complete {
                self.transport
                    .bulk_out(EP_OUT, &build_crio_data(&empty_block))
                    .await?;
            }

            // 8. 按 header.size 截断（最后一块可能补了 0）
            chunks.truncate(total as usize);

            Ok(DownloadResult {
                header,
                header_buffer: Box::new(full_header_buf),
                data: chunks,
            })
        }
        .await;

        if outcome.is_err() {
            self.abort_transfer().await;
        }
        outcome
    }

    /// 删除文件（OP_RIO_DELET 0x78）
    ///
    /// 时序（PROTOCOL.md §9）：
    /// 1. `send_command(0x78, mem_unit, 0)`
    /// 2. `bulk_in` 期望 SRIODELS（删除就绪）
    /// 3. `bulk_out` 写裸 2048B 文件头（**无 CRIODATA 前缀**）
    /// 4. `bulk_in` 期望 SRIODELD（删除完成）
    ///
    /// # 关键
    /// 删除时发送的 2048B `rio_file_t` **不**前置 CRIODATA 包，直接写裸 2048B。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `file_no`：要删除的文件号
    pub async fn delete_file(&self, mem_unit: u8, file_no: u32) -> Result<()> {
        let outcome = async {
            // 1. 删除命令
            self.send_command(OP_RIO_DELET, mem_unit as u16, 0).await?;

            // 2. 期望 SRIODELS（删除就绪）
            let dels = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            expect_magic(&dels, MAGIC_SRIODELS, "delete init")?;

            // 3. 写裸 2048B 文件头（无 CRIODATA 前缀！）
            //    只需 file_no 字段，其余字段为 0
            let mut header = RioFile::empty();
            header.file_no = file_no;
            let header_buf = serialize_rio_file(&header);
            self.transport.bulk_out(EP_OUT, &header_buf).await?;

            // 4. 期望 SRIODELD（删除完成）
            let deld = self.transport.bulk_in(EP_IN, PKT_HANDSHAKE).await?;
            expect_magic(&deld, MAGIC_SRIODELD, "delete done")?;

            Ok(())
        }
        .await;

        if outcome.is_err() {
            self.abort_transfer().await;
        }
        outcome
    }

    // ========================================================================
    // 内部辅助
    // ========================================================================

    /// 中止当前传输
    ///
    /// 尽力发 64B "CRIOABRT" 包，不抛错。
    /// 用于异常路径：在 upload/download/delete 失败时调用，防止设备停留等待状态。
    pub async fn abort_transfer(&self) {
        let _ = self
            .transport
            .bulk_out(crate::protocol::constants::EP_OUT, &build_crio_abort())
            .await;
    }

    /// 不校验 status[0] 的 control_in（用于 init 序列）
    ///
    /// 0x61 poll 真机返回 status[0]=0x00，0x65 在不同运行返回 0x00 或 0x01，
    /// 都正常。绕过 send_command 校验避免误判失败。
    async fn raw_control_in(&self, opcode: u8, value: u16, index: u16) -> Result<Vec<u8>> {
        let setup = ControlSetup {
            request_type: 0,
            request: opcode,
            value,
            index,
            length: CONTROL_STATUS_LENGTH as u16,
        };
        self.transport.control_in(setup).await
    }

    /// 检查 bulk 响应是否为 SRIONOFL（文件不存在）
    ///
    /// 返回 true 表示是 SRIONOFL
    #[allow(dead_code)] // Phase 5.2 list_playlist_songs 会用到
    pub(crate) fn is_no_file_response(buf: &[u8]) -> bool {
        parse_magic(buf).as_bytes() == MAGIC_SRIONOFL
    }

    /// 计算下载文件需要的块数
    pub(crate) fn download_block_count(file_size: u32) -> u32 {
        (file_size as usize).div_ceil(PKT_BLOCK) as u32
    }

    /// 迭代 RIO_FILEI 槽位，找到 fileNo 匹配的文件，返回完整 2048B header buffer
    ///
    /// RIO_FILEI 的 wIndex 是 **0-based 槽位索引**（真机实测），不是真实文件号。
    /// 设备返回的 `rio_file_t.file_no`（offset 0x00）才是真实文件号。
    ///
    /// 迭代从 slot=0 开始，遇到 `file_no==0`（空槽）时**不立即停止**，
    /// 连续遇到 `LIST_FILES_EMPTY_GAP`（200）个空槽才返回 `None`。
    ///
    /// 原因：历史上 wIndex=0 bug 会创建 file_no 不连续的残留文件（0 秒音频），
    /// 这些文件位于空槽之后。如果遇第一个空槽就返回 None，download_file 会报
    /// "file not found"，导致 rename_song_title 静默失败（前端显示成功但实际未写入）。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `target_file_no`：目标文件号（rio_file_t.file_no）
    pub async fn find_slot_buffer_by_file_no(
        &self,
        mem_unit: u8,
        target_file_no: u32,
    ) -> Result<Option<[u8; RIO_FILE_SIZE]>> {
        let mut consecutive_empty: u32 = 0;
        for slot in 0..FILE_NO_MAX {
            self.send_command(OP_RIO_FILEI, mem_unit as u16, slot as u16)
                .await?;
            let buf = self.transport.bulk_in(EP_IN, PKT_HEADER).await?;
            if buf.len() < RIO_FILE_SIZE {
                return Err(CyrioError::Parse(format!(
                    "find_slot_buffer_by_file_no: RIO_FILEI returned {} bytes, need {}",
                    buf.len(),
                    RIO_FILE_SIZE
                )));
            }
            let file = parse_rio_file(&buf)?;
            if file.file_no == 0 {
                consecutive_empty += 1;
                if consecutive_empty >= LIST_FILES_EMPTY_GAP {
                    return Ok(None); // 连续足够多空槽，目标不存在
                }
                continue;
            }
            consecutive_empty = 0;
            if file.file_no == target_file_no {
                let mut arr = [0u8; RIO_FILE_SIZE];
                arr.copy_from_slice(&buf[..RIO_FILE_SIZE]);
                return Ok(Some(arr));
            }
        }
        Ok(None) // 达到 FILE_NO_MAX 仍未找到
    }

    /// 扫描所有 slot 找新上传的文件，返回其 .file_no
    ///
    /// 设备分配的 fileNo 与 slot index 无关（真机实测：SD 卡上新歌单 slot=1 但 fileNo=18560，
    /// 内置存储 slot=0 对应 fileNo=16）。无法从 slot index 推断 fileNo，必须扫描所有 slot
    /// 找匹配的文件。
    ///
    /// 匹配策略（两级，避免设备 modDate 不精确导致找不到）：
    /// 1. **精确匹配**：size + type + modDate 三元组完全相同
    /// 2. **降级匹配**：若精确匹配无结果，用 size + type 匹配（设备可能用内部时钟覆盖
    ///    modDate 字段，导致存储值与上传 header.mod_date 不一致）
    ///
    /// 设备会把上传的 name 字段从 latin1 转 UTF-8 存储（真机实测），
    /// 导致 file.name ≠ header.name（双重编码）。因此不能用 name 匹配。
    /// 若有多个匹配（如重复上传相同文件），返回 fileNo 最大的（新文件通常 fileNo 最大）。
    ///
    /// 遇到空槽时连续 `LIST_FILES_EMPTY_GAP` 个才停止扫描（与 `list_files` 一致），
    /// 避免设备把新文件分配到前面有空槽的位置时找不到。
    ///
    /// # 参数
    /// - `mem_unit`：内存单元
    /// - `header`：上传时使用的文件元数据（用于匹配）
    pub async fn find_uploaded_file_no(
        &self,
        mem_unit: u8,
        header: &RioFile,
    ) -> Result<u32> {
        let mut exact_best: u32 = 0; // 精确匹配（size+type+modDate）
        let mut fuzzy_best: u32 = 0; // 降级匹配（仅 size+type）
        let mut consecutive_empty: u32 = 0;
        let mut scanned_files = 0u32;
        let mut first_non_empty: Option<RioFile> = None;
        for slot in 0..FILE_NO_MAX {
            self.send_command(OP_RIO_FILEI, mem_unit as u16, slot as u16)
                .await?;
            let buf = self.transport.bulk_in(EP_IN, PKT_HEADER).await?;
            let file = parse_rio_file(&buf)?;
            if file.file_no == 0 {
                consecutive_empty += 1;
                if consecutive_empty >= LIST_FILES_EMPTY_GAP {
                    break; // 连续足够多空槽，停止扫描
                }
                continue;
            }
            consecutive_empty = 0;
            scanned_files += 1;
            if first_non_empty.is_none() {
                first_non_empty = Some(file.clone());
            }
            let type_size_match = file.size == header.size
                && file.file_type == header.file_type;
            // 精确匹配：size + type + modDate
            if type_size_match
                && file.mod_date == header.mod_date
                && file.file_no > exact_best
            {
                exact_best = file.file_no;
            }
            // 降级匹配：仅 size + type（不要求 modDate 一致）
            // 仅当尚未精确命中时记录，避免覆盖精确结果
            if type_size_match && exact_best == 0 && file.file_no > fuzzy_best {
                fuzzy_best = file.file_no;
            }
        }
        let best_file_no = if exact_best > 0 {
            exact_best
        } else {
            fuzzy_best
        };
        if best_file_no == 0 {
            // 详细日志：列出扫描到的第一个文件，方便排查
            let first_info = first_non_empty.as_ref().map(|f| format!(
                "first_file: file_no={} size={} type=0x{:x} modDate={}",
                f.file_no, f.size, f.file_type, f.mod_date
            )).unwrap_or_else(|| "no files on device".to_string());
            log::error!(
                "find_uploaded_file_no: FAILED memUnit={} scanned={} files. expected size={} type=0x{:x} modDate={}. {}",
                mem_unit, scanned_files, header.size, header.file_type, header.mod_date, first_info
            );
            return Err(CyrioError::Device(format!(
                "上传后未找到文件：memUnit={} 已扫描 {} 个文件，期望 size={} type=0x{:x} modDate={}。{}",
                mem_unit, scanned_files, header.size, header.file_type, header.mod_date, first_info
            )));
        }
        if exact_best == 0 {
            // 降级匹配命中：modDate 不一致，记录警告便于诊断
            log::warn!(
                "find_uploaded_file_no: fuzzy match (size+type only) memUnit={} file_no={} expected_modDate={} (device may override modDate)",
                mem_unit, best_file_no, header.mod_date
            );
        } else {
            log::info!(
                "find_uploaded_file_no: exact match memUnit={} file_no={}",
                mem_unit, best_file_no
            );
        }
        Ok(best_file_no)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_no_file_response_detects_srionofl() {
        let mut buf = [0u8; 64];
        buf[..MAGIC_SRIONOFL.len()].copy_from_slice(MAGIC_SRIONOFL);
        assert!(RioDevice::is_no_file_response(&buf));

        // SRIORDY 不是 NOFL
        let mut buf = [0u8; 64];
        buf[..7].copy_from_slice(b"SRIORDY");
        assert!(!RioDevice::is_no_file_response(&buf));
    }

    #[test]
    fn download_block_count_for_typical_sizes() {
        // 0 字节：0 块（但实际协议下至少 1 块，这里只算数学上界）
        assert_eq!(RioDevice::download_block_count(0), 0);
        // 1 字节：1 块
        assert_eq!(RioDevice::download_block_count(1), 1);
        // 16384 字节：1 块
        assert_eq!(RioDevice::download_block_count(16384), 1);
        // 16385 字节：2 块
        assert_eq!(RioDevice::download_block_count(16385), 2);
        // 5MB：~320 块
        assert_eq!(RioDevice::download_block_count(5 * 1024 * 1024), 320);
    }

    #[test]
    fn download_block_count_uses_ceil() {
        // 验证向上取整
        assert_eq!(RioDevice::download_block_count(PKT_BLOCK as u32), 1);
        assert_eq!(RioDevice::download_block_count(PKT_BLOCK as u32 + 1), 2);
        assert_eq!(RioDevice::download_block_count(2 * PKT_BLOCK as u32), 2);
        assert_eq!(RioDevice::download_block_count(2 * PKT_BLOCK as u32 + 1), 3);
    }

    #[test]
    fn upload_progress_struct_is_copy() {
        let p = UploadProgress { transferred: 100, total: 1000 };
        let p2 = p;
        assert_eq!(p2.transferred, 100);
        assert_eq!(p2.total, 1000);
        // Copy trait 允许 p 仍然可用
        assert_eq!(p.transferred, 100);
    }

    #[test]
    fn download_result_fields_accessible() {
        let header = RioFile::empty();
        let header_buffer = Box::new([0u8; RIO_FILE_SIZE]);
        let data = vec![0u8; 100];
        let r = DownloadResult {
            header: header.clone(),
            header_buffer,
            data: data.clone(),
        };
        assert_eq!(r.header, header);
        assert_eq!(r.header_buffer.len(), RIO_FILE_SIZE);
        assert_eq!(r.data, data);
    }
}
