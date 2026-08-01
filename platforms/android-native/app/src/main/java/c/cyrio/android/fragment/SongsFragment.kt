package c.cyrio.android.fragment

import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.EditText
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.fragment.app.Fragment
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import com.google.android.material.bottomsheet.BottomSheetDialog
import c.cyrio.android.R
import c.cyrio.android.adapter.SongAdapter
import c.cyrio.android.model.Song
import c.cyrio.android.util.CyrioDeviceManager

/**
 * 歌曲列表页
 *
 * 行为：
 * - 启动时显示"连接中..."转圈（由 MainActivity 自动发起连接）
 * - 连接成功后自动加载所有歌曲
 * - 连接失败显示"未连接设备"
 * - 顶部标题动态显示"歌曲(设备名/连接中.../未连接)"
 * - 长按弹出底部菜单（BottomSheet）
 */
class SongsFragment : Fragment(), SongAdapter.SongAdapterListener {

    private lateinit var recyclerSongs: RecyclerView
    private lateinit var textEmpty: TextView
    private lateinit var layoutLoading: LinearLayout
    private lateinit var textLoading: TextView
    private lateinit var editSearch: EditText
    private lateinit var textCount: TextView
    private lateinit var textTitle: TextView
    private lateinit var batchToolbar: LinearLayout
    private lateinit var btnRefresh: ImageButton

    private lateinit var btnSelectAll: TextView
    private lateinit var btnClear: TextView
    private lateinit var btnDelete: TextView
    private lateinit var btnAddToPlaylist: TextView
    private lateinit var btnMore: TextView
    private lateinit var btnRefreshBatch: TextView

    private lateinit var adapter: SongAdapter

    private var allSongs: List<Song> = emptyList()
    private var filteredSongs: List<Song> = emptyList()
    private var loading = false
    private var hasLoaded = false

    /** 连接状态监听器（连接完成时自动更新 UI） */
    private val connectionListener: (Boolean) -> Unit = { _ ->
        updateConnectionState()
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return inflater.inflate(R.layout.fragment_songs, container, false)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        recyclerSongs = view.findViewById(R.id.recycler_songs)
        textEmpty = view.findViewById(R.id.text_empty)
        layoutLoading = view.findViewById(R.id.layout_loading)
        textLoading = view.findViewById(R.id.text_loading)
        editSearch = view.findViewById(R.id.edit_search)
        textCount = view.findViewById(R.id.text_count)
        textTitle = view.findViewById(R.id.text_title)
        batchToolbar = view.findViewById(R.id.batch_toolbar)
        btnRefresh = view.findViewById(R.id.btn_refresh)

        btnSelectAll = view.findViewById(R.id.btn_select_all)
        btnClear = view.findViewById(R.id.btn_clear)
        btnDelete = view.findViewById(R.id.btn_delete)
        btnAddToPlaylist = view.findViewById(R.id.btn_add_to_playlist)
        btnMore = view.findViewById(R.id.btn_more)
        btnRefreshBatch = view.findViewById(R.id.btn_refresh_batch)

        recyclerSongs.layoutManager = LinearLayoutManager(requireContext())
        adapter = SongAdapter(filteredSongs, this)
        recyclerSongs.adapter = adapter

        editSearch.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: Editable?) {
                filterSongs(s?.toString() ?: "")
            }
        })

        btnRefresh.setOnClickListener { loadSongs() }
        btnRefreshBatch.setOnClickListener { loadSongs() }
        btnSelectAll.setOnClickListener { adapter.selectAll() }
        btnClear.setOnClickListener { adapter.exitBatchMode(); hideBatchToolbar() }
        btnDelete.setOnClickListener { deleteSelectedSongs() }
        btnAddToPlaylist.setOnClickListener {
            Toast.makeText(requireContext(), "加入歌单功能开发中", Toast.LENGTH_SHORT).show()
        }
        btnMore.setOnClickListener { showMoreOptions() }

        // 默认显示未连接状态
        updateConnectionState()

        // 注册连接状态监听器（连接完成时自动刷新 UI）
        CyrioDeviceManager.addConnectionListener(connectionListener)
    }

    override fun onDestroyView() {
        super.onDestroyView()
        // 移除连接状态监听器，避免内存泄漏
        CyrioDeviceManager.removeConnectionListener(connectionListener)
    }

    /** 更新连接状态和标题 */
    fun updateConnectionState() {
        when {
            CyrioDeviceManager.isConnecting -> {
                // 连接中：显示转圈
                textTitle.text = "歌曲(连接中...)"
                showConnecting()
            }
            CyrioDeviceManager.isConnected -> {
                textTitle.text = "歌曲(${CyrioDeviceManager.deviceName})"
                if (!hasLoaded && !loading) {
                    loadSongs()
                }
            }
            else -> {
                textTitle.text = "歌曲(未连接)"
                hasLoaded = false
                allSongs = emptyList()
                filteredSongs = emptyList()
                adapter.updateData(emptyList())
                updateCount(0)
                showEmpty("未连接设备")
            }
        }
    }

    /** 加载歌曲列表 */
    private fun loadSongs() {
        if (!CyrioDeviceManager.isConnected) {
            showEmpty("未连接设备")
            return
        }

        loading = true
        showLoading()

        CyrioDeviceManager.listAllSongs { songs ->
            loading = false
            hasLoaded = true
            allSongs = songs
            filteredSongs = songs
            adapter.updateData(songs)
            updateCount(songs.size)
            showList()
        }
    }

    private fun filterSongs(query: String) {
        filteredSongs = if (query.isBlank()) {
            allSongs
        } else {
            val q = query.lowercase()
            allSongs.filter { song ->
                song.title.lowercase().contains(q) ||
                song.artist.lowercase().contains(q) ||
                song.album.lowercase().contains(q) ||
                song.name.lowercase().contains(q)
            }
        }
        adapter.updateData(filteredSongs)
        updateCount(filteredSongs.size)
        showList()
    }

    private fun updateCount(count: Int) {
        textCount.text = getString(R.string.song_count_format, count)
    }

    // === SongAdapter 回调 ===

    override fun onSongClick(song: Song, position: Int) {
        // 非批量模式下单击仅高亮
    }

    override fun onSongLongClick(song: Song, position: Int) {
        showSongBottomSheet(song)
    }

    override fun onSelectionChanged(selectedCount: Int) {
        updateBatchButtons()
        if (selectedCount == 0) hideBatchToolbar()
    }

    // === 底部弹出菜单 ===

    private fun showSongBottomSheet(song: Song) {
        val sheet = BottomSheetDialog(requireContext())
        val view = layoutInflater.inflate(R.layout.bottom_sheet_song, null)

        view.findViewById<TextView>(R.id.text_sheet_title).text =
            if (song.title.isNotBlank()) song.title else song.name

        view.findViewById<TextView>(R.id.btn_sheet_rename).setOnClickListener {
            sheet.dismiss()
            showRenameDialog(song)
        }
        view.findViewById<TextView>(R.id.btn_sheet_repair).setOnClickListener {
            sheet.dismiss()
            repairEncoding(song)
        }
        view.findViewById<TextView>(R.id.btn_sheet_download).setOnClickListener {
            sheet.dismiss()
            Toast.makeText(requireContext(), "下载功能开发中", Toast.LENGTH_SHORT).show()
        }
        view.findViewById<TextView>(R.id.btn_sheet_delete).setOnClickListener {
            sheet.dismiss()
            confirmDelete(song)
        }

        sheet.setContentView(view)
        sheet.show()
    }

    private fun showRenameDialog(song: Song) {
        val input = EditText(requireContext()).apply {
            setText(song.title.ifBlank { song.name })
            setSelection(text.length)
        }
        AlertDialog.Builder(requireContext())
            .setTitle("重命名")
            .setView(input)
            .setPositiveButton(R.string.confirm) { _, _ ->
                val newName = input.text.toString().trim()
                if (newName.isNotBlank()) {
                    CyrioDeviceManager.renameSong(song.fileNo, song.memUnit, newName) { ok ->
                        Toast.makeText(requireContext(),
                            if (ok) "重命名成功" else "重命名失败",
                            Toast.LENGTH_SHORT).show()
                        if (ok) loadSongs()
                    }
                }
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    private fun repairEncoding(song: Song) {
        CyrioDeviceManager.repairEncoding(song.fileNo, song.memUnit) { ok ->
            Toast.makeText(requireContext(),
                if (ok) "编码修复成功" else "编码修复失败",
                Toast.LENGTH_SHORT).show()
            if (ok) loadSongs()
        }
    }

    private fun confirmDelete(song: Song) {
        AlertDialog.Builder(requireContext())
            .setTitle("删除歌曲")
            .setMessage("确认删除「${song.title.ifBlank { song.name }}」？")
            .setPositiveButton(R.string.confirm) { _, _ ->
                CyrioDeviceManager.deleteFile(song.memUnit, song.fileNo) { ok ->
                    Toast.makeText(requireContext(),
                        if (ok) "已删除" else "删除失败",
                        Toast.LENGTH_SHORT).show()
                    if (ok) loadSongs()
                }
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    // === 批量操作 ===

    private fun showBatchToolbar() { batchToolbar.visibility = View.VISIBLE }
    private fun hideBatchToolbar() { batchToolbar.visibility = View.GONE }

    private fun updateBatchButtons() {
        val count = adapter.getSelectedCount()
        btnDelete.text = "删除($count)"
        btnAddToPlaylist.text = "加入歌单($count)"
    }

    private fun deleteSelectedSongs() {
        val selected = adapter.getSelectedSongs()
        if (selected.isEmpty()) return

        AlertDialog.Builder(requireContext())
            .setTitle("删除歌曲")
            .setMessage("确认删除 ${selected.size} 首歌曲？")
            .setPositiveButton(R.string.confirm) { _, _ ->
                var deletedCount = 0
                var index = 0
                fun deleteNext() {
                    if (index >= selected.size) {
                        Toast.makeText(requireContext(),
                            "已删除 $deletedCount 首", Toast.LENGTH_SHORT).show()
                        adapter.exitBatchMode()
                        hideBatchToolbar()
                        loadSongs()
                        return
                    }
                    val song = selected[index]
                    CyrioDeviceManager.deleteFile(song.memUnit, song.fileNo) { ok ->
                        if (ok) deletedCount++
                        index++
                        deleteNext()
                    }
                }
                deleteNext()
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    private fun showMoreOptions() {
        val selected = adapter.getSelectedSongs()
        if (selected.isEmpty()) return
        val items = arrayOf(getString(R.string.batch_slug), getString(R.string.batch_strip), getString(R.string.download))
        AlertDialog.Builder(requireContext())
            .setTitle(R.string.more)
            .setItems(items) { _, which ->
                when (which) {
                    0 -> batchProcessTitle(selected, true, false)
                    1 -> batchProcessTitle(selected, false, true)
                    2 -> Toast.makeText(requireContext(), "批量下载功能开发中", Toast.LENGTH_SHORT).show()
                }
            }
            .show()
    }

    private fun batchProcessTitle(songs: List<Song>, applySlug: Boolean, applyStrip: Boolean) {
        Toast.makeText(requireContext(), "批量处理中...", Toast.LENGTH_SHORT).show()
        var processed = 0
        var index = 0
        fun processNext() {
            if (index >= songs.size) {
                Toast.makeText(requireContext(), "已处理 $processed 首", Toast.LENGTH_SHORT).show()
                return
            }
            val song = songs[index]
            CyrioDeviceManager.processTitle(song.title, applySlug, applyStrip) { newName ->
                if (newName.isNotBlank() && newName != song.title) {
                    CyrioDeviceManager.renameSong(song.fileNo, song.memUnit, newName) { ok ->
                        if (ok) processed++
                        index++
                        processNext()
                    }
                } else { index++; processNext() }
            }
        }
        processNext()
    }

    // === 状态切换 ===

    private fun showConnecting() {
        layoutLoading.visibility = View.VISIBLE
        textLoading.text = "连接中..."
        recyclerSongs.visibility = View.GONE
        textEmpty.visibility = View.GONE
    }

    private fun showLoading() {
        layoutLoading.visibility = View.VISIBLE
        textLoading.text = "加载中..."
        recyclerSongs.visibility = View.GONE
        textEmpty.visibility = View.GONE
    }

    private fun showList() {
        if (filteredSongs.isEmpty()) {
            showEmpty("暂无歌曲")
        } else {
            layoutLoading.visibility = View.GONE
            textEmpty.visibility = View.GONE
            recyclerSongs.visibility = View.VISIBLE
        }
    }

    private fun showEmpty(message: String) {
        layoutLoading.visibility = View.GONE
        recyclerSongs.visibility = View.GONE
        textEmpty.visibility = View.VISIBLE
        textEmpty.text = message
        updateCount(0)
    }

    override fun onResume() {
        super.onResume()
        updateConnectionState()
    }
}
