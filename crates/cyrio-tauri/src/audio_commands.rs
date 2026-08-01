//! 音频播放命令：薄包装层
//!
//! 把 [`cyrio_audio::manager`] 的 `AudioState` 包装成 Tauri 命令。
//! 实际播放管理逻辑（专用线程、命令通道、共享原子状态）都在
//! `cyrio_audio::manager` 中，本模块只做 Tauri 命令绑定。

use cyrio_audio::manager::{start_audio_thread, AudioState, PlaybackState};

use crate::commands::DeviceState;

/// 全局音频状态别名
///
/// 实际类型在 `cyrio_audio::manager::AudioState`，这里 re-export 让
/// Tauri 的 `.manage()` 调用方不需要知道完整路径。
pub type SharedAudioState = AudioState;

/// 启动音频线程，返回 AudioState（供 `.manage()`）
pub fn start_audio() -> SharedAudioState {
    start_audio_thread()
}

/// 播放歌曲（从设备下载到内存 → 发送到音频线程）
///
/// 下载期间 is_loading=true（前端显示"加载中"），下载完发送 Play 后 is_loading=false。
/// 先持 device lock 下载文件，**下载完立即释放锁**（块作用域），再发 channel。
/// 避免音频播放期间阻塞设备锁导致 keepAlive 失败。
#[tauri::command]
pub async fn play_song(
    device_state: tauri::State<'_, DeviceState>,
    audio_state: tauri::State<'_, AudioState>,
    file_no: u32,
    mem_unit: u8,
) -> Result<(), String> {
    // 标记正在下载（前端显示"加载中"）
    audio_state.set_loading(true);

    // 1. 从设备下载到内存（块作用域释放 lock）
    let result = {
        let guard = device_state.device.lock().await;
        let device = guard.as_ref().ok_or_else(|| {
            audio_state.set_loading(false);
            "设备未连接".to_string()
        })?;
        device
            .download_file(mem_unit, file_no, |_| {})
            .await
            .map_err(|e| {
                audio_state.set_loading(false);
                e.to_string()
            })?
    };

    // 2. 下载完成，清除 loading 状态
    audio_state.set_loading(false);

    // 3. 发送到音频线程播放（音频线程会先 Stop 当前播放再 Play）
    audio_state.play(result.data)
}

/// 暂停播放
#[tauri::command]
pub async fn pause_audio(audio_state: tauri::State<'_, AudioState>) -> Result<(), String> {
    audio_state.pause()
}

/// 继续播放
#[tauri::command]
pub async fn resume_audio(audio_state: tauri::State<'_, AudioState>) -> Result<(), String> {
    audio_state.resume()
}

/// 停止播放
#[tauri::command]
pub async fn stop_audio(audio_state: tauri::State<'_, AudioState>) -> Result<(), String> {
    audio_state.stop()
}

/// 查询播放状态
#[tauri::command]
pub async fn get_playback_state(
    audio_state: tauri::State<'_, AudioState>,
) -> Result<PlaybackState, String> {
    Ok(audio_state.state())
}
