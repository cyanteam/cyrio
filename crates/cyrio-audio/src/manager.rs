//! 音频播放管理器：专用线程 + 命令通道 + 共享原子状态
//!
//! RodioPlayer 持 `OutputStream`（!Send），必须放专用线程。
//! 调用方通过 `Sender<AudioCommand>` 发命令到音频线程。
//! 状态查询通过 `Arc<Atomic*>` 共享，position 用 `Instant` 估算
//! （rodio 0.20 无 position API）。
//!
//! # 架构
//! ```text
//! 调用方（UI/命令层）         音频线程（线程本地 OutputStream）
//! ─────────────────           ──────────────────────────────
//! AudioState.play(data)  ──>  rx.recv() -> Play(data)
//!                             player.play(data)
//! AudioState.state()  <──    AtomicBool/AtomicU32 共享
//! ```
//!
//! 单曲状态：play() 会先 stop() 当前播放再播新的，避免 Sink 状态冲突。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde::Serialize;

use crate::create_player;

/// 音频线程命令
#[derive(Debug)]
pub enum AudioCommand {
    /// 播放 MP3 数据（已下载到内存）
    Play(Vec<u8>),
    /// 暂停
    Pause,
    /// 继续播放
    Resume,
    /// 停止
    Stop,
    /// 退出音频线程（应用关闭时使用）
    Quit,
}

/// 播放状态（返回给 UI 层）
#[derive(Debug, Clone, Serialize)]
pub struct PlaybackState {
    /// 是否正在播放
    pub is_playing: bool,
    /// 当前播放位置（秒）
    pub position: f32,
    /// 总时长（秒）
    pub duration: f32,
    /// 是否正在下载/加载（play_song 下载期间为 true）
    pub is_loading: bool,
}

/// 音频管理状态
///
/// 持有命令发送端 + 共享原子状态，供调用方查询播放进度和发送命令。
/// 通过 [`start_audio_thread`] 创建。
pub struct AudioState {
    /// 命令发送端
    tx: Sender<AudioCommand>,
    /// 是否正在播放
    is_playing: Arc<AtomicBool>,
    /// 总时长（毫秒，×1000 存 u32 避免 f32 的 Send 问题）
    duration_ms: Arc<AtomicU32>,
    /// 播放起始时刻（暂停时存 None）
    play_start: Arc<Mutex<Option<Instant>>>,
    /// 已累计的播放毫秒数（暂停时累加，恢复时重置起点）
    accumulated_ms: Arc<AtomicU32>,
    /// 是否正在下载（play_song 下载期间为 true，UI 显示"加载中"）
    is_loading: Arc<AtomicBool>,
}

/// 启动音频线程，返回 [`AudioState`]
///
/// 在专用 std::thread 内创建 RodioPlayer（OutputStream 线程本地），
/// 通过 mpsc::channel 接收命令，空闲时阻塞在 `recv()`。
pub fn start_audio_thread() -> AudioState {
    let (tx, rx) = mpsc::channel::<AudioCommand>();
    let is_playing = Arc::new(AtomicBool::new(false));
    let duration_ms = Arc::new(AtomicU32::new(0));
    let play_start = Arc::new(Mutex::new(None::<Instant>));
    let accumulated_ms = Arc::new(AtomicU32::new(0));
    let is_loading = Arc::new(AtomicBool::new(false));

    let is_playing_t = is_playing.clone();
    let duration_ms_t = duration_ms.clone();
    let play_start_t = play_start.clone();
    let accumulated_ms_t = accumulated_ms.clone();

    thread::Builder::new()
        .name("cyrio-audio".into())
        .spawn(move || {
            // 在此线程内创建 RodioPlayer（OutputStream 线程本地）
            let mut player = match create_player() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("音频线程启动失败: {}", e);
                    return;
                }
            };
            // 命令循环
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    AudioCommand::Play(data) => {
                        // 先停止当前播放，避免 Sink 状态冲突（切换歌曲时关键）
                        player.stop();
                        *play_start_t.lock().unwrap() = None;
                        accumulated_ms_t.store(0, Ordering::Relaxed);
                        is_playing_t.store(false, Ordering::Relaxed);

                        match player.play(data) {
                            Ok(()) => {
                                is_playing_t.store(true, Ordering::Relaxed);
                                let dur = player.duration();
                                duration_ms_t.store((dur * 1000.0) as u32, Ordering::Relaxed);
                                accumulated_ms_t.store(0, Ordering::Relaxed);
                                *play_start_t.lock().unwrap() = Some(Instant::now());
                            }
                            Err(e) => log::error!("播放失败: {}", e),
                        }
                    }
                    AudioCommand::Pause => {
                        player.pause();
                        if let Some(start) = play_start_t.lock().unwrap().take() {
                            let elapsed = start.elapsed().as_millis() as u32;
                            accumulated_ms_t.fetch_add(elapsed, Ordering::Relaxed);
                        }
                        is_playing_t.store(false, Ordering::Relaxed);
                    }
                    AudioCommand::Resume => {
                        player.resume();
                        *play_start_t.lock().unwrap() = Some(Instant::now());
                        is_playing_t.store(true, Ordering::Relaxed);
                    }
                    AudioCommand::Stop => {
                        player.stop();
                        *play_start_t.lock().unwrap() = None;
                        accumulated_ms_t.store(0, Ordering::Relaxed);
                        is_playing_t.store(false, Ordering::Relaxed);
                    }
                    AudioCommand::Quit => break,
                }
            }
        })
        .expect("spawn audio thread");

    AudioState {
        tx,
        is_playing,
        duration_ms,
        play_start,
        accumulated_ms,
        is_loading,
    }
}

impl AudioState {
    /// 播放 MP3 数据（先 stop 当前播放再 play 新的，单曲状态）
    pub fn play(&self, data: Vec<u8>) -> Result<(), String> {
        self.tx
            .send(AudioCommand::Play(data))
            .map_err(|_| "音频线程已关闭".to_string())
    }

    /// 暂停播放
    pub fn pause(&self) -> Result<(), String> {
        self.tx
            .send(AudioCommand::Pause)
            .map_err(|_| "音频线程已关闭".to_string())
    }

    /// 继续播放
    pub fn resume(&self) -> Result<(), String> {
        self.tx
            .send(AudioCommand::Resume)
            .map_err(|_| "音频线程已关闭".to_string())
    }

    /// 停止播放
    pub fn stop(&self) -> Result<(), String> {
        self.tx
            .send(AudioCommand::Stop)
            .map_err(|_| "音频线程已关闭".to_string())
    }

    /// 设置加载状态（下载期间为 true，UI 显示"加载中"）
    pub fn set_loading(&self, loading: bool) {
        self.is_loading.store(loading, Ordering::Relaxed);
    }

    /// 查询播放状态
    ///
    /// position = accumulated_ms + (当前段，若正在播放)。
    /// rodio 0.20 无 position API，用 Instant 估算，精度约 ±50ms。
    pub fn state(&self) -> PlaybackState {
        let is_playing = self.is_playing.load(Ordering::Relaxed);
        let is_loading = self.is_loading.load(Ordering::Relaxed);
        let duration = self.duration_ms.load(Ordering::Relaxed) as f32 / 1000.0;
        let mut position_ms = self.accumulated_ms.load(Ordering::Relaxed);
        if is_playing {
            if let Some(start) = *self.play_start.lock().unwrap() {
                position_ms += start.elapsed().as_millis() as u32;
            }
        }
        PlaybackState {
            is_playing,
            position: position_ms as f32 / 1000.0,
            duration,
            is_loading,
        }
    }

    /// 退出音频线程（应用关闭时调用）
    pub fn quit(&self) -> Result<(), String> {
        self.tx
            .send(AudioCommand::Quit)
            .map_err(|_| "音频线程已关闭".to_string())
    }
}
