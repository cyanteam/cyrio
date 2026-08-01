package c.cyrio.android.adapter

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import com.google.android.material.card.MaterialCardView
import c.cyrio.android.R
import c.cyrio.android.model.Playlist

/**
 * 歌单列表适配器（MD3 版本）
 *
 * 交互逻辑：
 * - 单击：打开歌单详情
 * - 长按：弹出上下文菜单（删除歌单）
 */
class PlaylistAdapter(
    private var playlists: List<Playlist> = emptyList(),
    private val listener: PlaylistAdapterListener
) : RecyclerView.Adapter<PlaylistAdapter.ViewHolder>() {

    interface PlaylistAdapterListener {
        /** 单击歌单，打开详情 */
        fun onPlaylistClick(playlist: Playlist, position: Int)

        /** 长按歌单，弹出上下文菜单 */
        fun onPlaylistLongClick(playlist: Playlist, position: Int)
    }

    class ViewHolder(itemView: View) : RecyclerView.ViewHolder(itemView) {
        val root: MaterialCardView = itemView.findViewById(R.id.item_root)
        val nameText: TextView = itemView.findViewById(R.id.text_name)
        val infoText: TextView = itemView.findViewById(R.id.text_info)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val view = LayoutInflater.from(parent.context)
            .inflate(R.layout.item_playlist, parent, false)
        return ViewHolder(view)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        val playlist = playlists[position]

        holder.nameText.text = playlist.displayName
        holder.infoText.text = "${playlist.size} 首歌曲"

        holder.root.setOnClickListener {
            listener.onPlaylistClick(playlist, position)
        }

        holder.root.setOnLongClickListener {
            listener.onPlaylistLongClick(playlist, position)
            true
        }
    }

    override fun getItemCount(): Int = playlists.size

    /** 更新歌单列表数据 */
    fun updateData(newPlaylists: List<Playlist>) {
        playlists = newPlaylists
        notifyDataSetChanged()
    }

    /** 获取指定位置的歌单 */
    fun getItem(position: Int): Playlist? = playlists.getOrNull(position)

    /** 删除指定位置的歌单 */
    fun removeItem(position: Int) {
        if (position in playlists.indices) {
            playlists = playlists.toMutableList().also { it.removeAt(position) }
            notifyItemRemoved(position)
            notifyItemRangeChanged(position, itemCount)
        }
    }
}
