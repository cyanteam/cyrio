package c.cyrio.android.fragment

import android.os.Bundle
import android.text.Editable
import android.text.TextWatcher
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.fragment.app.Fragment
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import c.cyrio.android.R
import c.cyrio.android.adapter.PlaylistAdapter
import c.cyrio.android.model.Playlist
import c.cyrio.android.util.CyrioDeviceManager

/**
 * 歌单列表页
 *
 * 行为：
 * - 启动时显示"连接中..."转圈（由 MainActivity 自动发起连接）
 * - 连接成功后自动加载所有歌单
 * - 连接失败显示"未连接设备"
 * - 单击歌单跳转到 PlaylistDetailFragment
 * - 长按弹出删除确认
 */
class PlaylistsFragment : Fragment(), PlaylistAdapter.PlaylistAdapterListener {

    private lateinit var recyclerPlaylists: RecyclerView
    private lateinit var textEmpty: TextView
    private lateinit var layoutLoading: LinearLayout
    private lateinit var textLoading: TextView
    private lateinit var editSearch: EditText
    private lateinit var textCount: TextView
    private lateinit var textTitle: TextView
    private lateinit var btnNewPlaylist: TextView
    private lateinit var adapter: PlaylistAdapter

    private var allPlaylists: List<Playlist> = emptyList()
    private var filteredPlaylists: List<Playlist> = emptyList()
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
        return inflater.inflate(R.layout.fragment_playlists, container, false)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        recyclerPlaylists = view.findViewById(R.id.recycler_playlists)
        textEmpty = view.findViewById(R.id.text_empty)
        layoutLoading = view.findViewById(R.id.layout_loading)
        textLoading = view.findViewById(R.id.text_loading)
        editSearch = view.findViewById(R.id.edit_search)
        textCount = view.findViewById(R.id.text_count)
        textTitle = view.findViewById(R.id.text_title)
        btnNewPlaylist = view.findViewById(R.id.btn_new_playlist)

        recyclerPlaylists.layoutManager = LinearLayoutManager(requireContext())
        adapter = PlaylistAdapter(filteredPlaylists, this)
        recyclerPlaylists.adapter = adapter

        editSearch.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: Editable?) {
                filterPlaylists(s?.toString() ?: "")
            }
        })

        btnNewPlaylist.setOnClickListener { showCreatePlaylistDialog() }

        updateConnectionState()

        // 注册连接状态监听器（连接完成时自动刷新 UI）
        CyrioDeviceManager.addConnectionListener(connectionListener)
    }

    override fun onDestroyView() {
        super.onDestroyView()
        // 移除连接状态监听器，避免内存泄漏
        CyrioDeviceManager.removeConnectionListener(connectionListener)
    }

    fun updateConnectionState() {
        when {
            CyrioDeviceManager.isConnecting -> {
                // 连接中：显示转圈
                textTitle.text = "歌单(连接中...)"
                showConnecting()
            }
            CyrioDeviceManager.isConnected -> {
                textTitle.text = "歌单(${CyrioDeviceManager.deviceName})"
                if (!hasLoaded) loadPlaylists()
            }
            else -> {
                textTitle.text = "歌单(未连接)"
                hasLoaded = false
                allPlaylists = emptyList()
                filteredPlaylists = emptyList()
                adapter.updateData(emptyList())
                updateCount(0)
                showEmpty("未连接设备")
            }
        }
    }

    private fun loadPlaylists() {
        if (!CyrioDeviceManager.isConnected) {
            showEmpty("未连接设备")
            return
        }

        showLoading()
        CyrioDeviceManager.listPlaylists(0) { internalPlaylists ->
            CyrioDeviceManager.listPlaylists(1) { sdPlaylists ->
                allPlaylists = internalPlaylists + sdPlaylists
                filteredPlaylists = allPlaylists
                hasLoaded = true
                adapter.updateData(allPlaylists)
                updateCount(allPlaylists.size)
                showList()
            }
        }
    }

    private fun filterPlaylists(query: String) {
        filteredPlaylists = if (query.isBlank()) {
            allPlaylists
        } else {
            val q = query.lowercase()
            allPlaylists.filter { pl ->
                pl.displayName.lowercase().contains(q) || pl.name.lowercase().contains(q)
            }
        }
        adapter.updateData(filteredPlaylists)
        updateCount(filteredPlaylists.size)
        showList()
    }

    private fun updateCount(count: Int) { textCount.text = "$count 个" }

    private fun showCreatePlaylistDialog() {
        if (!CyrioDeviceManager.isConnected) {
            Toast.makeText(requireContext(), "未连接设备", Toast.LENGTH_SHORT).show()
            return
        }
        val input = EditText(requireContext()).apply { hint = "歌单名称" }
        AlertDialog.Builder(requireContext())
            .setTitle("新建歌单")
            .setView(input)
            .setPositiveButton(R.string.confirm) { _, _ ->
                val name = input.text.toString().trim()
                if (name.isNotBlank()) createPlaylist(name)
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    private fun createPlaylist(name: String) {
        CyrioDeviceManager.createPlaylist(name, 0) { fileNo ->
            if (fileNo > 0) {
                Toast.makeText(requireContext(), "歌单创建成功", Toast.LENGTH_SHORT).show()
                loadPlaylists()
            } else {
                Toast.makeText(requireContext(), "歌单创建失败", Toast.LENGTH_SHORT).show()
            }
        }
    }

    // === PlaylistAdapter 回调 ===

    override fun onPlaylistClick(playlist: Playlist, position: Int) {
        // 跳转到歌单详情页
        val detailFragment = PlaylistDetailFragment.newInstance(playlist)
        parentFragmentManager.beginTransaction()
            .setCustomAnimations(
                R.anim.slide_in_right, R.anim.slide_out_left,
                R.anim.slide_in_left, R.anim.slide_out_right
            )
            .replace(R.id.fragment_container, detailFragment, "playlist_detail")
            .addToBackStack("playlist_detail")
            .commit()
    }

    override fun onPlaylistLongClick(playlist: Playlist, position: Int) {
        AlertDialog.Builder(requireContext())
            .setTitle("删除歌单")
            .setMessage("确认删除歌单「${playlist.displayName}」？")
            .setPositiveButton(R.string.confirm) { _, _ ->
                CyrioDeviceManager.deleteFile(0, playlist.fileNo) { ok ->
                    if (ok) {
                        adapter.removeItem(position)
                        allPlaylists = allPlaylists.toMutableList().also { list ->
                            list.removeAll { it.fileNo == playlist.fileNo }
                        }
                        updateCount(allPlaylists.size)
                        Toast.makeText(requireContext(), "已删除", Toast.LENGTH_SHORT).show()
                    } else {
                        Toast.makeText(requireContext(), "删除失败", Toast.LENGTH_SHORT).show()
                    }
                }
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    // === 状态切换 ===

    private fun showConnecting() {
        layoutLoading.visibility = View.VISIBLE
        textLoading.text = "连接中..."
        recyclerPlaylists.visibility = View.GONE
        textEmpty.visibility = View.GONE
    }

    private fun showLoading() {
        layoutLoading.visibility = View.VISIBLE
        textLoading.text = "加载中..."
        recyclerPlaylists.visibility = View.GONE
        textEmpty.visibility = View.GONE
    }

    private fun showList() {
        if (filteredPlaylists.isEmpty()) {
            showEmpty("暂无歌单")
        } else {
            layoutLoading.visibility = View.GONE
            textEmpty.visibility = View.GONE
            recyclerPlaylists.visibility = View.VISIBLE
        }
    }

    private fun showEmpty(message: String) {
        layoutLoading.visibility = View.GONE
        recyclerPlaylists.visibility = View.GONE
        textEmpty.visibility = View.VISIBLE
        textEmpty.text = message
        updateCount(0)
    }

    override fun onResume() {
        super.onResume()
        updateConnectionState()
    }
}
