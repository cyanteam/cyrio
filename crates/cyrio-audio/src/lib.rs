//! # cyrio-audio
//!
//! 音频播放抽象层 + MP3 帧头解析 + 播放管理器。
//!
//! 平台分支：
//! - 桌面（Windows/macOS/Linux）：`rodio`（基于 cpal）
//! - Web（WASM）：web-sys Web Audio API
//!
//! MP3 帧头解析（[`parse_mp3_info`]）是平台无关的，桌面和 Web 都能用。
//!
//! 播放管理器（[`manager`]）提供专用音频线程 + 命令通道 + 共享原子状态，
//! 让 UI 层（egui/Tauri）都能复用同一份播放管理逻辑。

#![warn(missing_docs)]

pub mod manager;

use thiserror::Error;

/// 音频错误
#[derive(Debug, Error)]
pub enum AudioError {
    /// 解码错误
    #[error("decode error: {0}")]
    Decode(String),
    /// 播放错误
    #[error("playback error: {0}")]
    Playback(String),
}

// ============================================================================
// MP3 帧头解析（PROTOCOL.md §16.x，参考 rioutil + ID3v2）
// ============================================================================

/// MP3 帧头解析结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mp3Info {
    /// 估计时长（秒）
    pub duration: u32,
    /// 采样率（Hz）
    pub sample_rate: u32,
    /// 比特率（kbps）
    pub bit_rate: u32,
    /// MPEG 层（1/2/3）
    pub layer: u8,
    /// 通道数（1=单声道, 2=立体声）
    pub channels: u8,
}

/// MPEG 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MpegVersion {
    /// MPEG Version 1 (ISO/IEC 11172-3)
    V1,
    /// MPEG Version 2 (ISO/IEC 13818-3)
    V2,
    /// MPEG Version 2.5 (non-official)
    V25,
}

/// 解析 MP3 数据，估算时长、采样率、比特率
///
/// 跳过 ID3v2 头，遍历所有 MP3 帧头，累加帧时长得到总时长。
/// 第一帧的采样率/比特率作为整体代表（CBR 假设）。
///
/// # 参数
/// - `data`：MP3 文件字节数（含 ID3v2 头）
///
/// # 返回
/// `Some(Mp3Info)` 或 `None`（无有效 MP3 帧）
pub fn parse_mp3_info(data: &[u8]) -> Option<Mp3Info> {
    // 1. 跳过 ID3v2
    let start = skip_id3v2(data);
    let body = &data[start..];

    // 2. 找第一个有效帧头
    let mut pos = 0;
    let mut first_info: Option<Mp3Info> = None;
    let mut total_duration_us: u64 = 0; // 微秒
    let mut frames_count: u32 = 0;

    while pos + 4 <= body.len() {
        // 找帧同步字 0xFFEx（11 位 sync）
        if body[pos] != 0xFF || (body[pos + 1] & 0xE0) != 0xE0 {
            pos += 1;
            continue;
        }

        let hdr = &body[pos..pos + 4];
        let b1 = hdr[1];
        let b2 = hdr[2];
        let b3 = hdr[3];

        // MPEG 版本
        let ver_bits = (b1 >> 3) & 0x03;
        let version = match ver_bits {
            0 => MpegVersion::V25,
            2 => MpegVersion::V2,
            3 => MpegVersion::V1,
            _ => {
                pos += 1;
                continue;
            }
        };

        // MPEG 层
        let layer_bits = (b1 >> 1) & 0x03;
        let layer = match layer_bits {
            1 => 3, // Layer III
            2 => 2, // Layer II
            3 => 1, // Layer I
            _ => {
                pos += 1;
                continue;
            }
        };

        // 比特率索引
        let bitrate_idx = (b2 >> 4) & 0x0F;
        if bitrate_idx == 0 || bitrate_idx == 15 {
            pos += 1;
            continue;
        }
        let bit_rate = match (version, layer, bitrate_idx) {
            // MPEG 1 Layer I
            (MpegVersion::V1, 1, i) => bitrate_v1_l1(i)?,
            // MPEG 1 Layer II
            (MpegVersion::V1, 2, i) => bitrate_v1_l2(i)?,
            // MPEG 1 Layer III
            (MpegVersion::V1, 3, i) => bitrate_v1_l3(i)?,
            // MPEG 2/2.5 Layer I
            (MpegVersion::V2 | MpegVersion::V25, 1, i) => bitrate_v2_l1(i)?,
            // MPEG 2/2.5 Layer II/III
            (MpegVersion::V2 | MpegVersion::V25, 2 | 3, i) => bitrate_v2_l23(i)?,
            _ => {
                pos += 1;
                continue;
            }
        };

        // 采样率索引
        let sr_idx = (b2 >> 2) & 0x03;
        if sr_idx == 3 {
            pos += 1;
            continue;
        }
        let sample_rate = match (version, sr_idx) {
            (MpegVersion::V1, 0) => 44100,
            (MpegVersion::V1, 1) => 48000,
            (MpegVersion::V1, 2) => 32000,
            (MpegVersion::V2, 0) => 22050,
            (MpegVersion::V2, 1) => 24000,
            (MpegVersion::V2, 2) => 16000,
            (MpegVersion::V25, 0) => 11025,
            (MpegVersion::V25, 1) => 12000,
            (MpegVersion::V25, 2) => 8000,
            _ => {
                pos += 1;
                continue;
            }
        };

        // padding
        let padding = if (b2 >> 1) & 0x01 == 1 { 1 } else { 0 };

        // 通道
        let channel_mode = (b3 >> 6) & 0x03;
        let channels = if channel_mode == 3 { 1 } else { 2 };

        // 帧长度（字节）
        // Layer I:  帧 = (12 * bitRate / sampleRate + padding) * 4
        // Layer II/III: 帧 = 144 * bitRate / sampleRate + padding
        let frame_size = if layer == 1 {
            (12 * bit_rate * 1000 / sample_rate + padding) * 4
        } else {
            144 * bit_rate * 1000 / sample_rate + padding
        };

        // 帧时长（微秒）
        // Layer I: 384 samples / sampleRate
        // Layer II/III: 1152 samples / sampleRate (V1) 或 576 (V2/V25)
        let samples_per_frame = if layer == 1 {
            384
        } else if version == MpegVersion::V1 {
            1152
        } else {
            576
        };
        let frame_duration_us = (samples_per_frame as u64) * 1_000_000 / sample_rate as u64;

        if first_info.is_none() {
            first_info = Some(Mp3Info {
                duration: 0,
                sample_rate,
                bit_rate,
                layer,
                channels,
            });
        }
        total_duration_us += frame_duration_us;
        frames_count += 1;

        // 跳到下一帧
        let frame_size_us = frame_size as usize;
        if frame_size_us == 0 || pos + frame_size_us > body.len() {
            break;
        }
        pos += frame_size_us;
    }

    if frames_count == 0 {
        return None;
    }

    let mut info = first_info?;
    info.duration = (total_duration_us / 1_000_000) as u32;
    Some(info)
}

/// 跳过 ID3v2 头，返回音频数据起始偏移
fn skip_id3v2(data: &[u8]) -> usize {
    if data.len() < 10 {
        return 0;
    }
    if &data[0..3] != b"ID3" {
        return 0;
    }
    let size = ((data[6] as usize & 0x7f) << 21)
        | ((data[7] as usize & 0x7f) << 14)
        | ((data[8] as usize & 0x7f) << 7)
        | (data[9] as usize & 0x7f);
    if size > 16 * 1024 * 1024 {
        return 0;
    }
    10 + size
}

// 比特率表（kbps），返回 None 表示无效
fn bitrate_v1_l1(i: u8) -> Option<u32> {
    const T: [u32; 16] = [
        0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0,
    ];
    Some(T[i as usize])
}
fn bitrate_v1_l2(i: u8) -> Option<u32> {
    const T: [u32; 16] = [
        0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0,
    ];
    Some(T[i as usize])
}
fn bitrate_v1_l3(i: u8) -> Option<u32> {
    const T: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    Some(T[i as usize])
}
fn bitrate_v2_l1(i: u8) -> Option<u32> {
    const T: [u32; 16] = [
        0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0,
    ];
    Some(T[i as usize])
}
fn bitrate_v2_l23(i: u8) -> Option<u32> {
    const T: [u32; 16] = [
        0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
    ];
    Some(T[i as usize])
}

// ============================================================================
// 音频播放器 trait（用于桌面 rodio / Web WebAudio 实现）
// ============================================================================

/// 音频播放器 trait
///
/// 实现者负责解码 MP3 数据并播放，提供进度查询与控制。
/// 不要求 `Send`：rodio 的 `OutputStream` 是线程本地的。
pub trait AudioPlayer {
    /// 播放 MP3 数据（解码后播）
    fn play(&mut self, mp3_data: Vec<u8>) -> Result<(), AudioError>;

    /// 暂停
    fn pause(&mut self);

    /// 继续播放
    fn resume(&mut self);

    /// 停止
    fn stop(&mut self);

    /// 跳转到指定位置（秒）
    fn seek(&mut self, seconds: f32);

    /// 当前播放位置（秒）
    fn position(&self) -> f32;

    /// 总时长（秒）
    fn duration(&self) -> f32;

    /// 是否正在播放
    fn is_playing(&self) -> bool;
}

/// 创建平台特定的播放器
///
/// - 桌面（非 Android）：rodio 实现
/// - Web：Web Audio API 实现（TODO）
/// - Android：不支持（USB 设备本身有播放能力）
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn create_player() -> Result<Box<dyn AudioPlayer>, AudioError> {
    RodioPlayer::new().map(|p| Box::new(p) as Box<dyn AudioPlayer>)
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
pub fn create_player() -> Result<Box<dyn AudioPlayer>, AudioError> {
    Err(AudioError::Playback(
        "当前平台不支持本地音频播放".into(),
    ))
}

// ============================================================================
// 桌面 rodio 实现（仅非 WASM 且非 Android）
// ============================================================================

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod rodio_player {
    use super::{AudioError, AudioPlayer};
    use rodio::Sink;
    use std::io::Cursor;
    use std::sync::Mutex;

    /// rodio 实现的 AudioPlayer
    ///
    /// 简化版：用 parse_mp3_info 估算时长，rodio Sink 控制 play/pause/stop。
    /// position 暂未实现（rodio 0.20 无直接 API）。
    pub struct RodioPlayer {
        sink: Mutex<Option<Sink>>,
        _stream: rodio::OutputStream,
        total_duration: std::sync::atomic::AtomicU32,
    }

    impl RodioPlayer {
        /// 创建播放器（获取默认输出设备）
        pub fn new() -> Result<Self, AudioError> {
            let (stream, handle) =
                rodio::OutputStream::try_default().map_err(|e| AudioError::Playback(e.to_string()))?;
            let sink = rodio::Sink::try_new(&handle).map_err(|e| AudioError::Playback(e.to_string()))?;
            // stream/handle 必须保活；handle 已被 Sink 引用，stream 用 forget 保活
            std::mem::forget(handle);
            Ok(Self {
                sink: Mutex::new(Some(sink)),
                _stream: stream,
                total_duration: std::sync::atomic::AtomicU32::new(0),
            })
        }
    }

    impl AudioPlayer for RodioPlayer {
        fn play(&mut self, mp3_data: Vec<u8>) -> Result<(), AudioError> {
            // 先用 parse_mp3_info 估算时长
            if let Some(info) = crate::parse_mp3_info(&mp3_data) {
                self.total_duration
                    .store(info.duration, std::sync::atomic::Ordering::Relaxed);
            }

            let cursor = Cursor::new(mp3_data);
            let decoder = rodio::Decoder::new(cursor)
                .map_err(|e| AudioError::Decode(e.to_string()))?;

            let guard = self.sink.lock().unwrap();
            let sink = guard.as_ref().expect("sink");
            // 清空当前播放队列，追加新解码数据
            sink.clear();
            sink.append(decoder);
            // 关键：stop() 会调用 pause()，新 append 后必须显式 play() 才能恢复播放
            // 否则切歌后 sink 处于 paused 状态，用户听不到声音
            sink.play();
            Ok(())
        }

        fn pause(&mut self) {
            if let Some(sink) = self.sink.lock().unwrap().as_ref() {
                sink.pause();
            }
        }

        fn resume(&mut self) {
            if let Some(sink) = self.sink.lock().unwrap().as_ref() {
                sink.play();
            }
        }

        fn stop(&mut self) {
            // 用 clear 代替 stop，保持 sink 可复用
            if let Some(sink) = self.sink.lock().unwrap().as_ref() {
                sink.clear();
                sink.pause();
            }
        }

        fn seek(&mut self, _seconds: f32) {
            // rodio 0.20 的 Sink::set_position 在新版本可用，但 API 不稳定，简化版不实现
        }

        fn position(&self) -> f32 {
            // rodio 0.20 没有直接提供 position，简化版返回 0
            0.0
        }

        fn duration(&self) -> f32 {
            self.total_duration.load(std::sync::atomic::Ordering::Relaxed) as f32
        }

        fn is_playing(&self) -> bool {
            self.sink
                .lock()
                .unwrap()
                .as_ref()
                .map(|s| !s.is_paused())
                .unwrap_or(false)
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub use rodio_player::RodioPlayer;

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_id3v2_returns_zero_for_no_tag() {
        let buf = [0u8; 100];
        assert_eq!(skip_id3v2(&buf), 0);
    }

    #[test]
    fn skip_id3v2_parses_valid_tag() {
        let mut buf = [0u8; 30];
        buf[0] = b'I';
        buf[1] = b'D';
        buf[2] = b'3';
        buf[6] = 0;
        buf[7] = 0;
        buf[8] = 0;
        buf[9] = 5;
        assert_eq!(skip_id3v2(&buf), 15);
    }

    #[test]
    fn parse_mp3_info_returns_none_for_empty() {
        assert!(parse_mp3_info(&[]).is_none());
    }

    #[test]
    fn parse_mp3_info_returns_none_for_random_data() {
        let buf = [0u8; 1000];
        assert!(parse_mp3_info(&buf).is_none());
    }

    #[test]
    fn parse_mp3_info_parses_simple_frame() {
        // 构造一个最小的 MPEG1 Layer III 128kbps 44100Hz 帧
        // 帧头: 0xFF 0xFB 0x90 0x00
        //   0xFF = sync
        //   0xFB = 1111 1011 -> MPEG1 (11), Layer III (01), no CRC (1)
        //   0x90 = 1001 0000 -> bitrate idx 9 (128kbps), sample rate idx 0 (44100), no padding
        //   0x00 = 0000 0000 -> channel mode 00 (stereo)
        let mut buf = vec![0u8; 4 + 417]; // 帧头 + 帧数据（128kbps/44100Hz -> 417 字节）
        buf[0] = 0xFF;
        buf[1] = 0xFB;
        buf[2] = 0x90;
        buf[3] = 0x00;
        // 计算预期帧长度：144 * 128000 / 44100 = 417.6 -> 417
        let info = parse_mp3_info(&buf);
        assert!(info.is_some(), "should parse MP3 info");
        let info = info.unwrap();
        assert_eq!(info.bit_rate, 128);
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.layer, 3);
        assert_eq!(info.channels, 2);
        // 单帧时长：1152 / 44100 = 0.026s
        assert_eq!(info.duration, 0);
    }

    #[test]
    fn parse_mp3_info_handles_multiple_frames() {
        // 两帧 MPEG1 Layer III 128kbps 44100Hz
        let frame_size = 417;
        let mut buf = vec![0u8; frame_size * 2];
        for i in 0..2 {
            buf[i * frame_size] = 0xFF;
            buf[i * frame_size + 1] = 0xFB;
            buf[i * frame_size + 2] = 0x90;
            buf[i * frame_size + 3] = 0x00;
        }
        let info = parse_mp3_info(&buf).expect("should parse");
        // 两帧 × 0.026s = 0.052s，向下取整 = 0
        assert_eq!(info.duration, 0);
    }
}
