package c.cyrio.android.model

import org.json.JSONObject

/**
 * 歌单数据模型
 *
 * 对应 Rust 侧 PlaylistJson：
 * {"fileNo","size","name","title"}
 */
data class Playlist(
    val fileNo: Int,
    val size: Int,
    val name: String,
    val title: String
) {
    /** 歌单名称（优先 title，空则用 name） */
    val displayName: String
        get() = if (title.isNotBlank()) title else name

    companion object {
        fun parseList(json: String): List<Playlist> {
            if (json.isBlank() || json.trim() == "[]") return emptyList()
            val arr = org.json.JSONArray(json)
            val result = ArrayList<Playlist>(arr.length())
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                result.add(Playlist(
                    fileNo = o.optInt("fileNo"),
                    size = o.optInt("size"),
                    name = o.optString("name"),
                    title = o.optString("title")
                ))
            }
            return result
        }
    }
}
