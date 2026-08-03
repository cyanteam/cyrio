//! Rio S-Series USB 协议常量定义
//!
//! 集中存放所有与 Rio S-Series（S10/S30S/S35S/S50）USB 通信相关的魔数、
//! 端点地址、操作码、二进制结构偏移量等常量。所有数值均来自对 rioutil
//! （GitHub: hjelmn/rioutil）源码的逆向分析，并与原厂 RIOUNIV.SYS 驱动
//! (v2.4.0.22) 的 INF 描述交叉验证。
//!
//! # 字节序约定
//! USB 线上传输的所有多字节整数均为**小端**（little-endian）。
//! 本文件中的偏移量以字节为单位，从结构体起始算起。
//!
//! # 来源
//! 移植自 NodeJS 项目 `rio-rs/node/src/protocol/constants.ts`。
//! 与原厂资料交叉验证详见 `docs/PROTOCOL.md` 附录 A。

// ============================================================================
// USB 设备识别
// ============================================================================

/// Diamond / DNNA (Digital Networks North America) 的 USB Vendor ID
pub const VENDOR_DIAMOND: u16 = 0x045a;

/// Rio S10 产品 ID
pub const PRODUCT_RIO_S10: u16 = 0x5005;

/// Rio S50 产品 ID（用户当前设备：S30S 刷 S50 固件后报告此 PID）
pub const PRODUCT_RIO_S50: u16 = 0x5006;

/// Rio S35S 产品 ID
pub const PRODUCT_RIO_S35S: u16 = 0x5007;

/// Rio S30S 产品 ID（用户硬件原始 PID）
pub const PRODUCT_RIO_S30S: u16 = 0x5009;

/// 当前实现支持的 PID 列表（用于设备扫描时的自动识别）
pub const SUPPORTED_PIDS: &[u16] = &[
    PRODUCT_RIO_S10,
    PRODUCT_RIO_S50,
    PRODUCT_RIO_S35S,
    PRODUCT_RIO_S30S,
];

/// Rio S-Series 分代标识（generation 4）
pub const RIO_GENERATION_S_SERIES: u8 = 4;

// ============================================================================
// USB 端点与配置
// ============================================================================

/// 要设置的 USB 配置号（rioutil 硬编码为 1）
pub const USB_CONFIG: u8 = 1;

/// 要 claim 的 USB 接口号
pub const USB_INTERFACE: u8 = 0;

/// Bulk IN 端点地址（端点 1，bit7=1 表示 IN）
pub const EP_IN: u8 = 0x81;

/// Bulk OUT 端点地址（端点 2，bit7=0 表示 OUT）
pub const EP_OUT: u8 = 0x02;

/// Vendor Control Transfer 的 bmRequestType（Vendor, Device→Host, IN 方向）
pub const CONTROL_REQUEST_TYPE: u8 = 0xc0;

/// Control Transfer 的数据阶段长度（12 字节状态缓冲区）
pub const CONTROL_STATUS_LENGTH: usize = 0x0c;

/// Control Transfer 超时（毫秒）
pub const CONTROL_TIMEOUT_MS: u64 = 15000;

/// Bulk 传输超时（毫秒）
pub const BULK_TIMEOUT_MS: u64 = 8000;

/// send_command 成功标志：返回的状态缓冲区首字节必须为 0x01
pub const COMMAND_SUCCESS_BYTE: u8 = 0x01;

/// send_command 失败时的最大重试次数（与 rioutil 一致）
pub const COMMAND_MAX_RETRIES: u32 = 3;

/// 重试之间的间隔（毫秒）
pub const COMMAND_RETRY_DELAY_MS: u64 = 50;

// ============================================================================
// 块大小
// ============================================================================

/// 握手包大小（CRIODATA/CRIOINFO/CRIOABRT/SRIORDY/SRIODATA 等控制包都是 64 字节）
pub const PKT_HANDSHAKE: usize = 64;

/// 文件头大小（rio_file_t 结构体，2048 字节）
pub const PKT_HEADER: usize = 2048;

/// S-Series 数据传输块大小（16KB，rioutil 中 RIO_FTS = 0x4000）
pub const PKT_BLOCK: usize = 16384;

/// rio_file_t / 偏好等 2KB 结构体的块大小（rioutil 中 RIO_MTS = 0x800）
pub const PKT_META: usize = 2048;

// ============================================================================
// 操作码（Opcode）
// ============================================================================

/// 初始化握手 / 上传完成收尾
pub const OP_UNKNOWN00: u8 = 0x60;

/// 轮询设备状态
pub const OP_RIO_POLLD: u8 = 0x61;

/// 读取设备描述（256B，含固件版本、序列号、型号名）
pub const OP_RIO_DESCP: u8 = 0x62;

/// 查询设备支持的文件类型
pub const OP_RIO_TYPEQ: u8 = 0x63;

/// open_rio 中发送一次，目的不明但必需
pub const OP_UNKNOWN65: u8 = 0x65;

/// 读取内存单元信息（256B，含 size/used/free/name）。arg1=memory_unit
pub const OP_RIO_MEMRI: u8 = 0x68;

/// 读取文件信息（2048B，rio_file_t）。arg1=memory_unit, arg2=file_no
pub const OP_RIO_FILEI: u8 = 0x69;

/// 格式化内存单元。arg1=memory_unit
pub const OP_RIO_FORMT: u8 = 0x6a;

/// 固件/ROM 升级（本库不实现）
pub const OP_RIO_UPDAT: u8 = 0x6b;

/// 上传新文件。arg1=memory_unit, arg2=0（设备分配新 file_no）
pub const OP_RIO_WRITE: u8 = 0x6c;

/// 下载文件。arg1=memory_unit, arg2=0（S-Series 原生支持，不删除原文件）
pub const OP_RIO_READF: u8 = 0x70;

/// 删除文件。arg1=memory_unit, arg2=0
pub const OP_RIO_DELET: u8 = 0x78;

/// 写设备偏好（2048B，音量/均衡/重复模式等）
pub const OP_RIO_PREFS: u8 = 0x79;

/// 读设备偏好（2048B）
pub const OP_RIO_PREFR: u8 = 0x7a;

/// 设置设备时钟。arg1=Unix 时间戳高 16 位, arg2=低 16 位
pub const OP_RIO_TIMES: u8 = 0x7b;

/// Group ID 查询（本库不用）
pub const OP_RIO_GIDS: u8 = 0x7c;

/// S-Series+：修改已存在文件的元信息
pub const OP_RIO_CHGIN: u8 = 0x85;

/// Nitrus 专用：发送歌曲数据库信息（本库不用）
pub const OP_RIO_NINFO: u8 = 0x87;

/// S-Series+：覆盖已存在的文件。arg1=memory_unit, arg2=已存在的 file_no
pub const OP_RIO_OVWRT: u8 = 0x88;

// ============================================================================
// 握手魔数（Host → Device）
// ============================================================================

/// 数据块前导包魔数（8B 魔数 + 4B CRC32 + 52B 填充，后接 16384B 数据）
pub const MAGIC_CRIODATA: &[u8] = b"CRIODATA";

/// 文件头前导包魔数（8B 魔数 + 4B 0x00000000 + 52B 填充，后接 2048B 头）
pub const MAGIC_CRIOINFO: &[u8] = b"CRIOINFO";

/// 中止传输包魔数（8B 魔数 + 56B 0）
pub const MAGIC_CRIOABRT: &[u8] = b"CRIOABRT";

// ============================================================================
// 握手魔数（Device → Host）
// ============================================================================

/// 设备就绪
pub const MAGIC_SRIORDY: &[u8] = b"SRIORDY";

/// 数据块确认
pub const MAGIC_SRIODATA: &[u8] = b"SRIODATA";

/// 传输完成
pub const MAGIC_SRIODONE: &[u8] = b"SRIODONE";

/// 文件不存在
pub const MAGIC_SRIONOFL: &[u8] = b"SRIONOFL";

/// 删除开始确认
pub const MAGIC_SRIODELS: &[u8] = b"SRIODELS";

/// 删除完成
pub const MAGIC_SRIODELD: &[u8] = b"SRIODELD";

/// 格式化完成
pub const MAGIC_SRIOFMTD: &[u8] = b"SRIOFMTD";

// ============================================================================
// 文件类型 FourCC（rio_file_t.type 字段，小端 u32）
// ============================================================================

/// MP3 文件（"MPG3" LE = 0x4d504733）
pub const TYPE_MP3: u32 = 0x4d504733;

/// Windows Media Audio（"WMA " LE）
pub const TYPE_WMA: u32 = 0x20414d57;

/// WAV 文件（"ACLP" LE，Rio 内部码）
pub const TYPE_WAV: u32 = 0x504c4341;

/// WAV 文件（备用码 "WAVE" LE）
pub const TYPE_WAVE: u32 = 0x45564157;

/// 播放列表文件（"PLS " LE，FIDL/ST10 格式）
pub const TYPE_PLS: u32 = 0x504c5320;

// ============================================================================
// rio_file_t 结构体偏移量（共 2048 字节）
// ============================================================================

/// rio_file_t 总大小
pub const RIO_FILE_SIZE: usize = 2048;

/// u32: 文件编号（由设备分配，1-based）
pub const OFF_FILE_NO: usize = 0x0000;

/// u32: 文件数据在闪存中的起始偏移（0 = 由设备分配）
pub const OFF_START: usize = 0x0004;

/// u32: 文件大小（字节数）
pub const OFF_SIZE: usize = 0x0008;

/// u32: 时长（秒）
pub const OFF_TIME: usize = 0x000c;

/// u32: 修改时间（Unix 时间戳，秒）
pub const OFF_MOD_DATE: usize = 0x0010;

/// u32: 标志位（bits）
pub const OFF_BITS: usize = 0x0014;

/// u32: 文件类型 FourCC（TYPE_MP3 / TYPE_PLS 等）
pub const OFF_TYPE: usize = 0x0018;

/// u32: 采样率（Hz，如 44100）
pub const OFF_SAMPLE_RATE: usize = 0x0024;

/// u32: 比特率（实际 kbps × 128，即 << 7）
pub const OFF_BIT_RATE: usize = 0x0028;

/// char[64]: 文件名（latin1 + NUL 填充）
pub const OFF_NAME: usize = 0x00c0;

/// char[64]: 标题
pub const OFF_TITLE: usize = 0x0100;

/// char[64]: 艺术家
pub const OFF_ARTIST: usize = 0x0140;

/// char[64]: 专辑
pub const OFF_ALBUM: usize = 0x0180;

/// 字符串字段的标准长度（name/title/artist/album 均为 64 字节）
pub const RIO_STRING_LEN: usize = 64;

// ============================================================================
// bits 标志位（rio_file_t.bits 字段）
// ============================================================================

/// 必须置位的三位组合（序列化时强制 `bits |= BITS_REQUIRED`）
pub const BITS_REQUIRED: u32 = 0x00000001 | 0x00000010 | 0x00000100; // 0x111

/// 文件可下载（仅 gen3 需要；S-Series 全部可下载）
pub const BITS_DOWNLOADABLE: u32 = 0x00000080;

/// 文件名不在设备界面显示
pub const BITS_HIDE_NAME: u32 = 0x00400000;

// ============================================================================
// rio_mem_t 结构体偏移量（共 256 字节）
// ============================================================================

/// rio_mem_t 总大小
pub const RIO_MEM_SIZE: usize = 256;

/// u32[4]: 保留/未知字段
pub const OFF_MEM_FOO: usize = 0x0000;

/// u32: 内存单元总大小（S-Series 单位是字节）
pub const OFF_MEM_SIZE: usize = 0x0010;

/// u32: 已用字节数
pub const OFF_MEM_USED: usize = 0x0014;

/// u32: 空闲字节数
pub const OFF_MEM_FREE: usize = 0x0018;

/// u32: 系统保留字节数
pub const OFF_MEM_SYSTEM: usize = 0x001c;

/// char[64]: 内存单元名（如 "Internal Memory"）
pub const OFF_MEM_NAME: usize = 0x0040;

/// char[64]: 型号字符串
pub const OFF_MEM_MODEL: usize = 0x00c0;

// ============================================================================
// 内存单元编号
// ============================================================================

/// 内存单元 0：内置闪存
pub const MEM_UNIT_INTERNAL: u8 = 0;

/// 内存单元 1：SD/MMC 卡
pub const MEM_UNIT_SDCARD: u8 = 1;

/// 最大内存单元数（内置 + SD 卡）
pub const MAX_MEM_UNITS: u8 = 2;

/// 文件号起始值（1-based，0 表示空槽/不存在）
pub const FILE_NO_MIN: u32 = 1;

/// 文件号上限（rioutil 中 MAX_RIO_FILES = 3000）
pub const FILE_NO_MAX: u32 = 3000;

/// `list_files` 遇到连续多少个空槽才停止扫描
///
/// 旧版遇第一个空槽就 break，会漏列 wIndex=0 bug 创建的残留文件（0 秒音频）。
///
/// 设为 200：删除操作后 slot 表可能出现大段连续空槽（如 slot 13-212 为空，
/// 后面 slot 213+ 还有歌曲）。16 太小会漏掉空槽后面的所有歌曲
/// （实测：131 首歌因中间 16 个连续空槽只显示 12 首）。
/// 200 在 USB 慢速场景下最多多扫 200×USB RTT（约 1-4 秒），可接受。
pub const LIST_FILES_EMPTY_GAP: u32 = 200;

/// FIDL 条目中 rio_num 与 file_no 的偏移量
///
/// FIDL 播放列表二进制中每个条目存 3B rio_num（小端），其值 = file_no + 0x4000。
/// 例如 file_no=144 对应 rio_num=0x4090=16528。
pub const RIO_NUM_OFFSET: u32 = 0x4000;

// ============================================================================
// FIDL/ST10 播放列表格式偏移量
// ============================================================================

/// 头部魔数 "FIDL"（4 字节）
pub const FIDL_MAGIC: &[u8] = b"FIDL";

/// 子类型 "ST"（2 字节）
pub const FIDL_SUBTYPE: &[u8] = b"ST";

/// 主版本号（1 字节，固定为 0x01）
pub const FIDL_VERSION_MAJOR: u8 = 0x01;

/// 次版本号（1 字节，固定为 0x00）
pub const FIDL_VERSION_MINOR: u8 = 0x00;

/// 头部总长度（4 + 2 + 1 + 1 + 1 + 3 = 12 字节）
pub const FIDL_HEADER_SIZE: usize = 12;

/// 单个条目长度（3 字节 rio_num + 3 字节 sflags = 6 字节）
pub const FIDL_ENTRY_SIZE: usize = 6;

/// rio_num 字段在条目内的偏移
pub const FIDL_OFF_RIO_NUM: usize = 0;

/// sflags 字段在条目内的偏移
pub const FIDL_OFF_SFLAGS: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_pids_contains_s50() {
        assert!(SUPPORTED_PIDS.contains(&PRODUCT_RIO_S50));
        assert_eq!(SUPPORTED_PIDS.len(), 4);
    }

    #[test]
    fn magic_lengths_are_8_bytes() {
        assert_eq!(MAGIC_CRIODATA.len(), 8);
        assert_eq!(MAGIC_CRIOINFO.len(), 8);
        assert_eq!(MAGIC_CRIOABRT.len(), 8);
        assert_eq!(MAGIC_SRIORDY.len(), 7);
        assert_eq!(MAGIC_SRIODONE.len(), 8);
    }

    #[test]
    fn type_mp3_value_matches_rioutil() {
        // rioutil 中 TYPE_MP3 = 0x4d504733（"MPG3" ASCII 拼接为 u32）
        assert_eq!(TYPE_MP3, 0x4d504733);
        // 验证 FourCC 拼接：'M' 'P' 'G' '3' 作为 u32 big-endian
        let fourcc = u32::from_be_bytes(*b"MPG3");
        assert_eq!(fourcc, TYPE_MP3);
    }

    #[test]
    fn rio_num_offset_is_0x4000() {
        assert_eq!(RIO_NUM_OFFSET, 0x4000);
        // file_no=144 → rio_num=16528 (0x4090)
        assert_eq!(144 + RIO_NUM_OFFSET, 0x4090);
    }

    #[test]
    fn bits_required_is_0x111() {
        assert_eq!(BITS_REQUIRED, 0x111);
    }
}
