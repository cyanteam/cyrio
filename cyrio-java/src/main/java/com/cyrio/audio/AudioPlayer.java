package com.cyrio.audio;

import javafx.animation.AnimationTimer;
import javafx.application.Platform;
import javafx.scene.media.Media;
import javafx.scene.media.MediaPlayer;
import javafx.util.Duration;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/**
 * 音频播放器（基于 JavaFX MediaPlayer）
 *
 * <p>提供 MP3/WAV/AAC 等格式的内存播放与文件播放能力，支持播放控制
 * （暂停/恢复/停止/跳转/音量）以及状态与位置回调。
 *
 * <h3>线程模型</h3>
 * <ul>
 *   <li>JavaFX 要求 {@link Media} 和 {@link MediaPlayer} 必须在 JavaFX Application Thread 上创建</li>
 *   <li>本类通过 {@link Platform#runLater} 将创建操作调度到 FX 线程</li>
 *   <li>{@code play} / {@code playFile} 方法会阻塞调用线程，直到 MediaPlayer 创建完成（最多 10 秒超时）</li>
 *   <li>若调用线程本身就是 FX 线程，则直接同步执行，避免死锁</li>
 *   <li>播放状态通过 volatile 字段 + FX 线程监听器维护，{@link #getPlaybackState()} 可在任意线程调用</li>
 * </ul>
 *
 * <h3>支持的格式</h3>
 * JavaFX MediaPlayer 原生支持 MP3、WAV（PCM）、AAC（M4A/MP4 容器）等。
 * 通过 {@code play(byte[], String format)} 播放内存数据时，会根据 format 参数
 * 选择临时文件扩展名，确保 JavaFX 能正确识别容器格式。
 *
 * <h3>临时文件清理</h3>
 * {@code play(byte[])} 会将数据写入系统临时目录，在 {@link #stop()} 或
 * {@link #dispose()} 时自动删除。
 *
 * <p>对应 Rust 项目 {@code cyrio-audio/src/lib.rs} 的 {@code RodioPlayer}。
 * Rust 版使用 rodio（基于 cpal），Java 版使用 JavaFX MediaPlayer。
 */
public class AudioPlayer {

    // ========================================================================
    // 播放状态枚举
    // ========================================================================

    /**
     * 播放状态枚举
     *
     * <p>简化自 JavaFX {@link MediaPlayer.Status}，只保留三种用户可见状态：
     * <ul>
     *   <li>{@link #STOPPED} — 已停止或未加载（对应 FX 的 STOPPED / READY / UNKNOWN / HALTED / DISPOSED）</li>
     *   <li>{@link #PLAYING} — 正在播放（对应 FX 的 PLAYING）</li>
     *   <li>{@link #PAUSED} — 已暂停（对应 FX 的 PAUSED / STALLED）</li>
     * </ul>
     */
    public enum State {
        /** 已停止（未加载 / 已停止 / 已结束） */
        STOPPED,
        /** 正在播放 */
        PLAYING,
        /** 已暂停 */
        PAUSED
    }

    // ========================================================================
    // PlaybackState 内部类
    // ========================================================================

    /**
     * 播放状态快照
     *
     * <p>不可变值对象，包含当前状态、播放位置、总时长和音量。
     * 由 {@link #getPlaybackState()} 返回，也在状态变化回调中传递。
     */
    public static class PlaybackState {

        /** 当前播放状态 */
        public final State state;

        /** 当前播放位置（秒） */
        public final double positionSeconds;

        /** 总时长（秒），未知时为 0 */
        public final double totalSeconds;

        /** 当前音量（0.0 ~ 1.0） */
        public final double volume;

        /**
         * 创建播放状态快照
         *
         * @param state           播放状态
         * @param positionSeconds 当前位置（秒）
         * @param totalSeconds    总时长（秒）
         * @param volume          音量（0.0~1.0）
         */
        public PlaybackState(State state, double positionSeconds, double totalSeconds, double volume) {
            this.state = state;
            this.positionSeconds = positionSeconds;
            this.totalSeconds = totalSeconds;
            this.volume = volume;
        }

        @Override
        public String toString() {
            return String.format("PlaybackState{state=%s, pos=%.1fs, total=%.1fs, vol=%.0f%%}",
                    state, positionSeconds, totalSeconds, volume * 100);
        }
    }

    // ========================================================================
    // 常量
    // ========================================================================

    /** play() 方法等待 MediaPlayer 创建的超时时间（秒） */
    private static final long CREATE_TIMEOUT_SECONDS = 10;

    /** 位置回调的帧间隔（约 30fps，每帧约 33ms） */
    private static final long POSITION_UPDATE_INTERVAL_NANOS = 33_000_000;

    // ========================================================================
    // 字段
    // ========================================================================

    /** JavaFX MediaPlayer 实例（仅在 FX 线程上访问） */
    private volatile MediaPlayer player;

    /** 当前播放状态（volatile，可在任意线程读取） */
    private volatile State state = State.STOPPED;

    /** 当前音量（0.0~1.0，volatile 可在任意线程读写） */
    private volatile double volume = 1.0;

    /** 当前播放位置（秒，由 FX 线程的 AnimationTimer 更新） */
    private volatile double positionSeconds = 0.0;

    /** 总时长（秒，由 FX 线程在 READY 时更新） */
    private volatile double totalSeconds = 0.0;

    /** 上一次位置回调的时间戳（纳秒），用于节流 */
    private volatile long lastPositionUpdateNanos = 0;

    /** 临时文件路径（play(byte[]) 创建，stop/dispose 时删除） */
    private volatile Path tempFile;

    /** 位置轮询定时器（FX 线程上运行） */
    private volatile AnimationTimer positionTimer;

    /** 状态变化回调 */
    private volatile Consumer<PlaybackState> onStateChanged;

    /** 位置变化回调 */
    private volatile Consumer<Double> onPositionChanged;

    /** 播放结束回调 */
    private volatile Runnable onEndOfMedia;

    // ========================================================================
    // 回调设置
    // ========================================================================

    /**
     * 设置播放状态变化回调
     *
     * <p>回调在 JavaFX Application Thread 上触发。状态变化包括：
     * STOPPED → PLAYING（开始播放）、PLAYING → PAUSED（暂停）、
     * PAUSED → PLAYING（恢复）、* → STOPPED（停止/结束）。
     *
     * @param callback 状态变化回调，传 null 取消
     */
    public void setOnStateChanged(Consumer<PlaybackState> callback) {
        this.onStateChanged = callback;
    }

    /**
     * 设置播放位置变化回调
     *
     * <p>回调在 JavaFX Application Thread 上触发，约每 33ms（~30fps）一次。
     * 参数为当前播放位置（秒）。
     *
     * @param callback 位置变化回调，传 null 取消
     */
    public void setOnPositionChanged(Consumer<Double> callback) {
        this.onPositionChanged = callback;
    }

    /**
     * 设置播放结束回调
     *
     * <p>当音频播放到结尾时触发。回调在 JavaFX Application Thread 上执行。
     *
     * @param callback 播放结束回调，传 null 取消
     */
    public void setOnEndOfMedia(Runnable callback) {
        this.onEndOfMedia = callback;
    }

    // ========================================================================
    // 播放控制
    // ========================================================================

    /**
     * 从内存数据播放音频
     *
     * <p>将 {@code audioData} 写入系统临时文件，然后用 JavaFX MediaPlayer 播放。
     * 临时文件在 {@link #stop()} 或 {@link #dispose()} 时自动删除。
     *
     * <p>JavaFX MediaPlayer 支持的格式：MP3、WAV（PCM）、AAC（M4A/MP4）等。
     * {@code format} 参数用于确定临时文件扩展名，确保 JavaFX 能正确识别容器。
     *
     * @param audioData 音频字节数据
     * @param format    格式标识（"mp3"、"wav"、"aac"、"m4a" 等，不区分大小写）
     * @throws IOException        临时文件写入失败
     * @throws RuntimeException   MediaPlayer 创建失败或超时
     */
    public void play(byte[] audioData, String format) throws IOException {
        if (audioData == null || audioData.length == 0) {
            throw new IllegalArgumentException("audioData 不能为空");
        }

        // 根据格式选择扩展名
        String ext = formatToExtension(format);

        // 写入临时文件
        Path temp = Files.createTempFile("cyrio-audio-", ext);
        try {
            Files.write(temp, audioData);
        } catch (IOException e) {
            deleteTempFile(temp);
            throw e;
        }

        // 清理上一次的临时文件和播放器
        cleanupPrevious();

        this.tempFile = temp;
        playFileInternal(temp);
    }

    /**
     * 播放本地文件
     *
     * @param file 要播放的音频文件路径
     * @throws RuntimeException MediaPlayer 创建失败或超时
     */
    public void playFile(Path file) {
        if (file == null || !Files.exists(file)) {
            throw new IllegalArgumentException("文件不存在: " + file);
        }

        // 清理上一次的临时文件和播放器
        cleanupPrevious();
        this.tempFile = null;

        playFileInternal(file);
    }

    /**
     * 内部播放实现：在 FX 线程上创建 MediaPlayer 并开始播放
     *
     * <p>使用 CountDownLatch 阻塞调用线程直到 MediaPlayer 创建完成。
     * 若调用线程本身就是 FX 线程，则直接同步执行。
     */
    private void playFileInternal(Path file) {
        // 重置状态
        this.positionSeconds = 0.0;
        this.totalSeconds = 0.0;
        updateState(State.STOPPED);

        if (Platform.isFxApplicationThread()) {
            // 调用线程就是 FX 线程，直接执行
            createAndPlay(file);
        } else {
            // 调度到 FX 线程，等待完成
            CountDownLatch latch = new CountDownLatch(1);
            AtomicReference<RuntimeException> errorRef = new AtomicReference<>();

            Platform.runLater(() -> {
                try {
                    createAndPlay(file);
                } catch (RuntimeException e) {
                    errorRef.set(e);
                } finally {
                    latch.countDown();
                }
            });

            try {
                if (!latch.await(CREATE_TIMEOUT_SECONDS, TimeUnit.SECONDS)) {
                    throw new RuntimeException("MediaPlayer 创建超时（" + CREATE_TIMEOUT_SECONDS + "s）");
                }
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                throw new RuntimeException("播放被中断", e);
            }

            // 如果 FX 线程上发生了异常，重新抛出
            RuntimeException err = errorRef.get();
            if (err != null) {
                throw err;
            }
        }
    }

    /**
     * 在 FX 线程上创建 Media + MediaPlayer 并开始播放
     *
     * <p>此方法必须在 JavaFX Application Thread 上调用。
     */
    private void createAndPlay(Path file) {
        // 构造 file:// URL（JavaFX Media 需要 URL 格式）
        String url = file.toUri().toString();

        Media media;
        try {
            media = new Media(url);
        } catch (Exception e) {
            throw new RuntimeException("创建 Media 失败: " + url, e);
        }

        MediaPlayer mp = new MediaPlayer(media);

        // 设置音量（保持当前音量）
        mp.setVolume(this.volume);

        // === 状态监听 ===
        mp.statusProperty().addListener((obs, oldStatus, newStatus) -> {
            mapAndNotifyState(newStatus);
        });

        // === 媒体就绪：获取总时长 ===
        mp.setOnReady(() -> {
            Duration total = mp.getTotalDuration();
            if (total != null && !total.isUnknown()) {
                this.totalSeconds = total.toSeconds();
            }
            // READY 状态映射为 STOPPED
            mapAndNotifyState(MediaPlayer.Status.READY);
        });

        // === 播放结束 ===
        mp.setOnEndOfMedia(() -> {
            this.positionSeconds = this.totalSeconds;
            updateState(State.STOPPED);
            stopPositionTimer();
            Runnable cb = this.onEndOfMedia;
            if (cb != null) {
                cb.run();
            }
        });

        // === 错误处理 ===
        mp.setOnError(() -> {
            String errMsg = media.getError() != null ? media.getError().getMessage()
                    : (mp.getError() != null ? mp.getError().getMessage() : "未知错误");
            updateState(State.STOPPED);
            stopPositionTimer();
            throw new RuntimeException("MediaPlayer 错误: " + errMsg);
        });

        // 保存播放器引用
        this.player = mp;

        // 开始播放（JavaFX 会自动缓冲，READY 后开始输出音频）
        mp.play();

        // 启动位置轮询定时器
        startPositionTimer();
    }

    // ========================================================================
    // 暂停 / 恢复 / 停止
    // ========================================================================

    /**
     * 暂停播放
     *
     * <p>若当前未在播放，此方法无效果。
     */
    public void pause() {
        MediaPlayer mp = this.player;
        if (mp != null) {
            runOnFxThread(() -> mp.pause());
        }
    }

    /**
     * 恢复播放
     *
     * <p>若当前未暂停，此方法无效果。
     */
    public void resume() {
        MediaPlayer mp = this.player;
        if (mp != null) {
            runOnFxThread(() -> mp.play());
        }
    }

    /**
     * 停止播放并重置位置到开头
     *
     * <p>停止后会清理临时文件。播放器实例保留，可再次调用 play() 播放新内容。
     */
    public void stop() {
        MediaPlayer mp = this.player;
        if (mp != null) {
            runOnFxThread(() -> {
                mp.stop();
                stopPositionTimer();
            });
        }
        this.positionSeconds = 0.0;
        updateState(State.STOPPED);

        // 清理临时文件
        deleteTempFile(this.tempFile);
        this.tempFile = null;
    }

    // ========================================================================
    // 音量 / 跳转
    // ========================================================================

    /**
     * 设置音量
     *
     * @param volume 音量，范围 0.0（静音）~ 1.0（最大），超出范围会被截断
     */
    public void setVolume(double volume) {
        double clamped = Math.max(0.0, Math.min(1.0, volume));
        this.volume = clamped;

        MediaPlayer mp = this.player;
        if (mp != null) {
            runOnFxThread(() -> mp.setVolume(clamped));
        }

        // 通知状态变化（音量变了）
        notifyStateChanged();
    }

    /**
     * 跳转到指定位置
     *
     * @param seconds 目标位置（秒），超出 [0, 总时长] 范围会被截断
     */
    public void seek(double seconds) {
        double clamped = Math.max(0.0, seconds);
        if (this.totalSeconds > 0) {
            clamped = Math.min(clamped, this.totalSeconds);
        }

        MediaPlayer mp = this.player;
        if (mp != null) {
            final double finalClamped = clamped;
            runOnFxThread(() -> {
                mp.seek(Duration.seconds(finalClamped));
                this.positionSeconds = finalClamped;
            });
        } else {
            this.positionSeconds = clamped;
        }
    }

    // ========================================================================
    // 状态查询
    // ========================================================================

    /**
     * 获取当前播放状态快照
     *
     * @return 包含状态、位置、总时长、音量的 {@link PlaybackState}
     */
    public PlaybackState getPlaybackState() {
        return new PlaybackState(this.state, this.positionSeconds, this.totalSeconds, this.volume);
    }

    /**
     * 获取当前播放状态枚举
     *
     * @return 当前状态（{@link State#STOPPED} / {@link State#PLAYING} / {@link State#PAUSED}）
     */
    public State getState() {
        return this.state;
    }

    /**
     * 获取当前播放位置（秒）
     *
     * @return 当前位置，未加载时为 0
     */
    public double getPositionSeconds() {
        return this.positionSeconds;
    }

    /**
     * 获取总时长（秒）
     *
     * @return 总时长，未知时为 0
     */
    public double getTotalSeconds() {
        return this.totalSeconds;
    }

    /**
     * 获取当前音量
     *
     * @return 音量（0.0~1.0）
     */
    public double getVolume() {
        return this.volume;
    }

    /**
     * 是否正在播放
     *
     * @return {@code true} 表示当前处于 PLAYING 状态
     */
    public boolean isPlaying() {
        return this.state == State.PLAYING;
    }

    // ========================================================================
    // 资源释放
    // ========================================================================

    /**
     * 释放所有资源
     *
     * <p>停止播放、清理临时文件、dispose MediaPlayer。
     * 调用后此实例不再可用。
     */
    public void dispose() {
        MediaPlayer mp = this.player;
        if (mp != null) {
            runOnFxThread(() -> {
                stopPositionTimer();
                mp.stop();
                mp.dispose();
            });
            this.player = null;
        }
        deleteTempFile(this.tempFile);
        this.tempFile = null;
        updateState(State.STOPPED);
    }

    // ========================================================================
    // 内部工具方法
    // ========================================================================

    /**
     * 将格式字符串转换为文件扩展名
     *
     * @param format 格式标识（mp3/wav/aac/m4a 等），null 或空则默认 .mp3
     * @return 带 "." 前缀的扩展名
     */
    private static String formatToExtension(String format) {
        if (format == null || format.isBlank()) {
            return ".mp3";
        }
        String lower = format.trim().toLowerCase();
        return switch (lower) {
            case "mp3", "mpeg" -> ".mp3";
            case "wav", "wave" -> ".wav";
            case "aac" -> ".aac";
            case "m4a", "mp4", "aac-m4a" -> ".m4a";
            case "flv" -> ".flv";
            default -> "." + lower.replaceAll("[^a-z0-9]", "");
        };
    }

    /**
     * 将 JavaFX MediaPlayer.Status 映射到本类的 State 枚举并通知回调
     */
    private void mapAndNotifyState(MediaPlayer.Status fxStatus) {
        if (fxStatus == null) {
            return;
        }
        State newState = switch (fxStatus) {
            case PLAYING -> State.PLAYING;
            case PAUSED, STALLED -> State.PAUSED;
            // READY / STOPPED / UNKNOWN / HALTED / DISPOSED → STOPPED
            default -> State.STOPPED;
        };
        updateState(newState);
    }

    /**
     * 更新状态并触发回调（在 FX 线程上调用）
     */
    private void updateState(State newState) {
        State oldState = this.state;
        this.state = newState;
        if (oldState != newState) {
            notifyStateChanged();
        }
    }

    /**
     * 通知状态变化回调（在 FX 线程上调用）
     */
    private void notifyStateChanged() {
        Consumer<PlaybackState> cb = this.onStateChanged;
        if (cb != null) {
            cb.accept(new PlaybackState(this.state, this.positionSeconds, this.totalSeconds, this.volume));
        }
    }

    /**
     * 启动位置轮询定时器（FX 线程）
     *
     * <p>使用 {@link AnimationTimer} 每帧读取 MediaPlayer 当前位置，
     * 更新 {@link #positionSeconds} 并触发位置回调（节流到 ~30fps）。
     */
    private void startPositionTimer() {
        stopPositionTimer();

        AnimationTimer timer = new AnimationTimer() {
            @Override
            public void handle(long now) {
                MediaPlayer mp = player;
                if (mp == null) {
                    return;
                }
                // 节流：每 POSITION_UPDATE_INTERVAL_NANOS 纳秒更新一次
                if (now - lastPositionUpdateNanos < POSITION_UPDATE_INTERVAL_NANOS) {
                    return;
                }
                lastPositionUpdateNanos = now;

                Duration current = mp.getCurrentTime();
                if (current != null && !current.isUnknown()) {
                    double pos = current.toSeconds();
                    positionSeconds = pos;

                    Consumer<Double> cb = onPositionChanged;
                    if (cb != null) {
                        cb.accept(pos);
                    }
                }
            }
        };
        this.positionTimer = timer;
        timer.start();
    }

    /**
     * 停止位置轮询定时器（FX 线程）
     */
    private void stopPositionTimer() {
        AnimationTimer timer = this.positionTimer;
        if (timer != null) {
            timer.stop();
            this.positionTimer = null;
        }
    }

    /**
     * 清理上一次播放的资源（临时文件 + MediaPlayer）
     *
     * <p>在开始新播放前调用。
     */
    private void cleanupPrevious() {
        // 停止旧的位置定时器
        runOnFxThread(this::stopPositionTimer);

        // dispose 旧的 MediaPlayer
        MediaPlayer oldPlayer = this.player;
        if (oldPlayer != null) {
            runOnFxThread(() -> {
                oldPlayer.stop();
                oldPlayer.dispose();
            });
            this.player = null;
        }

        // 删除旧临时文件
        deleteTempFile(this.tempFile);
        this.tempFile = null;
    }

    /**
     * 安全删除临时文件（忽略失败）
     */
    private static void deleteTempFile(Path file) {
        if (file != null) {
            try {
                Files.deleteIfExists(file);
            } catch (IOException e) {
                // 忽略删除失败，临时文件会在 JVM 退出时由系统清理
            }
        }
    }

    /**
     * 在 FX 线程上执行 Runnable
     *
     * <p>若当前已是 FX 线程则直接执行，否则通过 {@link Platform#runLater} 调度。
     */
    private static void runOnFxThread(Runnable action) {
        if (Platform.isFxApplicationThread()) {
            action.run();
        } else {
            Platform.runLater(action);
        }
    }
}
