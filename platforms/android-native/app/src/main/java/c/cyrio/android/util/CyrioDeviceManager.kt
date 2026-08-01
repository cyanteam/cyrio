package c.cyrio.android.util

import android.os.Handler
import android.os.Looper
import android.util.Log
import c.cyrio.android.jni.CyrioNative
import c.cyrio.android.model.*

/**
 * 设备管理器 — 封装 CyrioNative JNI 调用
 *
 * 所有 USB 操作都是阻塞调用，必须在后台线程执行。
 * 使用 Handler 切换回主线程通知 UI。
 *
 * 使用方式：
 *   CyrioDeviceManager.openDevice { success -> ... }
 *   CyrioDeviceManager.listSongs(0) { songs -> ... }
 */
object CyrioDeviceManager {

    private const val TAG = "CyrioDeviceManager"

    /** 主线程 Handler（回调切换到主线程） */
    private val mainHandler = Handler(Looper.getMainLooper())

    /** 当前设备句柄（0 = 未连接） */
    @Volatile
    var deviceHandle: Long = 0
        private set

    /** 设备是否已连接 */
    val isConnected: Boolean get() = deviceHandle != 0L

    /** 是否正在连接中（启动时自动连接，连接完成前 UI 显示转圈） */
    @Volatile
    var isConnecting: Boolean = false
        private set

    /** 设备名称（连接后通过 getStorage 获取） */
    @Volatile
    var deviceName: String = "未连接"
        private set

    /** 后台执行器（单线程，USB 操作必须串行） */
    private val executor = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
        Thread(r, "cyrio-device").apply { isDaemon = true }
    }

    /** 连接状态监听器列表（连接成功/失败时通知 UI） */
    private val connectionListeners = mutableListOf<(Boolean) -> Unit>()

    /** 注册连接状态监听器（立即回调当前状态） */
    fun addConnectionListener(listener: (Boolean) -> Unit) {
        connectionListeners.add(listener)
        // 立即通知当前状态
        listener(isConnected)
    }

    /** 移除连接状态监听器 */
    fun removeConnectionListener(listener: (Boolean) -> Unit) {
        connectionListeners.remove(listener)
    }

    /** 通知所有监听器连接状态变化（主线程回调） */
    private fun notifyConnectionChanged(connected: Boolean) {
        mainHandler.post {
            connectionListeners.forEach { it(connected) }
        }
    }

    // ========================================================================
    // 设备管理
    // ========================================================================

    /** 打开设备（自动扫描 Diamond VID=0x045a） */
    fun openDevice(callback: (Boolean) -> Unit) {
        if (isConnecting) {
            // 已在连接中，不重复发起
            mainHandler.post { callback(isConnected) }
            return
        }
        isConnecting = true
        executor.execute {
            val handle = CyrioNative.openDevice()
            deviceHandle = handle
            isConnecting = false
            Log.i(TAG, "openDevice → handle=$handle")
            if (handle != 0L) {
                // 连接成功后立即读取设备名称
                try {
                    val storageJson = CyrioNative.getStorage(handle, 0)
                    val storage = StorageInfo.parse(storageJson)
                    if (!storage?.name.isNullOrBlank()) {
                        deviceName = storage!!.name
                    } else {
                        deviceName = "RioS50"
                    }
                } catch (e: Exception) {
                    deviceName = "RioS50"
                }
            }
            // 通知所有监听器
            notifyConnectionChanged(handle != 0L)
            mainHandler.post { callback(handle != 0L) }
        }
    }

    /** 强制以指定 VID/PID 打开设备 */
    fun openDeviceWithVidPid(vid: Int, pid: Int, callback: (Boolean) -> Unit) {
        executor.execute {
            val handle = CyrioNative.openDeviceWithVidPid(vid, pid)
            deviceHandle = handle
            Log.i(TAG, "openDeviceWithVidPid($vid, $pid) → handle=$handle")
            mainHandler.post { callback(handle != 0L) }
        }
    }

    /** 关闭设备 */
    fun closeDevice(callback: (() -> Unit)? = null) {
        executor.execute {
            if (deviceHandle != 0L) {
                CyrioNative.closeDevice(deviceHandle)
                deviceHandle = 0
                deviceName = "未连接"
                Log.i(TAG, "closeDevice")
            }
            // 通知所有监听器断开连接
            notifyConnectionChanged(false)
            mainHandler.post { callback?.invoke() }
        }
    }

    // ========================================================================
    // 歌曲操作
    // ========================================================================

    /** 列出歌曲（内置=0, SD卡=1） */
    fun listSongs(memUnit: Int, callback: (List<Song>) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(emptyList()) }; return }
        executor.execute {
            val json = CyrioNative.listSongs(deviceHandle, memUnit)
            val songs = Song.parseList(json)
            Log.i(TAG, "listSongs($memUnit) → ${songs.size} songs")
            mainHandler.post { callback(songs) }
        }
    }

    /** 列出所有歌曲（先内置后SD卡，串行避免 USB 锁竞争） */
    fun listAllSongs(callback: (List<Song>) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(emptyList()) }; return }
        executor.execute {
            val all = ArrayList<Song>()
            // 先读取内置存储
            val internalJson = CyrioNative.listSongs(deviceHandle, 0)
            all.addAll(Song.parseList(internalJson))
            // 再读取 SD 卡（可能未插入，失败返回空）
            val sdJson = CyrioNative.listSongs(deviceHandle, 1)
            all.addAll(Song.parseList(sdJson))
            Log.i(TAG, "listAllSongs → ${all.size} songs")
            mainHandler.post { callback(all) }
        }
    }

    /** 上传文件 */
    fun uploadFile(
        memUnit: Int, filePath: String,
        applySlug: Boolean = false, applyStrip: Boolean = false,
        callback: (Int) -> Unit
    ) {
        if (deviceHandle == 0L) { mainHandler.post { callback(-1) }; return }
        executor.execute {
            val fileNo = CyrioNative.uploadFile(deviceHandle, memUnit, filePath, applySlug, applyStrip)
            Log.i(TAG, "uploadFile → fileNo=$fileNo")
            mainHandler.post { callback(fileNo) }
        }
    }

    /** 下载文件 */
    fun downloadFile(memUnit: Int, fileNo: Int, outputPath: String, callback: (Boolean) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(false) }; return }
        executor.execute {
            val ok = CyrioNative.downloadFile(deviceHandle, memUnit, fileNo, outputPath)
            mainHandler.post { callback(ok) }
        }
    }

    /** 删除文件 */
    fun deleteFile(memUnit: Int, fileNo: Int, callback: (Boolean) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(false) }; return }
        executor.execute {
            val ok = CyrioNative.deleteFile(deviceHandle, memUnit, fileNo)
            Log.i(TAG, "deleteFile($memUnit, $fileNo) → $ok")
            mainHandler.post { callback(ok) }
        }
    }

    // ========================================================================
    // 歌单操作
    // ========================================================================

    fun listPlaylists(memUnit: Int, callback: (List<Playlist>) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(emptyList()) }; return }
        executor.execute {
            val json = CyrioNative.listPlaylists(deviceHandle, memUnit)
            val playlists = Playlist.parseList(json)
            mainHandler.post { callback(playlists) }
        }
    }

    fun createPlaylist(name: String, memUnit: Int, callback: (Int) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(-1) }; return }
        executor.execute {
            val fileNo = CyrioNative.createPlaylist(deviceHandle, name, memUnit)
            mainHandler.post { callback(fileNo) }
        }
    }

    fun addToPlaylist(songFileNo: Int, songMemUnit: Int, playlistFileNo: Int, playlistMemUnit: Int, callback: (Boolean) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(false) }; return }
        executor.execute {
            val ok = CyrioNative.addToPlaylist(deviceHandle, songFileNo, songMemUnit, playlistFileNo, playlistMemUnit)
            mainHandler.post { callback(ok) }
        }
    }

    fun listPlaylistSongs(playlistFileNo: Int, memUnit: Int, callback: (List<Song>) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(emptyList()) }; return }
        executor.execute {
            val json = CyrioNative.listPlaylistSongs(deviceHandle, playlistFileNo, memUnit)
            val songs = Song.parseList(json)
            mainHandler.post { callback(songs) }
        }
    }

    fun removeFromPlaylist(playlistFileNo: Int, memUnit: Int, index: Int, callback: (Boolean) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(false) }; return }
        executor.execute {
            val ok = CyrioNative.removeFromPlaylist(deviceHandle, playlistFileNo, memUnit, index)
            mainHandler.post { callback(ok) }
        }
    }

    // ========================================================================
    // 重命名 / 编码修复
    // ========================================================================

    fun renameSong(fileNo: Int, memUnit: Int, newName: String, callback: (Boolean) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(false) }; return }
        executor.execute {
            val ok = CyrioNative.renameSong(deviceHandle, fileNo, memUnit, newName)
            mainHandler.post { callback(ok) }
        }
    }

    fun repairEncoding(fileNo: Int, memUnit: Int, callback: (Boolean) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(false) }; return }
        executor.execute {
            val ok = CyrioNative.repairEncoding(deviceHandle, fileNo, memUnit)
            mainHandler.post { callback(ok) }
        }
    }

    // ========================================================================
    // 存储信息
    // ========================================================================

    fun getStorage(memUnit: Int, callback: (StorageInfo?) -> Unit) {
        if (deviceHandle == 0L) { mainHandler.post { callback(null) }; return }
        executor.execute {
            val json = CyrioNative.getStorage(deviceHandle, memUnit)
            val info = StorageInfo.parse(json)
            mainHandler.post { callback(info) }
        }
    }

    // ========================================================================
    // 文本处理（纯 CPU 操作，不需要设备连接）
    // ========================================================================

    fun processTitle(title: String, applySlug: Boolean, applyStrip: Boolean, callback: (String) -> Unit) {
        executor.execute {
            val result = CyrioNative.processTitle(title, applySlug, applyStrip)
            mainHandler.post { callback(result) }
        }
    }

    // ========================================================================
    // USB 设备列表
    // ========================================================================

    fun listUsbDevices(callback: (List<UsbDeviceInfo>) -> Unit) {
        executor.execute {
            val json = CyrioNative.listUsbDevices()
            val devices = UsbDeviceInfo.parseList(json)
            mainHandler.post { callback(devices) }
        }
    }
}
