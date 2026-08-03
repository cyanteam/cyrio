//! 重命名/批量文本处理 API
//!
//! 基于 cyrio_text 的 slug/strip 功能，对设备上的文件标题进行修改。
//!
//! ## rename 实现策略：OVWRT + 验证 + upload-then-delete fallback
//!
//! 1. `download_file` 取 data + 原始 header_buffer
//! 2. 修改 title（+ 更新 mod_date 用于后续 upload 匹配）
//! 3. `overwrite_file`（OP_RIO_OVWRT 0x88，wIndex=file_no）尝试覆盖
//! 4. **验证**：重新读取文件，检查 title 是否真的更新
//! 5. 验证失败（SD 卡固件不支持下详述）→ fallback：**先 upload 新文件，成功后再 delete 旧文件**
//!    用原始 header_buffer 保留未知字段
//!
//! ## 为什么需要 fallback
//!
//! SD 卡固件对 `OP_RIO_OVWRT` 支持不完整：命令返回成功（SRIORDY/SRIODATA），
//! 但实际未写入 title 字段。真机日志显示 "overwrite_file: completed successfully"
//! 后刷新列表，title 仍是旧值。内置存储（mem_unit=0）OVWRT 工作正常。
//! 因此 rename 后必须验证，不工作则用 upload + delete 替代。
//!
//! ## 安全顺序（先 upload 后 delete）
//!
//! 旧实现是先 delete 后 upload，如果 upload 失败原文件已删除，数据永久丢失。
//! 新实现改为先 upload 新文件，upload 成功后再 delete 旧文件。
//! 这样 upload 失败时原文件仍然安全，不会丢失数据。
//! 代价：需要临时额外空间存放新旧两份文件（但 upload 失败时原数据安全）。
//!
//! ## 关于"能否不下载直接改名"
//!
//! Rio 设备协议（OP_RIO_OVWRT 0x88）要求覆盖文件时重传**全部音频数据**，
//! 不支持只改 header。即使 fallback 到 delete + upload 也需要重传全部数据。
//! 这是设备固件限制，非软件层面可绕过。

use crate::api::device::{DownloadResult, RioDevice};
use crate::error::Result;

/// 重命名单个文件的 title
///
/// 流程：download → 修改 title → overwrite → 验证 → (失败时) delete + upload fallback
///
/// # 关键
/// - 必须传入 `download_file` 返回的原始 `header_buffer`（保留未知字段）
/// - SD 卡固件不支持 OVWRT，需要验证 + fallback 到 delete + upload
/// - delete + upload 会改变 file_no（设备分配新号），歌单引用可能失效
pub async fn rename_song_title(
    device: &RioDevice,
    mem_unit: u8,
    file_no: u32,
    new_title: &str,
) -> Result<()> {
    log::info!(
        "rename_song_title: start mem_unit={} file_no={} new_title={:?}",
        mem_unit, file_no, new_title
    );

    // 1. 下载（取 data + header + 原始 header_buffer）
    let dl = device.download_file(mem_unit, file_no, |_| {}).await?;
    log::info!(
        "rename_song_title: downloaded, original title={:?} name={:?} size={} data_len={}",
        dl.header.title, dl.header.name, dl.header.size, dl.data.len()
    );

    // 2. 修改 title（mod_date 在 overwrite_with_fallback 内更新）
    let mut parsed = dl.header.clone();
    parsed.title = new_title.to_string();
    log::info!(
        "rename_song_title: parsed.title set to {:?}, parsed.name={:?}",
        parsed.title, parsed.name
    );

    // 3. OVWRT + 验证 + delete+upload fallback
    overwrite_with_fallback(device, mem_unit, file_no, parsed, dl).await
}

/// 修复单个歌曲编码（清 bit 0，重新写回 name/title）
///
/// 用于修复 Phase A 之前上传的歌曲（bit 0=1 导致中文乱码）。
/// 复用 `rename_song_title` 的 OVWRT + 验证 + delete+upload fallback 策略
/// （SD 卡固件不支持 OVWRT，需要 fallback）。
///
/// 流程：download → 用解析后的正确 title（`read_fixed_string` 已恢复）→
///       overwrite_with_fallback（OVWRT + 验证 + fallback）
pub async fn repair_song_encoding(
    device: &RioDevice,
    mem_unit: u8,
    file_no: u32,
) -> Result<()> {
    // 1. download 取 header（read_fixed_string 已自动恢复正确 title）
    let dl = device.download_file(mem_unit, file_no, |_| {}).await?;
    let mut parsed = dl.header.clone();
    // 清 bit 0：修复双重编码问题
    // 设备对 bit 0=1 的文件做 latin1→UTF-8 双重编码，导致中文屏幕乱码
    // 清 bit 0 后设备按 UTF-8 直接显示，配合 read_fixed_string 恢复的正确 title 即可正常显示
    parsed.bits &= !0x01;
    log::info!(
        "repair_song_encoding: mem_unit={} file_no={} title={:?} bits 0x{:x} -> 0x{:x} (bit0 cleared)",
        mem_unit,
        file_no,
        parsed.title,
        dl.header.bits,
        parsed.bits
    );
    // 2. OVWRT + 验证 + delete+upload fallback（用解析后的 title + 清 bit 0 覆盖回设备）
    overwrite_with_fallback(device, mem_unit, file_no, parsed, dl).await
}

/// OVWRT + 验证 + delete+upload fallback 的公共核心逻辑
///
/// 接受已 download 的数据，避免 `rename_song_title` 和 `repair_song_encoding` 重复 download。
/// 内部更新 mod_date（确保 fallback 时 find_uploaded_file_no 能找到新文件）。
async fn overwrite_with_fallback(
    device: &RioDevice,
    mem_unit: u8,
    file_no: u32,
    mut parsed: crate::protocol::rio_file::RioFile,
    dl: DownloadResult,
) -> Result<()> {
    // 更新 mod_date：确保 fallback 时 upload 后能通过 find_uploaded_file_no 找到新文件
    // （size + type + modDate 三元组匹配，mod_date 不同避免与旧文件混淆）
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or_else(|_| parsed.mod_date.wrapping_add(1));
    parsed.mod_date = now;
    let expected_title = parsed.title.clone();
    log::info!(
        "overwrite_with_fallback: mem_unit={} file_no={} new title={:?} new mod_date={}",
        mem_unit,
        file_no,
        expected_title,
        parsed.mod_date
    );

    // 1. 尝试 overwrite_file（OVWRT 0x88，wIndex=file_no）
    let overwrite_result = device
        .overwrite_file(
            mem_unit,
            file_no,
            &parsed,
            &dl.data,
            |_| {},
            Some(&*dl.header_buffer),
        )
        .await;

    match &overwrite_result {
        Ok(()) => log::info!("overwrite_with_fallback: overwrite_file returned Ok"),
        Err(e) => log::warn!("overwrite_with_fallback: overwrite_file failed: {}", e),
    }

    // 2. 验证 overwrite 是否真的生效（SD 卡固件可能静默失败）
    if overwrite_result.is_ok() {
        match verify_title_updated(device, mem_unit, file_no, &expected_title).await {
            Ok(true) => {
                log::info!(
                    "overwrite_with_fallback: verified, title updated to {:?}",
                    expected_title
                );
                return Ok(());
            }
            Ok(false) => {
                log::warn!(
                    "overwrite_with_fallback: overwrite returned Ok but title NOT updated (SD card firmware bug), falling back to delete + upload"
                );
            }
            Err(e) => {
                log::warn!(
                    "overwrite_with_fallback: verify failed ({}), falling back to delete + upload",
                    e
                );
            }
        }
    }

    // 3. Fallback：先 upload 新文件，验证成功后再 delete 旧文件
    //    注意：upload + delete 会改变 file_no，歌单引用可能失效
    //
    //    安全顺序（先 upload 后 delete）：
    //    - 如果 upload 失败，旧文件仍在，数据不丢失
    //    - 如果 upload 成功但空间不足（旧文件占着空间），upload 会报错，旧文件保留
    //    - 只有 upload 成功后才 delete 旧文件
    //    代价：需要临时额外空间存放新旧两份文件（但 upload 失败时原数据安全）
    log::info!(
        "overwrite_with_fallback: fallback to upload-then-delete, file_no={}",
        file_no
    );

    let new_file_no = device
        .upload_file(
            mem_unit,
            &parsed,
            &dl.data,
            |_| {},
            Some(&*dl.header_buffer),
        )
        .await;
    match new_file_no {
        Ok(nfn) => {
            log::info!(
                "overwrite_with_fallback: upload succeeded (new file_no={}), deleting old file_no={}",
                nfn,
                file_no
            );
            // upload 成功，现在安全删除旧文件
            device.delete_file(mem_unit, file_no).await?;
            log::info!(
                "overwrite_with_fallback: upload + delete succeeded, old file_no={} -> new file_no={}",
                file_no,
                nfn
            );
            Ok(())
        }
        Err(e) => {
            // upload 失败：旧文件仍在，不删除
            log::error!(
                "overwrite_with_fallback: upload failed ({}), keeping original file_no={} (NOT deleted)",
                e,
                file_no
            );
            Err(e)
        }
    }
}

/// 验证文件的 title 和 bit 0 是否已正确写入
///
/// 重新读取文件头，对比 title 字段，并检查 bit 0 是否被清。
/// 用于检测 OVWRT 是否真的生效（SD 卡固件可能静默失败）。
///
/// # 为什么也验证 bit 0
/// `overwrite_rio_file_fields` 写入时总是清 bit 0（防止设备双重编码）。
/// 如果 OVWRT 静默失败，title 和 bit 0 都不会更新。
/// 对于 `repair_song_encoding`，title 没变（read_fixed_string 已恢复正确 title），
/// 只验证 title 无法检测 OVWRT 失败；验证 bit 0 可以检测到。
async fn verify_title_updated(
    device: &RioDevice,
    mem_unit: u8,
    file_no: u32,
    new_title: &str,
) -> Result<bool> {
    let buf = device
        .find_slot_buffer_by_file_no(mem_unit, file_no)
        .await?
        .ok_or_else(|| {
            crate::error::CyrioError::Device(format!(
                "verify_title_updated: file {} not found on memUnit {}",
                file_no, mem_unit
            ))
        })?;
    let file = crate::protocol::rio_file::parse_rio_file(&buf)?;
    let title_ok = file.title == new_title;
    let bit0_cleared = file.bits & 0x01 == 0;
    let updated = title_ok && bit0_cleared;
    log::info!(
        "verify_title_updated: file_no={} expected_title={:?} actual_title={:?} title_ok={} bits=0x{:x} bit0_cleared={} -> updated={}",
        file_no,
        new_title,
        file.title,
        title_ok,
        file.bits,
        bit0_cleared,
        updated
    );
    Ok(updated)
}

/// 重命名操作结果（批量操作返回）
#[derive(Debug, Clone, serde::Serialize)]
pub struct RenameResult {
    /// 文件号
    pub file_no: u32,
    /// 所在内存单元
    pub mem_unit: u8,
    /// 是否成功
    pub success: bool,
    /// 原始标题
    pub original: String,
    /// 新标题
    pub new_title: String,
    /// 错误信息（失败时）
    pub error: String,
}

/// 预览结果（不执行实际改名，只计算新标题）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreviewResult {
    /// 文件号
    pub file_no: u32,
    /// 所在内存单元
    pub mem_unit: u8,
    /// 原始标题
    pub original: String,
    /// 预览的新标题
    pub new_title: String,
    /// 是否会发生变化
    pub changed: bool,
}

/// 批量操作的进度回调参数
#[derive(Debug, Clone, Copy)]
pub struct BatchProgress {
    /// 当前处理到的文件索引（0-based）
    pub current: usize,
    /// 总文件数
    pub total: usize,
    /// 当前文件的原始标题
    pub current_title: &'static str,
}

/// 预览：转拼音（不执行实际改名，只计算新标题）
///
/// 纯文本操作，不需要设备连接。用于操作前让用户确认改名效果。
pub fn preview_slug(items: &[(u32, u8, String)]) -> Vec<PreviewResult> {
    let opts = cyrio_text::SlugOptions::default();
    items
        .iter()
        .map(|(file_no, mem_unit, title)| {
            let new_title = cyrio_text::to_slug(title, &opts);
            PreviewResult {
                file_no: *file_no,
                mem_unit: *mem_unit,
                original: title.clone(),
                changed: new_title != *title,
                new_title,
            }
        })
        .collect()
}

/// 预览：去词（不执行实际改名，只计算新标题）
///
/// 纯文本操作，不需要设备连接。用于操作前让用户确认改名效果。
pub fn preview_strip(
    items: &[(u32, u8, String)],
    custom_words: Vec<String>,
) -> Vec<PreviewResult> {
    let opts = cyrio_text::StripOptions {
        custom_stop_words: custom_words,
        ..Default::default()
    };
    items
        .iter()
        .map(|(file_no, mem_unit, title)| {
            let stripped = cyrio_text::strip_noise(title, &opts);
            PreviewResult {
                file_no: *file_no,
                mem_unit: *mem_unit,
                original: title.clone(),
                changed: stripped.cleaned != *title,
                new_title: stripped.cleaned,
            }
        })
        .collect()
}

/// 批量转拼音
///
/// 对给定的 (file_no, mem_unit, current_title) 列表逐个 rename。
/// 无中文的标题跳过（不调用 USB）。
///
/// `on_progress` 回调在每处理完一个文件后调用，用于前端进度反馈。
pub async fn batch_to_slug<F>(
    device: &RioDevice,
    items: Vec<(u32, u8, String)>,
    mut on_progress: F,
) -> Vec<RenameResult>
where
    F: FnMut(usize, usize, &str),
{
    let opts = cyrio_text::SlugOptions::default();
    let total = items.len();
    log::info!("batch_to_slug: start, {} items", total);
    let mut results = Vec::with_capacity(total);
    for (idx, (file_no, mem_unit, title)) in items.into_iter().enumerate() {
        let new_title = cyrio_text::to_slug(&title, &opts);
        log::info!(
            "batch_to_slug: [{}/{}] file_no={} mem_unit={} title={:?} -> new_title={:?}",
            idx + 1, total, file_no, mem_unit, title, new_title
        );
        if new_title == title {
            // 无中文，跳过
            log::info!("batch_to_slug: skipping (no change)");
            results.push(RenameResult {
                file_no,
                mem_unit,
                success: true,
                original: title.clone(),
                new_title: title,
                error: "无需转换".to_string(),
            });
            on_progress(idx + 1, total, "（跳过）");
            continue;
        }
        match rename_song_title(device, mem_unit, file_no, &new_title).await {
            Ok(()) => results.push(RenameResult {
                file_no,
                mem_unit,
                success: true,
                original: title,
                new_title,
                error: String::new(),
            }),
            Err(e) => results.push(RenameResult {
                file_no,
                mem_unit,
                success: false,
                original: title,
                new_title,
                error: e.to_string(),
            }),
        }
        on_progress(idx + 1, total, "");
    }
    results
}

/// 批量去词
///
/// 对给定的 (file_no, mem_unit, current_title) 列表逐个 rename。
/// 应用 strip_noise 去除无关词汇。无需去词的标题跳过。
///
/// `on_progress` 回调在每处理完一个文件后调用，用于前端进度反馈。
pub async fn batch_strip_noise<F>(
    device: &RioDevice,
    items: Vec<(u32, u8, String)>,
    custom_words: Vec<String>,
    mut on_progress: F,
) -> Vec<RenameResult>
where
    F: FnMut(usize, usize, &str),
{
    let opts = cyrio_text::StripOptions {
        custom_stop_words: custom_words,
        ..Default::default()
    };
    let total = items.len();
    let mut results = Vec::with_capacity(total);
    for (idx, (file_no, mem_unit, title)) in items.into_iter().enumerate() {
        let stripped = cyrio_text::strip_noise(&title, &opts);
        if stripped.cleaned == title {
            // 无需去词，跳过
            results.push(RenameResult {
                file_no,
                mem_unit,
                success: true,
                original: title.clone(),
                new_title: title,
                error: "无需去词".to_string(),
            });
            on_progress(idx + 1, total, "（跳过）");
            continue;
        }
        match rename_song_title(device, mem_unit, file_no, &stripped.cleaned).await {
            Ok(()) => results.push(RenameResult {
                file_no,
                mem_unit,
                success: true,
                original: title,
                new_title: stripped.cleaned,
                error: String::new(),
            }),
            Err(e) => results.push(RenameResult {
                file_no,
                mem_unit,
                success: false,
                original: title,
                new_title: stripped.cleaned,
                error: e.to_string(),
            }),
        }
        on_progress(idx + 1, total, "");
    }
    results
}
