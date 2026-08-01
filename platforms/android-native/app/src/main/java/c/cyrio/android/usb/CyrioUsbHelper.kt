package c.cyrio.android.usb

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.*
import android.os.Handler
import android.os.Looper
import android.util.Log
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Android USB Host API 管理层
 *
 * 封装 UsbManager 的所有操作：设备枚举、权限请求、设备打开/关闭、
 * 控制传输和批量传输。Rust 侧通过 JNI 调用此单例的静态方法。
 *
 * 线程模型：
 * - openDevice/listDevices 等 JNI 调用在 smol::unblock 线程上执行
 * - USB 权限请求通过 Handler post 到主线程（Android 要求）
 * - CountDownLatch 在调用线程同步等待权限结果
 */
object CyrioUsbHelper {

    private const val TAG = "CyrioUsbHelper"
    private const val ACTION_USB_PERMISSION = "c.cyrio.android.USB_PERMISSION"

    /** 传输超时（毫秒） */
    private const val TRANSFER_TIMEOUT = 15000

    /** 权限请求超时（秒） */
    private const val PERMISSION_TIMEOUT = 10L

    private lateinit var appContext: Context
    private lateinit var usbManager: UsbManager

    private var connection: UsbDeviceConnection? = null
    private var bulkInEndpoint: UsbEndpoint? = null
    private var bulkOutEndpoint: UsbEndpoint? = null

    /** 上次打开的设备 VID/PID（用于 resetDevice） */
    private var lastVid: Int = 0x045a
    private var lastPid: Int = 0

    /**
     * 初始化 USB Helper
     *
     * 在 MainActivity.onCreate() 中调用，保存 Context 并通知 Rust 侧保存 JavaVM。
     */
    fun init(activity: Activity) {
        appContext = activity.applicationContext
        usbManager = appContext.getSystemService(Context.USB_SERVICE) as UsbManager
        Log.i(TAG, "CyrioUsbHelper initialized")

        // 先确保 libcyrio_jni.so 已加载
        // （CyrioNative.init 也会加载，但 CyrioNative 对象可能尚未被引用触发初始化）
        try {
            System.loadLibrary("cyrio_jni")
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "Failed to load libcyrio_jni.so", e)
        }

        try {
            nativeInit()
            Log.i(TAG, "nativeInit() called successfully")
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "nativeInit() failed - native library not loaded", e)
        }
    }

    // ========================================================================
    // JNI 声明 — Rust 侧实现
    // ========================================================================

    /** 通知 Rust 侧保存 JavaVM 引用 */
    @JvmStatic
    external fun nativeInit()

    // ========================================================================
    // 设备枚举
    // ========================================================================

    /**
     * 列出所有已连接的 USB 设备
     * 返回 JSON: [{"vid":1114,"pid":64769,"name":"...","manufacturer":"...","serial":"..."}]
     */
    @JvmStatic
    fun listDevices(): String {
        val deviceList = usbManager.deviceList
        val sb = StringBuilder("[")

        var first = true
        for ((_, device) in deviceList) {
            if (!first) sb.append(",")
            first = false

            val vid = device.vendorId
            val pid = device.productId
            val name = escapeJson(device.productName ?: "")
            val manufacturer = escapeJson(device.manufacturerName ?: "")
            val serial = escapeJson(device.serialNumber ?: "")

            sb.append("""{"vid":$vid,"pid":$pid,"name":"$name","manufacturer":"$manufacturer","serial":"$serial"}""")
        }

        sb.append("]")
        Log.d(TAG, "listDevices() → ${sb.length} chars, ${deviceList.size} devices")
        return sb.toString()
    }

    // ========================================================================
    // 设备打开/关闭
    // ========================================================================

    /**
     * 打开 USB 设备
     *
     * @param vid 厂商 ID（Diamond = 0x045a）
     * @param pid 产品 ID（0 = 接受任何 PID）
     * @return true=成功
     */
    @JvmStatic
    fun openDevice(vid: Int, pid: Int): Boolean {
        Log.i(TAG, "openDevice(vid=0x${vid.toString(16)}, pid=0x${pid.toString(16)})")
        closeDevice()

        lastVid = vid
        lastPid = pid

        val device = usbManager.deviceList.values.find {
            it.vendorId == vid && (pid == 0 || it.productId == pid)
        }
        if (device == null) {
            Log.e(TAG, "Device not found")
            return false
        }

        if (!requestPermission(device)) {
            Log.e(TAG, "USB permission denied")
            return false
        }

        connection = usbManager.openDevice(device)
        if (connection == null) {
            Log.e(TAG, "openDevice() returned null")
            return false
        }

        val usbInterface = device.getInterface(0)
        if (!connection!!.claimInterface(usbInterface, true)) {
            Log.e(TAG, "claimInterface(0) failed")
            connection!!.close()
            connection = null
            return false
        }

        for (i in 0 until usbInterface.endpointCount) {
            val ep = usbInterface.getEndpoint(i)
            if (ep.type == UsbConstants.USB_ENDPOINT_XFER_BULK) {
                if (ep.direction == UsbConstants.USB_DIR_IN) bulkInEndpoint = ep
                else if (ep.direction == UsbConstants.USB_DIR_OUT) bulkOutEndpoint = ep
            }
        }

        if (bulkInEndpoint == null || bulkOutEndpoint == null) {
            Log.e(TAG, "Failed to find bulk endpoints")
            connection!!.close()
            connection = null
            return false
        }

        Log.i(TAG, "Device opened successfully")
        return true
    }

    /** 关闭设备连接 */
    @JvmStatic
    fun closeDevice() {
        try { connection?.close() } catch (e: Exception) { }
        connection = null
        bulkInEndpoint = null
        bulkOutEndpoint = null
    }

    /** 重置设备 */
    @JvmStatic
    fun resetDevice(): Boolean {
        closeDevice()
        return openDevice(lastVid, lastPid)
    }

    // ========================================================================
    // USB 传输
    // ========================================================================

    /** 控制传输 IN（设备 → 主机） */
    @JvmStatic
    fun controlTransferIn(request: Int, value: Int, index: Int, length: Int): ByteArray {
        val conn = connection ?: return ByteArray(0)
        val buffer = ByteArray(length)
        val n = conn.controlTransfer(
            UsbConstants.USB_DIR_IN or UsbConstants.USB_TYPE_VENDOR,
            request, value, index, buffer, length, TRANSFER_TIMEOUT
        )
        if (n < 0) { Log.e(TAG, "controlTransferIn(req=0x${request.toString(16)}) failed: n=$n"); return ByteArray(0) }
        return if (n == length) buffer else buffer.copyOf(n)
    }

    /** 控制传输 OUT（主机 → 设备） */
    @JvmStatic
    fun controlTransferOut(request: Int, value: Int, index: Int, data: ByteArray): Int {
        val conn = connection ?: return -1
        val n = conn.controlTransfer(
            UsbConstants.USB_DIR_OUT or UsbConstants.USB_TYPE_VENDOR,
            request, value, index, data, data.size, TRANSFER_TIMEOUT
        )
        if (n < 0) Log.e(TAG, "controlTransferOut(req=0x${request.toString(16)}) failed: n=$n")
        return n
    }

    /** 批量传输 IN */
    @JvmStatic
    fun bulkTransferIn(length: Int): ByteArray {
        val conn = connection ?: return ByteArray(0)
        val ep = bulkInEndpoint ?: return ByteArray(0)
        val buffer = ByteArray(length)
        val n = conn.bulkTransfer(ep, buffer, length, TRANSFER_TIMEOUT)
        if (n < 0) { Log.e(TAG, "bulkTransferIn(len=$length) failed: n=$n"); return ByteArray(0) }
        return if (n == length) buffer else buffer.copyOf(n)
    }

    /** 批量传输 OUT */
    @JvmStatic
    fun bulkTransferOut(data: ByteArray): Int {
        val conn = connection ?: return -1
        val ep = bulkOutEndpoint ?: return -1
        val n = conn.bulkTransfer(ep, data, data.size, TRANSFER_TIMEOUT)
        if (n < 0) Log.e(TAG, "bulkTransferOut(len=${data.size}) failed: n=$n")
        return n
    }

    // ========================================================================
    // 权限请求
    // ========================================================================

    private fun requestPermission(device: UsbDevice): Boolean {
        if (usbManager.hasPermission(device)) return true

        val latch = CountDownLatch(1)
        var granted = false

        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                if (ACTION_USB_PERMISSION == intent.action) {
                    granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                    latch.countDown()
                }
            }
        }

        val mainHandler = Handler(Looper.getMainLooper())
        mainHandler.post {
            try {
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
                    appContext.registerReceiver(receiver, IntentFilter(ACTION_USB_PERMISSION), Context.RECEIVER_NOT_EXPORTED)
                } else {
                    appContext.registerReceiver(receiver, IntentFilter(ACTION_USB_PERMISSION))
                }
                val pendingIntent = PendingIntent.getBroadcast(
                    appContext, 0, Intent(ACTION_USB_PERMISSION), PendingIntent.FLAG_IMMUTABLE
                )
                usbManager.requestPermission(device, pendingIntent)
            } catch (e: Exception) {
                latch.countDown()
            }
        }

        try { latch.await(PERMISSION_TIMEOUT, TimeUnit.SECONDS) } catch (e: InterruptedException) { }

        mainHandler.post {
            try { appContext.unregisterReceiver(receiver) } catch (e: Exception) { }
        }

        return granted
    }

    // ========================================================================
    // 工具函数
    // ========================================================================

    private fun escapeJson(s: String): String {
        val sb = StringBuilder(s.length + 8)
        for (ch in s) {
            when (ch) {
                '"' -> sb.append("\\\"")
                '\\' -> sb.append("\\\\")
                '\n' -> sb.append("\\n")
                '\r' -> sb.append("\\r")
                '\t' -> sb.append("\\t")
                else -> sb.append(ch)
            }
        }
        return sb.toString()
    }
}
