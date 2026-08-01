package c.cyrio.android.fragment

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageButton
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.fragment.app.Fragment
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import c.cyrio.android.R
import c.cyrio.android.adapter.SongAdapter
import c.cyrio.android.model.Playlist
import c.cyrio.android.model.Song
import c.cyrio.android.util.CyrioDeviceManager

/**
 * 歌单详情页 — 显示歌单内的歌曲列表
 *
 * 单击歌单从 PlaylistsFragment 跳转到此页面。
 * 支持从歌单移除歌曲。
 */
class PlaylistDetailFragment : Fragment(), SongAdapter.SongAdapterListener {

    private lateinit var btnBack: ImageButton
    private lateinit var textTitle: TextView
    private lateinit var recyclerSongs: RecyclerView
    private lateinit var textEmpty: TextView
    private lateinit var progressLoading: ProgressBar
    private lateinit var adapter: SongAdapter

    private var playlist: Playlist? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // 从 arguments 获取歌单信息
        playlist = arguments?.let {
            Playlist(
                fileNo = it.getInt(ARG_FILE_NO),
                size = it.getInt(ARG_SIZE),
                name = it.getString(ARG_NAME) ?: "",
                title = it.getString(ARG_TITLE) ?: ""
            )
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return inflater.inflate(R.layout.fragment_playlist_detail, container, false)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        btnBack = view.findViewById(R.id.btn_back)
        textTitle = view.findViewById(R.id.text_title)
        recyclerSongs = view.findViewById(R.id.recycler_playlist_songs)
        textEmpty = view.findViewById(R.id.text_empty)
        progressLoading = view.findViewById(R.id.progress_loading)

        textTitle.text = playlist?.displayName ?: "歌单"

        btnBack.setOnClickListener {
            parentFragmentManager.popBackStack()
        }

        recyclerSongs.layoutManager = LinearLayoutManager(requireContext())
        adapter = SongAdapter(emptyList(), this)
        recyclerSongs.adapter = adapter

        loadPlaylistSongs()
    }

    private fun loadPlaylistSongs() {
        val pl = playlist ?: return
        if (!CyrioDeviceManager.isConnected) {
            textEmpty.text = "未连接设备"
            textEmpty.visibility = View.VISIBLE
            return
        }

        progressLoading.visibility = View.VISIBLE
        textEmpty.visibility = View.GONE

        CyrioDeviceManager.listPlaylistSongs(pl.fileNo, 0) { songs ->
            progressLoading.visibility = View.GONE
            if (songs.isEmpty()) {
                textEmpty.visibility = View.VISIBLE
            } else {
                textEmpty.visibility = View.GONE
                recyclerSongs.visibility = View.VISIBLE
                adapter.updateData(songs)
            }
        }
    }

    override fun onSongClick(song: Song, position: Int) {}

    override fun onSongLongClick(song: Song, position: Int) {
        // 从歌单移除
        val pl = playlist ?: return
        AlertDialog.Builder(requireContext())
            .setTitle("移除歌曲")
            .setMessage("确认从歌单移除「${song.title.ifBlank { song.name }}」？")
            .setPositiveButton(R.string.confirm) { _, _ ->
                CyrioDeviceManager.removeFromPlaylist(pl.fileNo, 0, position) { ok ->
                    if (ok) {
                        Toast.makeText(requireContext(), "已移除", Toast.LENGTH_SHORT).show()
                        loadPlaylistSongs()
                    } else {
                        Toast.makeText(requireContext(), "移除失败", Toast.LENGTH_SHORT).show()
                    }
                }
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    override fun onSelectionChanged(selectedCount: Int) {}

    companion object {
        private const val ARG_FILE_NO = "fileNo"
        private const val ARG_SIZE = "size"
        private const val ARG_NAME = "name"
        private const val ARG_TITLE = "title"

        fun newInstance(playlist: Playlist): PlaylistDetailFragment {
            return PlaylistDetailFragment().apply {
                arguments = Bundle().apply {
                    putInt(ARG_FILE_NO, playlist.fileNo)
                    putInt(ARG_SIZE, playlist.size)
                    putString(ARG_NAME, playlist.name)
                    putString(ARG_TITLE, playlist.title)
                }
            }
        }
    }
}
