package c.cyrio.android.fragment

import android.net.Uri
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.RadioGroup
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.fragment.app.Fragment
import c.cyrio.android.MainActivity
import c.cyrio.android.R
import c.cyrio.android.util.CyrioDeviceManager

/**
 * 上传传输页 — 空闲状态显示上传选项，传输中显示进度
 *
 * 空闲状态：
 * - 选择文件按钮（支持多选）
 * - 目标存储选择（内置/SD 卡）
 * - 拼音转换/去词选项（从设置读取默认值）
 *
 * 传输中状态：
 * - 当前文件名
 * - 总进度条
 * - 百分比 + 队列进度（1/N）
 *
 * 传输进行时禁用其他 Tab（通过 MainActivity.setTransferring）
 */
class UploadFragment : Fragment() {

    private lateinit var layoutIdle: LinearLayout
    private lateinit var layoutTransferring: LinearLayout

    private lateinit var btnSelectFiles: Button
    private lateinit var rgStorage: RadioGroup
    private lateinit var rbInternal: android.widget.RadioButton
    private lateinit var cbSlug: CheckBox
    private lateinit var cbStrip: CheckBox

    // 传输中视图
    private lateinit var textCurrentFile: TextView
    private lateinit var progressTransfer: ProgressBar
    private lateinit var textProgress: TextView
    private lateinit var textQueue: TextView

    /** 待上传文件 URI 列表 */
    private var pendingUris: List<Uri> = emptyList()

    /** 文件选择器 — 使用 GetMultipleContents（ACTION_GET_CONTENT），兼容性更好，不过滤文件类型 */
    private val filePickerLauncher = registerForActivityResult(
        ActivityResultContracts.GetMultipleContents()
    ) { uris ->
        if (uris.isNotEmpty()) {
            pendingUris = uris
            startUpload()
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return inflater.inflate(R.layout.fragment_upload, container, false)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        // 绑定视图
        layoutIdle = view.findViewById(R.id.layout_idle)
        layoutTransferring = view.findViewById(R.id.layout_transferring)

        btnSelectFiles = view.findViewById(R.id.btn_select_files)
        rgStorage = view.findViewById(R.id.rg_storage)
        rbInternal = view.findViewById(R.id.rb_internal)
        cbSlug = view.findViewById(R.id.cb_slug)
        cbStrip = view.findViewById(R.id.cb_strip)

        textCurrentFile = view.findViewById(R.id.text_current_file)
        progressTransfer = view.findViewById(R.id.progress_transfer)
        textProgress = view.findViewById(R.id.text_progress)
        textQueue = view.findViewById(R.id.text_queue)

        // 从设置读取默认值
        context?.let { ctx ->
            cbSlug.isChecked = SettingsFragment.getDefaultSlug(ctx)
            cbStrip.isChecked = SettingsFragment.getDefaultStrip(ctx)
        }

        // 选择文件按钮
        btnSelectFiles.setOnClickListener {
            if (!CyrioDeviceManager.isConnected) {
                Toast.makeText(requireContext(), "未连接设备", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            // 打开文件选择器（GetMultipleContents 不过滤文件类型，MP3/WAV 均可见）
            filePickerLauncher.launch("*/*")
        }
    }

    /** 开始上传 */
    private fun startUpload() {
        if (pendingUris.isEmpty()) return
        if (!CyrioDeviceManager.isConnected) {
            Toast.makeText(requireContext(), "未连接设备", Toast.LENGTH_SHORT).show()
            return
        }

        // 切换到传输中状态
        showTransferring()

        // 通知 Activity 禁用其他 Tab
        (activity as? MainActivity)?.setTransferring(true)

        // 获取上传参数
        val memUnit = if (rbInternal.isChecked) 0 else 1
        val applySlug = cbSlug.isChecked
        val applyStrip = cbStrip.isChecked

        // 逐个上传（串行避免 USB 锁竞争）
        var index = 0
        var successCount = 0

        fun uploadNext() {
            if (index >= pendingUris.size) {
                // 全部上传完成
                val msg = "上传完成：$successCount/${pendingUris.size} 个文件"
                Toast.makeText(requireContext(), msg, Toast.LENGTH_LONG).show()

                // 恢复空闲状态
                showIdle()
                (activity as? MainActivity)?.setTransferring(false)
                pendingUris = emptyList()
                return
            }

            val uri = pendingUris[index]
            val fileName = getFileName(uri)
            textCurrentFile.text = fileName
            textQueue.text = "${index + 1}/${pendingUris.size}"

            // 将 Uri 转为文件路径（复制到缓存目录后上传）
            copyUriToCache(uri, fileName) { cachePath ->
                if (cachePath == null) {
                    // 复制失败，跳过
                    index++
                    uploadNext()
                    return@copyUriToCache
                }

                // 更新进度（开始上传）
                progressTransfer.progress = 0
                textProgress.text = "0%"

                CyrioDeviceManager.uploadFile(
                    memUnit, cachePath, applySlug, applyStrip
                ) { fileNo ->
                    if (fileNo > 0) {
                        successCount++
                    }

                    // 更新进度
                    progressTransfer.progress = 100
                    textProgress.text = "100%"

                    // 删除缓存文件
                    try {
                        java.io.File(cachePath).delete()
                    } catch (e: Exception) { }

                    index++
                    uploadNext()
                }
            }
        }

        uploadNext()
    }

    /** 获取文件名 */
    private fun getFileName(uri: Uri): String {
        var name = "unknown"
        val cursor = activity?.contentResolver?.query(uri, null, null, null, null)
        cursor?.use {
            val nameIndex = it.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
            if (nameIndex >= 0 && it.moveToFirst()) {
                name = it.getString(nameIndex)
            }
        }
        return name
    }

    /** 将 Uri 对应的文件复制到缓存目录，返回缓存文件路径 */
    private fun copyUriToCache(uri: Uri, fileName: String, callback: (String?) -> Unit) {
        Thread {
            try {
                val cacheDir = requireContext().cacheDir
                val cacheFile = java.io.File(cacheDir, "upload_${System.currentTimeMillis()}_$fileName")
                requireContext().contentResolver.openInputStream(uri)?.use { input ->
                    java.io.FileOutputStream(cacheFile).use { output ->
                        input.copyTo(output)
                    }
                }
                requireActivity().runOnUiThread { callback(cacheFile.absolutePath) }
            } catch (e: Exception) {
                requireActivity().runOnUiThread { callback(null) }
            }
        }.start()
    }

    /** 显示空闲状态 */
    private fun showIdle() {
        layoutIdle.visibility = View.VISIBLE
        layoutTransferring.visibility = View.GONE
    }

    /** 显示传输中状态 */
    private fun showTransferring() {
        layoutIdle.visibility = View.GONE
        layoutTransferring.visibility = View.VISIBLE
        progressTransfer.progress = 0
        textProgress.text = "0%"
    }
}
