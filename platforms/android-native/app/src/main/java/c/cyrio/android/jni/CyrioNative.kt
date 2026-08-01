package c.cyrio.android.jni

/**
 * Rust cyrio-core 的 JNI 桥接层
 *
 * 所有方法由 Rust 动态库 (libcyrio_jni) 实现，Kotlin 仅声明 external 方法。
 * 核心逻辑（USB 协议、文件传输、文本处理等）全部在 Rust 中完成，
 * Kotlin 仅负责 UI 展示和用户交互。
 *
 * 架构：
 *   Kotlin UI → CyrioNative (JNI) → cyrio-jni (Rust cdylib) → cyrio-core
 *
 * 设备句柄：long 类型（Rust 端 Box<RioDevice> 的裸指针）。
 * 0 表示无效/失败。由 openDevice() 创建，closeDevice() 释放。
 *
 * 数据交换：复杂数据通过 JSON 字符串交换，Kotlin 用 org.json 解析。
 *
 * JNI 命名规则：@JvmStatic external fun openDevice()
 *   → Java_c_cyrio_android_jni_CyrioNative_openDevice
 * 与 Rust 侧 #[no_mangle] 函数名严格匹配。
 */
object CyrioNative {

    init {
        System.loadLibrary("cyrio_jni")
    }

    // ========================================================================
    // 设备管理
    // ========================================================================

    /** 打开 Rio S-Series 设备（自动扫描 VID=0x045a） */
    @JvmStatic external fun openDevice(): Long

    /** 以指定 VID/PID 强制打开设备 */
    @JvmStatic external fun openDeviceWithVidPid(vid: Int, pid: Int): Long

    /** 关闭设备并释放资源 */
    @JvmStatic external fun closeDevice(handle: Long)

    /** 检查设备是否已连接 */
    @JvmStatic external fun isConnected(handle: Long): Boolean

    // ========================================================================
    // 设备操作
    // ========================================================================

    /**
     * 列出内存单元中的所有歌曲
     *
     * @param memUnit 0=内置, 1=SD卡
     * @return JSON: [{"fileNo","size","time","bitRate","sampleRate","name","title","artist","album","memUnit"}]
     */
    @JvmStatic external fun listSongs(handle: Long, memUnit: Int): String

    /**
     * 获取存储信息
     * @return JSON: {"totalSize","usedSize","freeSize","systemSize","name","model","isPresent"}
     */
    @JvmStatic external fun getStorage(handle: Long, memUnit: Int): String

    /**
     * 上传 MP3 文件到设备
     * @return 文件号 (>0 成功, -1 失败)
     */
    @JvmStatic external fun uploadFile(
        handle: Long, memUnit: Int, filePath: String,
        applySlug: Boolean, applyStrip: Boolean
    ): Int

    /** 下载文件到本地路径 */
    @JvmStatic external fun downloadFile(handle: Long, memUnit: Int, fileNo: Int, outputPath: String): Boolean

    /** 删除设备上的文件 */
    @JvmStatic external fun deleteFile(handle: Long, memUnit: Int, fileNo: Int): Boolean

    // ========================================================================
    // 歌单操作
    // ========================================================================

    /** 列出所有歌单 */
    @JvmStatic external fun listPlaylists(handle: Long, memUnit: Int): String

    /** 创建新歌单，返回文件号 (>0 成功, -1 失败) */
    @JvmStatic external fun createPlaylist(handle: Long, name: String, memUnit: Int): Int

    /** 添加歌曲到歌单 */
    @JvmStatic external fun addToPlaylist(
        handle: Long, songFileNo: Int, songMemUnit: Int,
        playlistFileNo: Int, playlistMemUnit: Int
    ): Boolean

    /** 列出歌单内的歌曲 */
    @JvmStatic external fun listPlaylistSongs(handle: Long, playlistFileNo: Int, memUnit: Int): String

    /** 从歌单中移除指定位置的歌曲 */
    @JvmStatic external fun removeFromPlaylist(handle: Long, playlistFileNo: Int, memUnit: Int, index: Int): Boolean

    // ========================================================================
    // 重命名 / 编码修复
    // ========================================================================

    /** 重命名歌曲 */
    @JvmStatic external fun renameSong(handle: Long, fileNo: Int, memUnit: Int, newName: String): Boolean

    /** 修复歌曲编码 */
    @JvmStatic external fun repairEncoding(handle: Long, fileNo: Int, memUnit: Int): Boolean

    // ========================================================================
    // 文本处理
    // ========================================================================

    /** Slug 转换（中文→拼音，日文→罗马字） */
    @JvmStatic external fun toSlug(text: String, separator: String, capitalize: Boolean, keepPunctuation: Boolean): String

    /** 去除标题噪音词 */
    @JvmStatic external fun stripNoise(text: String): String

    /** 处理标题（先 strip 再 slug） */
    @JvmStatic external fun processTitle(title: String, applySlug: Boolean, applyStrip: Boolean): String

    // ========================================================================
    // USB 设备扫描
    // ========================================================================

    /** 列出系统中所有 USB 设备 */
    @JvmStatic external fun listUsbDevices(): String
}
