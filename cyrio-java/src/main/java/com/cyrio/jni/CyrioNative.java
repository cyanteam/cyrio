package com.cyrio.jni;

/**
 * Rust cyrio-core 的 JNI 桥接层
 *
 * <p>所有方法均由 Rust 动态库 (libcyrio_jni) 实现，Java 端仅声明 native 方法。
 * 核心逻辑（USB 协议、文件传输、文本处理等）全部在 Rust 中完成，
 * Java 仅负责 UI 展示和用户交互。
 *
 * <h3>架构</h3>
 * <pre>
 * JavaFX UI  →  CyrioNative (JNI)  →  cyrio-jni (Rust cdylib)  →  cyrio-core
 * </pre>
 *
 * <h3>设备句柄</h3>
 * <p>设备以 {@code long} 句柄传递（Rust 端 {@code Box<RioDevice>} 的裸指针）。
 * {@code 0} 表示无效/失败。句柄由 {@link #openDevice()} 创建，
 * 由 {@link #closeDevice(long)} 释放。
 *
 * <h3>数据交换</h3>
 * <p>复杂数据（歌曲列表、存储信息等）通过 JSON 字符串交换，
 * Java 端用 Jackson 解析。简单类型（fileNo、boolean 等）直接返回。
 */
public class CyrioNative {

    static {
        NativeLibraryLoader.load("cyrio_jni");
    }

    // ========================================================================
    // 设备管理
    // ========================================================================

    /**
     * 打开 Rio S-Series 设备（自动扫描 VID=0x045a）
     *
     * @return 设备句柄 (＞0 成功, 0 失败)
     */
    public static native long openDevice();

    /**
     * 以指定 VID/PID 强制打开设备
     *
     * @param vid USB Vendor ID
     * @param pid USB Product ID
     * @return 设备句柄 (＞0 成功, 0 失败)
     */
    public static native long openDeviceWithVidPid(int vid, int pid);

    /**
     * 关闭设备并释放资源
     *
     * @param handle 设备句柄
     */
    public static native void closeDevice(long handle);

    /**
     * 检查设备是否已连接
     *
     * @param handle 设备句柄
     * @return true 表示句柄有效
     */
    public static native boolean isConnected(long handle);

    // ========================================================================
    // 设备操作
    // ========================================================================

    /**
     * 列出内存单元中的所有歌曲
     *
     * @param handle  设备句柄
     * @param memUnit 内存单元 (0=内置, 1=SD卡)
     * @return JSON 数组: [{"fileNo","size","time","bitRate","sampleRate","name","title","artist","album","memUnit"}]
     */
    public static native String listSongs(long handle, int memUnit);

    /**
     * 获取存储信息
     *
     * @param handle  设备句柄
     * @param memUnit 内存单元
     * @return JSON: {"totalSize","usedSize","freeSize","systemSize","name","model","isPresent"}
     */
    public static native String getStorage(long handle, int memUnit);

    /**
     * 上传 MP3 文件到设备
     *
     * @param handle     设备句柄
     * @param memUnit    目标内存单元
     * @param filePath   本地文件路径
     * @param applySlug  是否应用拼音转换
     * @param applyStrip 是否应用去词
     * @return 文件号 (＞0 成功, -1 失败)
     */
    public static native int uploadFile(long handle, int memUnit, String filePath,
                                        boolean applySlug, boolean applyStrip);

    /**
     * 下载文件到本地路径
     *
     * @param handle     设备句柄
     * @param memUnit    内存单元
     * @param fileNo     文件号
     * @param outputPath 本地输出路径
     * @return true 成功
     */
    public static native boolean downloadFile(long handle, int memUnit, int fileNo, String outputPath);

    /**
     * 删除设备上的文件
     *
     * @param handle  设备句柄
     * @param memUnit 内存单元
     * @param fileNo  文件号
     * @return true 成功
     */
    public static native boolean deleteFile(long handle, int memUnit, int fileNo);

    // ========================================================================
    // 歌单操作
    // ========================================================================

    /**
     * 列出所有歌单
     *
     * @param handle  设备句柄
     * @param memUnit 内存单元
     * @return JSON 数组: [{"fileNo","size","name","title"}]
     */
    public static native String listPlaylists(long handle, int memUnit);

    /**
     * 创建新歌单
     *
     * @param handle  设备句柄
     * @param name    歌单名称
     * @param memUnit 内存单元
     * @return 文件号 (＞0 成功, -1 失败)
     */
    public static native int createPlaylist(long handle, String name, int memUnit);

    /**
     * 添加歌曲到歌单
     *
     * @param handle           设备句柄
     * @param songFileNo       歌曲文件号
     * @param songMemUnit      歌曲所在内存单元
     * @param playlistFileNo   歌单文件号
     * @param playlistMemUnit  歌单所在内存单元
     * @return true 成功
     */
    public static native boolean addToPlaylist(long handle, int songFileNo, int songMemUnit,
                                                int playlistFileNo, int playlistMemUnit);

    /**
     * 列出歌单内的歌曲
     *
     * @param handle          设备句柄
     * @param playlistFileNo  歌单文件号
     * @param memUnit         歌单所在内存单元
     * @return JSON 数组: [{"fileNo","size","time","bitRate","sampleRate","name","title","artist","album","memUnit","index"}]
     */
    public static native String listPlaylistSongs(long handle, int playlistFileNo, int memUnit);

    /**
     * 从歌单中移除指定位置的歌曲
     *
     * @param handle          设备句柄
     * @param playlistFileNo  歌单文件号
     * @param memUnit         歌单所在内存单元
     * @param index           条目索引 (0-based，来自 listPlaylistSongs 返回的 index)
     * @return true 成功
     */
    public static native boolean removeFromPlaylist(long handle, int playlistFileNo,
                                                     int memUnit, int index);

    // ========================================================================
    // 重命名 / 编码修复
    // ========================================================================

    /**
     * 重命名歌曲（修改 name 和 title 字段）
     *
     * @param handle  设备句柄
     * @param fileNo  文件号
     * @param memUnit 内存单元
     * @param newName 新名称
     * @return true 成功
     */
    public static native boolean renameSong(long handle, int fileNo, int memUnit, String newName);

    /**
     * 修复歌曲编码（双重编码 → 正确 UTF-8）
     *
     * @param handle  设备句柄
     * @param fileNo  文件号
     * @param memUnit 内存单元
     * @return true 成功
     */
    public static native boolean repairEncoding(long handle, int fileNo, int memUnit);

    // ========================================================================
    // 文本处理
    // ========================================================================

    /**
     * Slug 转换（中文→拼音，日文→罗马字）
     *
     * @param text           输入文本
     * @param separator      分隔符 (如 "-")
     * @param capitalize     是否首字母大写
     * @param keepPunctuation 是否保留标点
     * @return 转换后的字符串
     */
    public static native String toSlug(String text, String separator,
                                       boolean capitalize, boolean keepPunctuation);

    /**
     * 去除标题噪音词
     *
     * @param text 输入文本
     * @return 去噪后的文本
     */
    public static native String stripNoise(String text);

    /**
     * 处理标题（先 strip 再 slug）
     *
     * @param title      原始标题
     * @param applySlug  是否应用拼音转换
     * @param applyStrip 是否应用去词
     * @return 处理后的标题
     */
    public static native String processTitle(String title, boolean applySlug, boolean applyStrip);

    // ========================================================================
    // USB 设备扫描
    // ========================================================================

    /**
     * 列出系统中所有 USB 设备
     *
     * @return JSON 数组: [{"vid","pid","name","manufacturer","serial"}]
     */
    public static native String listUsbDevices();

    // ========================================================================
    // 私有构造（工具类不可实例化）
    // ========================================================================

    private CyrioNative() {
    }
}
