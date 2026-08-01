package c.cyrio.android.model

import org.json.JSONObject

/**
 * 歌曲数据模型
 *
 * 对应 Rust 侧 SongJson：
 * {"fileNo","size","time","bitRate","sampleRate","name","title","artist","album","memUnit"}
 */
data class Song(
    val fileNo: Int,
    val size: Int,
    val time: Int,
    val bitRate: Int,
    val sampleRate: Int,
    val name: String,
    val title: String,
    val artist: String,
    val album: String,
    val memUnit: Int
) {
    /** 文件大小格式化（KB/MB） */
    val sizeText: String
        get() = formatSize(size)

    /** 时长格式化 mm:ss */
    val timeText: String
        get() {
            val m = time / 60
            val s = time % 60
            return "%d:%02d".format(m, s)
        }

    /** 比特率格式化 */
    val bitRateText: String
        get() = "${bitRate / 1000}kbps"

    /** 存储位置文本 */
    val memUnitText: String
        get() = if (memUnit == 0) "内置" else "SD卡"

    companion object {
        /** 从 JSON 数组字符串解析歌曲列表 */
        fun parseList(json: String): List<Song> {
            if (json.isBlank() || json.trim() == "[]") return emptyList()
            val arr = org.json.JSONArray(json)
            val result = ArrayList<Song>(arr.length())
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                result.add(parse(o))
            }
            return result
        }

        /** 从 JSONObject 解析单首歌曲 */
        fun parse(o: JSONObject): Song {
            return Song(
                fileNo = o.optInt("fileNo"),
                size = o.optInt("size"),
                time = o.optInt("time"),
                bitRate = o.optInt("bitRate"),
                sampleRate = o.optInt("sampleRate"),
                name = o.optString("name"),
                title = o.optString("title"),
                artist = o.optString("artist"),
                album = o.optString("album"),
                memUnit = o.optInt("memUnit")
            )
        }

        /** 格式化文件大小 */
        fun formatSize(bytes: Int): String {
            if (bytes < 1024) return "${bytes}B"
            if (bytes < 1024 * 1024) return "%.1fKB".format(bytes / 1024.0)
            return "%.1fMB".format(bytes / (1024.0 * 1024.0))
        }
    }
}
