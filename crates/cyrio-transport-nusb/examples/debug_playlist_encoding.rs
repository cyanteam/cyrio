//! 调试工具：读取设备上所有 PLS 歌单的原始 header 字节，对比编码
//!
//! 用法：
//!   cargo run --package cyrio-transport-nusb --example debug_playlist_encoding
//!     扫描并显示所有 PLS 歌单的编码状态
//!
//!   cargo run --package cyrio-transport-nusb --example debug_playlist_encoding -- --repair
//!     扫描并修复所有损坏的歌单（bit 0=1 或 name 字节被双重编码）

use cyrio_core::api::device::RioDevice;
use cyrio_core::api::playlist::repair_playlist_encoding;
use cyrio_core::protocol::constants::{
    EP_IN, OFF_BITS, OFF_FILE_NO, OFF_NAME, OFF_TITLE, OFF_TYPE, PKT_HEADER,
    RIO_FILE_SIZE, RIO_STRING_LEN, TYPE_PLS,
};
use cyrio_core::protocol::rio_file::parse_rio_file;
use cyrio_transport_nusb::NusbTransport;

/// 扫描到的 PLS 歌单条目
struct PlaylistEntry {
    file_no: u32,
    slot: u32,
    bits: u32,
    name_bytes: Vec<u8>,
    title_bytes: Vec<u8>,
    parsed_name: String,
    parsed_title: String,
}

impl PlaylistEntry {
    /// bit 0 是否开启（设备会做 latin1→UTF-8 双重编码）
    fn bit0_on(&self) -> bool {
        self.bits & 0x01 != 0
    }

    /// name 字节是否被双重编码污染
    ///
    /// 双重编码特征：原始 UTF-8 字节（如 0xE6）被当作 latin1 字符（æ=U+00E6）
    /// 再编码为 UTF-8（0xC3 0xA6）。因此解码后所有非 ASCII 字符都落在
    /// latin1 范围 (U+0080-U+00FF)。
    fn name_is_double_encoded(&self) -> bool {
        if self.name_bytes.is_empty() {
            return false;
        }
        let Ok(s) = std::str::from_utf8(&self.name_bytes) else {
            return false; // 无效 UTF-8 不是双重编码
        };
        // 必须含非 ASCII 字符（纯 ASCII 不是双重编码）
        let has_non_ascii = s.chars().any(|c| !c.is_ascii());
        if !has_non_ascii {
            return false;
        }
        // 所有非 ASCII 字符都在 latin1 范围 → 双重编码特征
        s.chars()
            .all(|c| c.is_ascii() || ((c as u32) >= 0x80 && (c as u32) <= 0xFF))
    }

    /// 是否需要修复
    fn needs_repair(&self) -> bool {
        self.bit0_on() || self.name_is_double_encoded()
    }

    /// 修复原因描述
    fn repair_reason(&self) -> String {
        let mut reasons = Vec::new();
        if self.bit0_on() {
            reasons.push("bit 0=1 (设备会双重编码)".to_string());
        }
        if self.name_is_double_encoded() {
            reasons.push("name 字节被双重编码污染".to_string());
        }
        reasons.join(", ")
    }
}

fn hex(bytes: &[u8], max_len: usize) -> String {
    bytes
        .iter()
        .take(max_len)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn try_utf8(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => "(无效 UTF-8)".to_string(),
    }
}

fn try_gbk(bytes: &[u8]) -> String {
    let (cow, had_errors) = encoding_rs::GBK.decode_without_bom_handling(bytes);
    if had_errors {
        format!("(GBK 解码有错误) {}", cow.into_owned())
    } else {
        cow.into_owned()
    }
}

fn try_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// 扫描指定 mem_unit 的所有 PLS 歌单
async fn scan_playlists(device: &RioDevice, mem_unit: u8) -> Vec<PlaylistEntry> {
    let mut entries = Vec::new();

    for slot in 0..3000u32 {
        if let Err(e) = device
            .send_command(0x69, mem_unit as u16, slot as u16)
            .await
        {
            if slot > 0 {
                println!("  send_command 错误 slot={}: {}", slot, e);
            }
            break;
        }

        let buf = match device.transport.bulk_in(EP_IN, PKT_HEADER).await {
            Ok(b) => b,
            Err(e) => {
                if slot > 0 {
                    println!("  bulk_in 错误 slot={}: {}", slot, e);
                }
                break;
            }
        };

        if buf.len() < RIO_FILE_SIZE {
            break;
        }

        let file_no = u32::from_le_bytes([
            buf[OFF_FILE_NO],
            buf[OFF_FILE_NO + 1],
            buf[OFF_FILE_NO + 2],
            buf[OFF_FILE_NO + 3],
        ]);
        if file_no == 0 {
            break;
        }

        let file_type = u32::from_le_bytes([
            buf[OFF_TYPE],
            buf[OFF_TYPE + 1],
            buf[OFF_TYPE + 2],
            buf[OFF_TYPE + 3],
        ]);

        if file_type != TYPE_PLS {
            continue;
        }

        let bits = u32::from_le_bytes([
            buf[OFF_BITS],
            buf[OFF_BITS + 1],
            buf[OFF_BITS + 2],
            buf[OFF_BITS + 3],
        ]);

        let name_slice = &buf[OFF_NAME..OFF_NAME + RIO_STRING_LEN];
        let name_end = name_slice.iter().position(|&b| b == 0).unwrap_or(RIO_STRING_LEN);
        let name_bytes = name_slice[..name_end].to_vec();

        let title_slice = &buf[OFF_TITLE..OFF_TITLE + RIO_STRING_LEN];
        let title_end = title_slice.iter().position(|&b| b == 0).unwrap_or(RIO_STRING_LEN);
        let title_bytes = title_slice[..title_end].to_vec();

        let (parsed_name, parsed_title) = match parse_rio_file(&buf) {
            Ok(p) => (p.name, p.title),
            Err(e) => {
                println!("  [PLS #{}] parse_rio_file 错误: {}", file_no, e);
                (String::new(), String::new())
            }
        };

        entries.push(PlaylistEntry {
            file_no,
            slot,
            bits,
            name_bytes,
            title_bytes,
            parsed_name,
            parsed_title,
        });
    }

    entries
}

/// 打印单个歌单的详细编码信息
fn print_playlist(entry: &PlaylistEntry) {
    println!(
        "\n  [PLS #{}] slot={} bits=0x{:08x}",
        entry.file_no, entry.slot, entry.bits
    );
    println!(
        "    bit 0 (双重编码开关): {}",
        if entry.bit0_on() {
            "开 (会双重编码)"
        } else {
            "关 (原样返回)"
        }
    );
    println!("    name hex:    {}", hex(&entry.name_bytes, 32));
    println!("    name utf8:   \"{}\"", try_utf8(&entry.name_bytes));
    println!("    name gbk:    \"{}\"", try_gbk(&entry.name_bytes));
    println!("    name latin1: \"{}\"", try_latin1(&entry.name_bytes));
    println!("    title hex:   {}", hex(&entry.title_bytes, 32));
    println!("    title utf8:  \"{}\"", try_utf8(&entry.title_bytes));
    println!("    parse_rio_file name:  \"{}\"", entry.parsed_name);
    println!("    parse_rio_file title: \"{}\"", entry.parsed_title);
    if entry.needs_repair() {
        println!("    [WARN] 需要修复: {}", entry.repair_reason());
    } else {
        println!("    [OK] 编码正常");
    }
}

/// 列出模式：扫描并显示所有歌单
async fn list_mode(device: &RioDevice) {
    println!("=== PLS 歌单编码对比 ===\n");

    for (mem_unit, label) in [(0u8, "内置存储"), (1u8, "SD 卡")] {
        println!("\n=== {} (memUnit={}) 所有 PLS 歌单 ===", label, mem_unit);
        let entries = scan_playlists(device, mem_unit).await;
        for e in &entries {
            print_playlist(e);
        }
        println!("\n  共 {} 个歌单", entries.len());
    }
}

/// 修复模式：扫描并修复所有损坏的歌单
async fn repair_mode(device: &RioDevice) {
    println!("=== PLS 歌单编码修复 ===\n");

    let mut total_repaired = 0usize;
    let mut total_failed = 0usize;

    for (mem_unit, label) in [(0u8, "内置存储"), (1u8, "SD 卡")] {
        println!("\n--- 扫描 {} (memUnit={}) ---", label, mem_unit);
        let entries = scan_playlists(device, mem_unit).await;
        println!("  发现 {} 个歌单", entries.len());

        let corrupted: Vec<&PlaylistEntry> =
            entries.iter().filter(|e| e.needs_repair()).collect();
        if corrupted.is_empty() {
            println!("  [OK] 没有需要修复的歌单");
            continue;
        }

        println!("  发现 {} 个损坏的歌单：", corrupted.len());
        for e in &corrupted {
            println!(
                "    - PLS #{}: \"{}\" ({})",
                e.file_no,
                e.parsed_name,
                e.repair_reason()
            );
        }

        for e in &corrupted {
            print!("\n  修复 PLS #{} (\"{}\")... ", e.file_no, e.parsed_name);
            match repair_playlist_encoding(device, e.file_no, mem_unit).await {
                Ok(()) => {
                    println!("[OK] 成功");
                    total_repaired += 1;
                }
                Err(err) => {
                    println!("[FAIL] 失败: {}", err);
                    total_failed += 1;
                }
            }
        }
    }

    println!("\n=== 修复完成 ===");
    println!("成功修复: {} 个", total_repaired);
    println!("修复失败: {} 个", total_failed);

    if total_repaired > 0 {
        println!("\n建议重新运行本工具（不带 --repair）验证修复结果。");
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let is_repair = std::env::args().any(|a| a == "--repair");

    // 启动 smol 全局执行器
    std::thread::Builder::new()
        .name("smol-executor".into())
        .spawn(|| {
            smol::block_on(smol::future::pending::<()>());
        })
        .expect("spawn smol executor thread");

    smol::block_on(async {
        let transport = match NusbTransport::open().await {
            Ok(t) => Box::new(t),
            Err(e) => {
                eprintln!("打开设备失败: {}", e);
                std::process::exit(1);
            }
        };

        let mut device = RioDevice::new(transport);
        if let Err(e) = device.open().await {
            eprintln!("设备初始化失败: {}", e);
            std::process::exit(1);
        }

        if is_repair {
            repair_mode(&device).await;
        } else {
            list_mode(&device).await;
        }

        println!("\n=== 完成 ===");
    });
}
