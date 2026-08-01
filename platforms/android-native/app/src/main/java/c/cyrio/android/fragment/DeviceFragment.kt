package c.cyrio.android.fragment

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.fragment.app.Fragment
import c.cyrio.android.R
import c.cyrio.android.model.StorageInfo
import c.cyrio.android.util.CyrioDeviceManager

/**
 * 设备页 — 自动扫描 Diamond 设备（VID=0x045a），8 秒间隔重试
 *
 * 三种状态：
 * 1. scanning — 扫描中（圆形加载动画）
 * 2. connected — 已连接（显示设备名 + 内置/SD 卡存储信息 + 断开按钮）
 * 3. disconnected — 未连接/失败（提示 + 重试按钮）
 *
 * USB 重试策略：初始 3 次每 500ms，之后每 2 秒
 */
class DeviceFragment : Fragment() {

    private val TAG = "DeviceFragment"

    private lateinit var layoutScanning: LinearLayout
    private lateinit var layoutConnected: LinearLayout
    private lateinit var layoutDisconnected: LinearLayout

    private lateinit var textDeviceName: TextView
    private lateinit var textStatus: TextView

    // 内置存储
    private lateinit var textStorageInternalName: TextView
    private lateinit var textStorageInternalTotal: TextView
    private lateinit var textStorageInternalUsed: TextView
    private lateinit var progressInternal: View

    // SD 卡
    private lateinit var textStorageSdName: TextView
    private lateinit var textStorageSdTotal: TextView
    private lateinit var textStorageSdUsed: TextView
    private lateinit var progressSd: View

    private lateinit var btnConnect: Button
    private lateinit var btnDisconnect: Button

    /** 主线程 Handler */
    private val handler = Handler(Looper.getMainLooper())

    /** 扫描重试次数 */
    private var scanRetryCount = 0

    /** 是否正在扫描 */
    private var scanning = false

    /** 扫描 Runnable */
    private val scanRunnable = Runnable { startScan() }

    /** 连接状态监听器（连接完成时自动更新 UI） */
    private val connectionListener: (Boolean) -> Unit = { connected ->
        if (connected) {
            showConnected()
            loadStorageInfo()
        } else if (!CyrioDeviceManager.isConnecting) {
            // 连接失败（不是正在连接中）
            scanRetryCount++
            Log.w(TAG, "Connection failed via listener, retry #$scanRetryCount")
            val delay = if (scanRetryCount <= 3) 500L else 2000L
            handler.postDelayed(scanRunnable, delay)
            showDisconnected()
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return inflater.inflate(R.layout.fragment_device, container, false)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        // 绑定视图
        layoutScanning = view.findViewById(R.id.layout_scanning)
        layoutConnected = view.findViewById(R.id.layout_connected)
        layoutDisconnected = view.findViewById(R.id.layout_disconnected)

        textDeviceName = view.findViewById(R.id.text_device_name)
        textStatus = view.findViewById(R.id.text_status)

        textStorageInternalName = view.findViewById(R.id.text_storage_internal_name)
        textStorageInternalTotal = view.findViewById(R.id.text_storage_internal_total)
        textStorageInternalUsed = view.findViewById(R.id.text_storage_internal_used)
        progressInternal = view.findViewById(R.id.progress_internal)

        textStorageSdName = view.findViewById(R.id.text_storage_sd_name)
        textStorageSdTotal = view.findViewById(R.id.text_storage_sd_total)
        textStorageSdUsed = view.findViewById(R.id.text_storage_sd_used)
        progressSd = view.findViewById(R.id.progress_sd)

        btnConnect = view.findViewById(R.id.btn_connect)
        btnDisconnect = view.findViewById(R.id.btn_disconnect)

        // 连接按钮
        btnConnect.setOnClickListener {
            startScan()
        }

        // 断开按钮
        btnDisconnect.setOnClickListener {
            CyrioDeviceManager.closeDevice {
                showDisconnected()
            }
        }

        // 注册连接状态监听器
        CyrioDeviceManager.addConnectionListener(connectionListener)

        // 根据当前连接状态显示对应 UI
        when {
            CyrioDeviceManager.isConnected -> {
                showConnected()
                loadStorageInfo()
            }
            CyrioDeviceManager.isConnecting -> {
                // MainActivity 已发起连接，显示扫描中
                showScanning()
            }
            else -> {
                // 未连接，开始自动扫描
                startScan()
            }
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        // 停止扫描
        scanning = false
        handler.removeCallbacks(scanRunnable)
        // 移除连接状态监听器
        CyrioDeviceManager.removeConnectionListener(connectionListener)
    }

    /**
     * 开始扫描设备
     * USB 重试策略：初始 3 次每 500ms，之后每 2 秒
     */
    private fun startScan() {
        if (scanning) return
        scanning = true
        showScanning()

        CyrioDeviceManager.openDevice { success ->
            scanning = false
            if (success) {
                Log.i(TAG, "Device connected")
                showConnected()
                loadStorageInfo()
            } else {
                scanRetryCount++
                Log.w(TAG, "Scan failed, retry #$scanRetryCount")

                // 重试策略
                val delay = if (scanRetryCount <= 3) 500L else 2000L
                handler.postDelayed(scanRunnable, delay)
            }
        }
    }

    /** 显示扫描中状态 */
    private fun showScanning() {
        layoutScanning.visibility = View.VISIBLE
        layoutConnected.visibility = View.GONE
        layoutDisconnected.visibility = View.GONE
    }

    /** 显示已连接状态 */
    private fun showConnected() {
        layoutScanning.visibility = View.GONE
        layoutConnected.visibility = View.VISIBLE
        layoutDisconnected.visibility = View.GONE

        textDeviceName.text = CyrioDeviceManager.deviceName
        textStatus.text = getString(R.string.device_connected)
        scanRetryCount = 0
        scanning = false
    }

    /** 显示未连接状态 */
    private fun showDisconnected() {
        layoutScanning.visibility = View.GONE
        layoutConnected.visibility = View.GONE
        layoutDisconnected.visibility = View.VISIBLE
    }

    /** 加载存储信息（内置 + SD 卡） */
    private fun loadStorageInfo() {
        // 内置存储
        CyrioDeviceManager.getStorage(0) { info ->
            if (info != null && info.isPresent) {
                updateStorageCard(
                    textStorageInternalName, textStorageInternalTotal,
                    textStorageInternalUsed, progressInternal,
                    info, getString(R.string.storage_internal)
                )
            } else {
                textStorageInternalTotal.text = getString(R.string.storage_not_present)
                textStorageInternalUsed.text = ""
                progressInternal.layoutParams.width = 0
            }
        }

        // SD 卡
        CyrioDeviceManager.getStorage(1) { info ->
            if (info != null && info.isPresent) {
                updateStorageCard(
                    textStorageSdName, textStorageSdTotal,
                    textStorageSdUsed, progressSd,
                    info, getString(R.string.storage_sd)
                )
            } else {
                textStorageSdTotal.text = getString(R.string.storage_not_present)
                textStorageSdUsed.text = ""
                progressSd.layoutParams.width = 0
            }
        }
    }

    /** 更新存储卡片信息 */
    private fun updateStorageCard(
        nameText: TextView, totalText: TextView,
        usedText: TextView, progressBar: View,
        info: StorageInfo, defaultName: String
    ) {
        nameText.text = if (info.name.isNotBlank()) info.name else defaultName
        totalText.text = info.totalSizeText
        usedText.text = getString(R.string.storage_used_format, info.usedSizeText, info.freeSizeText)

        // 更新进度条宽度（按百分比）
        progressBar.post {
            val parent = progressBar.parent as View
            val percent = info.usedPercent.coerceIn(0, 100)
            val width = parent.width * percent / 100
            val params = progressBar.layoutParams
            params.width = width
            progressBar.layoutParams = params
        }
    }

    override fun onResume() {
        super.onResume()
        // 根据当前状态更新 UI（连接状态变化由 listener 处理）
        when {
            CyrioDeviceManager.isConnected -> {
                showConnected()
            }
            CyrioDeviceManager.isConnecting -> {
                showScanning()
            }
            !scanning -> {
                // 未连接且未在扫描，重新开始扫描
                startScan()
            }
        }
    }
}
