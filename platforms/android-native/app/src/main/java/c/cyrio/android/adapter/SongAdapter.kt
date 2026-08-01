package c.cyrio.android.adapter

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.RecyclerView
import c.cyrio.android.R
import c.cyrio.android.model.Song

/**
 * 歌曲列表适配器（紧凑版）
 *
 * 列表项：标题 + 元信息行（存储 · 大小 · 时长 · 比特率）
 *
 * 交互逻辑：
 * - 单击：切换选中状态（批量模式）或设置活跃行
 * - 长按：触发回调（由 Fragment 显示 BottomSheet）
 */
class SongAdapter(
    private var songs: List<Song> = emptyList(),
    private val listener: SongAdapterListener
) : RecyclerView.Adapter<SongAdapter.ViewHolder>() {

    private val selectedPositions = mutableSetOf<Int>()
    private var activePosition = -1
    private var batchMode = false

    interface SongAdapterListener {
        fun onSongClick(song: Song, position: Int)
        fun onSongLongClick(song: Song, position: Int)
        fun onSelectionChanged(selectedCount: Int)
    }

    class ViewHolder(itemView: View) : RecyclerView.ViewHolder(itemView) {
        val root: LinearLayout = itemView.findViewById(R.id.item_root)
        val titleText: TextView = itemView.findViewById(R.id.text_title)
        val metaText: TextView = itemView.findViewById(R.id.text_meta)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val view = LayoutInflater.from(parent.context)
            .inflate(R.layout.item_song, parent, false)
        return ViewHolder(view)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        val song = songs[position]
        val context = holder.root.context

        // 标题
        holder.titleText.text = if (song.title.isNotBlank()) song.title else song.name

        // 元信息行：存储位置 · 大小 · 时长 · 比特率
        val meta = buildString {
            append(song.memUnitText)
            append(" · ")
            append(song.sizeText)
            append(" · ")
            append(song.timeText)
            append(" · ")
            append(song.bitRateText)
            if (song.artist.isNotBlank()) {
                append(" · ")
                append(song.artist)
            }
        }
        holder.metaText.text = meta

        // 选中/活跃状态 — 背景色切换
        val isSelected = selectedPositions.contains(position)
        val isActive = activePosition == position

        val bgColor = when {
            isSelected -> ContextCompat.getColor(context, R.color.md_primary_container)
            isActive -> ContextCompat.getColor(context, R.color.md_secondary_container)
            else -> ContextCompat.getColor(context, R.color.md_surface)
        }
        holder.root.setBackgroundColor(bgColor)

        holder.root.setOnClickListener {
            if (batchMode) {
                toggleSelection(position)
            } else {
                setActivePosition(position)
                listener.onSongClick(song, position)
            }
        }

        holder.root.setOnLongClickListener {
            if (!batchMode) {
                enterBatchMode()
                toggleSelection(position)
            }
            listener.onSongLongClick(song, position)
            true
        }
    }

    override fun getItemCount(): Int = songs.size

    fun updateData(newSongs: List<Song>) {
        songs = newSongs
        selectedPositions.clear()
        activePosition = -1
        batchMode = false
        notifyDataSetChanged()
    }

    fun enterBatchMode() { batchMode = true }

    fun exitBatchMode() {
        batchMode = false
        selectedPositions.clear()
        notifyDataSetChanged()
        listener.onSelectionChanged(0)
    }

    private fun toggleSelection(position: Int) {
        if (selectedPositions.contains(position)) {
            selectedPositions.remove(position)
        } else {
            selectedPositions.add(position)
        }
        notifyItemChanged(position)
        listener.onSelectionChanged(selectedPositions.size)
        if (selectedPositions.isEmpty() && batchMode) batchMode = false
    }

    private fun setActivePosition(position: Int) {
        val old = activePosition
        activePosition = position
        if (old >= 0) notifyItemChanged(old)
        notifyItemChanged(position)
    }

    fun selectAll() {
        selectedPositions.clear()
        for (i in songs.indices) selectedPositions.add(i)
        batchMode = true
        notifyDataSetChanged()
        listener.onSelectionChanged(selectedPositions.size)
    }

    fun clearSelection() {
        selectedPositions.clear()
        notifyDataSetChanged()
        listener.onSelectionChanged(0)
    }

    fun getSelectedSongs(): List<Song> = selectedPositions.mapNotNull { songs.getOrNull(it) }
    fun isBatchMode(): Boolean = batchMode
    fun getSelectedCount(): Int = selectedPositions.size
}
