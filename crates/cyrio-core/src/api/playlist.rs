//! 播放列表操作 API
//!
//! 三个高层 API：
//! - [`list_playlist_songs`]：列出歌单内的歌曲（支持跨存储引用）
//! - [`create_playlist`]：在设备上创建空歌单
//! - [`add_to_playlist`]：将歌曲加入歌单（支持跨存储 sflags）
//!
//! # 跨存储引用
//! 真机实测：内置存储的歌单可以引用 SD 卡上的歌曲（反之亦然）。
//! FIDL 条目的 `sflags` 字段在真机上的值（如 `[0x3a, 0x6e, 0x76]`、
//! `[7, 235, 198]`）并非简单的 0/1 跨存储标志，不能用来判断歌曲所在 mem_unit。
//!
//! [`list_playlist_songs`] 的做法：同时查询两个 mem_unit 的文件列表，
//! 对每个歌单条目先在歌单所在 mem_unit 查找，找不到再查另一个 mem_unit。
//! 返回的 `PlaylistSong.mem_unit` 是歌曲实际所在的 mem_unit。
//!
//! [`add_to_playlist`] 根据 `song_mem_unit` 与 `playlist_mem_unit` 是否相同
//! 自动设置 sflags（同存储 `[0,0,0]`，跨存储 `[0,0,1]`），调用方只需明确两个 mem_unit。
//!
//! # 来源
//! 移植自 NodeJS：
//! - `rio-rs/node/src/api/listPlaylistSongs.ts`
//! - `rio-rs/node/src/api/createPlaylist.ts`
//! - `rio-rs/node/src/api/addToPlaylist.ts`

use crate::api::device::RioDevice;
use crate::api::types::{is_mp3_file, precheck_free_space, rio_file_to_song, Playlist, Song};
use crate::error::Result;
use crate::protocol::constants::{RIO_NUM_OFFSET, TYPE_PLS};
use crate::protocol::fidl::{append_to_fidl, parse_fidl, serialize_fidl, FidlPlaylist};
use crate::protocol::rio_file::RioFile;

/// 歌单内单首歌曲的信息（含在歌单中的序号）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistSong {
    /// 歌曲信息
    pub song: Song,
    /// 在歌单中的序号（0-based）
    pub index: usize,
    /// 歌曲所在内存单元（用于跨存储引用显示）
    pub mem_unit: u8,
}

/// [`create_playlist`] 的返回结果
#[derive(Debug, Clone)]
pub struct CreatePlaylistResult {
    /// 设备分配的新文件号
    pub file_no: u32,
    /// 新歌单信息
    pub playlist: Playlist,
}

/// 列出指定歌单内的所有歌曲（支持跨存储引用）
///
/// # 流程
/// 1. `device.download_file(mem_unit, playlist_file_no)` → FIDL 二进制数据
/// 2. `parse_fidl(data)` → 条目列表（每个条目含 `rio_num = 歌曲文件号 + 0x4000`）
/// 3. 同时查询两个 mem_unit 的文件列表，各自按 file_no 建索引
/// 4. 遍历条目，先在歌单所在 mem_unit 查找，找不到再查另一个 mem_unit
///
/// # 跨存储引用
/// 真机实测：内置存储的歌单可以引用 SD 卡上的歌曲（反之亦然）。
/// sflags 字段不能用来判断跨存储（真机值如 `[7, 235, 198]` 无简单 0/1 规律），
/// 因此采用双存储搜索策略：先查歌单所在 mem_unit，找不到再查另一个。
/// 返回的 `PlaylistSong.mem_unit` 是歌曲实际所在的 mem_unit。
///
/// # 注意
/// RIO_FILEI 命令的 wIndex 是 0-based slot index，不是真实文件号。
/// 因此不能直接用 `get_file_info(mem_unit, rio_num)` 查询，
/// 必须先列出所有文件再在内存中按 file_no 匹配。
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `playlist_file_no`：歌单的文件号（从 `list_playlists` 获取）
/// - `mem_unit`：歌单所在内存单元（0=内置, 1=SD 卡）
///
/// # 返回
/// [`PlaylistSong`] 数组（按歌单内的顺序），每项含歌曲实际所在 mem_unit
pub async fn list_playlist_songs(
    device: &RioDevice,
    playlist_file_no: u32,
    mem_unit: u8,
) -> Result<Vec<PlaylistSong>> {
    // 1. 下载歌单文件内容（FIDL 二进制）
    let download = device.download_file(mem_unit, playlist_file_no, |_| {}).await?;
    log::info!(
        "list_playlist_songs: playlist file_no={}, mem_unit={}, FIDL data size={} bytes",
        playlist_file_no,
        mem_unit,
        download.data.len()
    );

    // 2. 解析 FIDL 条目
    let playlist = match parse_fidl(&download.data) {
        Ok(p) => {
            log::info!(
                "list_playlist_songs: FIDL parsed, nsongs={}, entries={}",
                p.entries.len(),
                p.entries.len()
            );
            for (i, e) in p.entries.iter().enumerate() {
                log::info!(
                    "  entry[{}]: rio_num=0x{:06x}, file_no={}, sflags={:?}",
                    i,
                    e.rio_num,
                    e.rio_num.saturating_sub(RIO_NUM_OFFSET),
                    e.sflags
                );
            }
            p
        }
        Err(e) => {
            log::warn!(
                "list_playlist_songs: FIDL parse failed: {}. First 16 bytes: {:?}",
                e,
                &download.data[..download.data.len().min(16)]
            );
            return Err(e);
        }
    };

    // 3. 同时查询两个 mem_unit 的文件列表，各自按 file_no 建索引
    //    真机实测：歌单可能跨存储引用（如内置歌单引用 SD 卡歌曲）。
    //
    //    关键发现：SD 卡的 file_no 已经包含 0x4000 偏移量（如 16416=0x4020），
    //    而内置存储的 file_no 不包含偏移（如 32=0x20）。
    //    因此查找时需同时尝试 rio_num 和 rio_num - RIO_NUM_OFFSET 两个键。
    let files_local = device.list_files(mem_unit, |_| {}).await?;
    log::info!(
        "list_playlist_songs: files_local (mem_unit={}) count={}, file_nos={:?}",
        mem_unit,
        files_local.len(),
        files_local.iter().map(|f| f.file_no).collect::<Vec<_>>()
    );
    let file_by_no_local: std::collections::HashMap<u32, &RioFile> =
        files_local.iter().map(|f| (f.file_no, f)).collect();

    let other_mem_unit = if mem_unit == 0 { 1 } else { 0 };
    let files_remote = match device.list_files(other_mem_unit, |_| {}).await {
        Ok(files) => {
            log::info!(
                "list_playlist_songs: files_remote (mem_unit={}) count={}, file_nos={:?}",
                other_mem_unit,
                files.len(),
                files.iter().map(|f| f.file_no).collect::<Vec<_>>()
            );
            files
        }
        Err(e) => {
            log::info!(
                "list_playlist_songs: cannot list files on mem_unit={} ({}), skipping remote lookup",
                other_mem_unit,
                e
            );
            Vec::new()
        }
    };
    let file_by_no_remote: std::collections::HashMap<u32, &RioFile> =
        files_remote.iter().map(|f| (f.file_no, f)).collect();

    // 4. 遍历歌单条目，先在歌单所在 mem_unit 查找，找不到再查另一个 mem_unit
    //    每个存储都尝试两个键：rio_num 和 rio_num - RIO_NUM_OFFSET
    //    （因为 SD 卡 file_no 含 0x4000 偏移，内置存储不含）
    let mut result = Vec::with_capacity(playlist.entries.len());
    for (i, entry) in playlist.entries.iter().enumerate() {
        let rio_num = entry.rio_num;
        let file_no_offset = rio_num.saturating_sub(RIO_NUM_OFFSET);

        let (file, song_mem_unit) = if let Some(f) = file_by_no_local.get(&file_no_offset) {
            (f, mem_unit)
        } else if let Some(f) = file_by_no_local.get(&rio_num) {
            (f, mem_unit)
        } else if let Some(f) = file_by_no_remote.get(&file_no_offset) {
            (f, other_mem_unit)
        } else if let Some(f) = file_by_no_remote.get(&rio_num) {
            (f, other_mem_unit)
        } else {
            log::warn!(
                "list_playlist_songs: entry[{}] rio_num=0x{:06x} (file_no={} or {}) not found in either mem_unit",
                i,
                rio_num,
                file_no_offset,
                rio_num
            );
            continue;
        };

        if !is_mp3_file(file) {
            log::warn!(
                "list_playlist_songs: entry[{}] file_no={} is not MP3 (type=0x{:x}), skipped",
                i,
                file.file_no,
                file.file_type
            );
            continue;
        }
        result.push(PlaylistSong {
            song: rio_file_to_song(file),
            index: i,
            mem_unit: song_mem_unit,
        });
    }
    log::info!(
        "list_playlist_songs: returning {} songs",
        result.len()
    );

    Ok(result)
}

/// 在设备上创建空歌单
///
/// 在指定内存单元上创建一个空的播放列表（`file_type = TYPE_PLS`）。
/// 内部上传一个 12 字节的空 FIDL 文件（header + 0 个条目）。
///
/// # 流程
/// 1. `serialize_fidl(FidlPlaylist::empty())` → 12B 空 FIDL
/// 2. `precheck_free_space(device, mem_unit, 12)` 存储空间预检
/// 3. 构造 `RioFile`：`file_type = TYPE_PLS`、`name/title = name`、`size = 12`
/// 4. `device.upload_file(mem_unit, header, fidl_data)` → 设备分配新 fileNo
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `name`：歌单名称（UTF-8 字符串）
/// - `mem_unit`：内存单元（0=内置, 1=SD 卡）
///
/// # 返回
/// [`CreatePlaylistResult`] 含新 fileNo 和歌单信息
pub async fn create_playlist(
    device: &RioDevice,
    name: &str,
    mem_unit: u8,
) -> Result<CreatePlaylistResult> {
    // 1. 构造空 FIDL 数据（12 字节头，nsongs=0）
    let fidl_data = serialize_fidl(&FidlPlaylist::empty());

    // 2. 存储空间预检
    precheck_free_space(device, mem_unit, fidl_data.len()).await?;

    // 3. 构造 rio_file_t 头
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    let header = RioFile {
        file_no: 0, // 设备分配
        start: 0,   // 设备分配
        size: fidl_data.len() as u32,
        time: 0,
        mod_date: now,
        bits: 0, // serialize_rio_file 自动置位（PLS 用 0x110，不设 bit 0）
        file_type: TYPE_PLS,
        sample_rate: 0,
        bit_rate: 0,
        name: name.to_string(),
        title: name.to_string(),
        artist: String::new(),
        album: String::new(),
    };

    // 4. 上传
    let new_file_no = device
        .upload_file(mem_unit, &header, &fidl_data, |_| {}, None)
        .await?;

    // 5. 返回结果
    Ok(CreatePlaylistResult {
        file_no: new_file_no,
        playlist: Playlist {
            file_no: new_file_no,
            size: fidl_data.len() as u32,
            name: name.to_string(),
            title: name.to_string(),
        },
    })
}

/// 将已存在的歌曲加入指定歌单（支持跨存储引用）
///
/// 下载歌单 PLS 文件 → 解析 FIDL → 追加歌曲条目 → 覆盖回设备。
/// 根据 `song_mem_unit` 与 `playlist_mem_unit` 是否相同自动设置 sflags。
///
/// # 流程（PROTOCOL.md §7 RIO_OVWRT 0x88）
/// 1. `device.download_file(playlist_mem_unit, playlist_file_no)` → `{ header, data: FIDL }`
/// 2. 计算 sflags：同存储 `[0,0,0]`，跨存储 `[0,0,1]`
/// 3. `append_to_fidl(data, rio_num, sflags)` → new FIDL 二进制
/// 4. `precheck_free_space(device, playlist_mem_unit, new_fid.len())` 存储空间预检
/// 5. `header.size = new_fid.len()`；`header.mod_date = now`
/// 6. `device.overwrite_file(playlist_mem_unit, playlist_file_no, header, new_fid, header_buffer)`
///
/// # 关键：S-Series 播放列表兼容性
/// `download_file` 返回的 `header_buffer` 含原始 2048B（保留所有未知字段如 0x78 `unk1[4]`），
/// 覆盖时必须回传原始 buffer，否则设备无法识别目标歌单。
///
/// # 跨存储引用
/// 原版 Windows 软件支持在内置存储歌单中引用 SD 卡歌曲。
/// 当 `song_mem_unit != playlist_mem_unit` 时，sflags 自动设为 `[0, 0, 1]`，
/// 设备固件据此从另一个 mem_unit 查找歌曲。
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `song_file_no`：要加入的歌曲文件号
/// - `song_mem_unit`：歌曲所在内存单元（0=内置, 1=SD 卡）
/// - `playlist_file_no`：目标歌单的文件号
/// - `playlist_mem_unit`：歌单所在内存单元（0=内置, 1=SD 卡）
pub async fn add_to_playlist(
    device: &RioDevice,
    song_file_no: u32,
    song_mem_unit: u8,
    playlist_file_no: u32,
    playlist_mem_unit: u8,
) -> Result<()> {
    // 根据 song_mem_unit 与 playlist_mem_unit 是否相同自动设置 sflags
    let sflags: [u8; 3] = if song_mem_unit == playlist_mem_unit {
        [0, 0, 0] // 同存储引用
    } else {
        [0, 0, 1] // 跨存储引用：sflags[2]=1 表示歌曲在另一个 mem_unit
    };
    add_to_playlist_with_sflags(
        device,
        song_file_no,
        song_mem_unit,
        playlist_file_no,
        playlist_mem_unit,
        sflags,
    )
    .await
}

/// 将歌曲加入歌单（带自定义 sflags）
///
/// 与 [`add_to_playlist`] 相同，但允许调用方直接指定 sflags。
/// 一般情况下用 [`add_to_playlist`] 即可，它会自动计算 sflags。
pub async fn add_to_playlist_with_sflags(
    device: &RioDevice,
    song_file_no: u32,
    song_mem_unit: u8,
    playlist_file_no: u32,
    mem_unit: u8,
    sflags: [u8; 3],
) -> Result<()> {
    // 1. 下载歌单文件内容（FIDL 二进制）+ 保留原 header buffer
    //    header_buffer 含原始 2048B（保留所有未知字段如 0x78 unk1[4] S-Series playlists），
    //    覆盖时必须回传原始 buffer，否则设备无法识别目标歌单
    let download = device
        .download_file(mem_unit, playlist_file_no, |_| {})
        .await?;
    let mut header = download.header;
    let header_buffer = download.header_buffer;
    let data = download.data;

    // 2. 追加歌曲条目到 FIDL
    //    FIDL 条目存的是 rio_num。关键：SD 卡的 file_no 已含 0x4000 偏移量，
    //    内置存储的 file_no 不含偏移量。
    //    - 内置存储 (mem_unit=0)：rio_num = file_no + 0x4000
    //    - SD 卡 (mem_unit=1)：rio_num = file_no（已经是 0x4000 + 实际编号）
    let rio_num = if song_mem_unit == 1 {
        song_file_no
    } else {
        song_file_no + RIO_NUM_OFFSET
    };
    let new_fid = append_to_fidl(&data, rio_num, sflags)?;

    // 3. 存储空间预检（覆盖时新内容可能比旧内容大）
    precheck_free_space(device, mem_unit, new_fid.len()).await?;

    // 4. 更新 header：size 改为新 FIDL 长度，modDate 更新为当前时间
    //    其他字段（fileNo/type/name/title/...）保持不变
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    header.size = new_fid.len() as u32;
    header.mod_date = now;

    // 5. 覆盖回设备（RIO_OVWRT 0x88，wIndex=0）
    //    传入 header_buffer 保留原始未知字段，仅 fileNo/size/modDate 被更新
    device
        .overwrite_file(
            mem_unit,
            playlist_file_no,
            &header,
            &new_fid,
            |_| {},
            Some(&*header_buffer),
        )
        .await
}

/// 从歌单中移除指定位置的歌曲
///
/// 流程：下载歌单 FIDL → 解析 → 移除第 `index` 个条目 → 重新序列化 → 覆盖回设备。
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `playlist_file_no`：歌单文件号
/// - `mem_unit`：歌单所在内存单元
/// - `index`：要移除的条目索引（0-based，来自 [`list_playlist_songs`] 返回的 `PlaylistSong.index`）
pub async fn remove_from_playlist(
    device: &RioDevice,
    playlist_file_no: u32,
    mem_unit: u8,
    index: usize,
) -> Result<()> {
    // 1. 下载歌单文件内容（FIDL 二进制）+ 保留原 header buffer
    let download = device
        .download_file(mem_unit, playlist_file_no, |_| {})
        .await?;
    let mut header = download.header;
    let header_buffer = download.header_buffer;
    let data = download.data;

    // 2. 解析 FIDL → 移除指定索引条目 → 重新序列化
    let mut playlist = parse_fidl(&data)?;
    if index >= playlist.entries.len() {
        return Err(crate::error::CyrioError::Other(format!(
            "remove_from_playlist: index {} out of range (entries={})",
            index,
            playlist.entries.len()
        )));
    }
    playlist.entries.remove(index);
    let new_fid = serialize_fidl(&playlist);

    // 3. 存储空间预检（覆盖时新内容可能比旧内容小，但仍需检查）
    precheck_free_space(device, mem_unit, new_fid.len()).await?;

    // 4. 更新 header：size 改为新 FIDL 长度，modDate 更新为当前时间
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    header.size = new_fid.len() as u32;
    header.mod_date = now;

    // 5. 覆盖回设备
    device
        .overwrite_file(
            mem_unit,
            playlist_file_no,
            &header,
            &new_fid,
            |_| {},
            Some(&*header_buffer),
        )
        .await
}

/// 修复歌单的编码（name/title + bits）
///
/// 用于修复旧版本软件创建的歌单：
/// - bit 0=1 的歌单：设备屏幕显示双重编码乱码
/// - name/title 字段被双重编码字节污染的歌单
///
/// 流程：下载歌单 → parse_rio_file 恢复正确 name → 覆盖回设备（修正 bits + 写入 UTF-8 name）
///
/// # 参数
/// - `device`：已打开的 RioDevice
/// - `playlist_file_no`：歌单文件号
/// - `mem_unit`：内存单元
pub async fn repair_playlist_encoding(
    device: &RioDevice,
    playlist_file_no: u32,
    mem_unit: u8,
) -> Result<()> {
    // 1. 下载歌单（获取 header + header_buffer + FIDL 数据）
    let download = device
        .download_file(mem_unit, playlist_file_no, |_| {})
        .await?;
    let mut header = download.header;
    let header_buffer = download.header_buffer;
    let data = download.data;

    // 2. 更新 mod_date（触发设备刷新）
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    header.mod_date = now;

    // 3. 覆盖回设备
    // overwrite_file 现在会用 parse_rio_file 解析后的正确 name/title 覆盖
    // header_buffer 中的双重编码字节，并清除 PLS 的 bit 0
    device
        .overwrite_file(
            mem_unit,
            playlist_file_no,
            &header,
            &data,
            |_| {},
            Some(&*header_buffer),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_song_struct_fields() {
        let s = Song {
            file_no: 48,
            size: 1024,
            time: 180,
            bit_rate: 128,
            sample_rate: 44100,
            name: "test.mp3".to_string(),
            title: "Test".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
        };
        let ps = PlaylistSong {
            song: s.clone(),
            index: 3,
            mem_unit: 0,
        };
        assert_eq!(ps.song.file_no, 48);
        assert_eq!(ps.index, 3);
        assert_eq!(ps.mem_unit, 0);
    }

    #[test]
    fn create_playlist_result_fields() {
        let r = CreatePlaylistResult {
            file_no: 224,
            playlist: Playlist {
                file_no: 224,
                size: 12,
                name: "TESTING".to_string(),
                title: "TESTING".to_string(),
            },
        };
        assert_eq!(r.file_no, 224);
        assert_eq!(r.playlist.name, "TESTING");
    }
}
