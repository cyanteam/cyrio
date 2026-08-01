package c.cyrio.android.model

import org.json.JSONObject

/**
 * 存储信息模型
 *
 * 对应 Rust 侧 StorageJson：
 * {"totalSize","usedSize","freeSize","systemSize","name","model","isPresent"}
 */
data class StorageInfo(
    val totalSize: Int,
    val usedSize: Int,
    val freeSize: Int,
    val systemSize: Int,
    val name: String,
    val model: String,
    val isPresent: Boolean
) {
    /** 已用百分比 */
    val usedPercent: Int
        get() = if (totalSize > 0) (usedSize * 100 / totalSize) else 0

    /** 总容量文本 */
    val totalSizeText: String
        get() = Song.formatSize(totalSize)

    /** 已用容量文本 */
    val usedSizeText: String
        get() = Song.formatSize(usedSize)

    /** 可用容量文本 */
    val freeSizeText: String
        get() = Song.formatSize(freeSize)

    companion object {
        fun parse(json: String): StorageInfo? {
            if (json.isBlank() || json.trim() == "{}") return null
            val o = JSONObject(json)
            return StorageInfo(
                totalSize = o.optInt("totalSize"),
                usedSize = o.optInt("usedSize"),
                freeSize = o.optInt("freeSize"),
                systemSize = o.optInt("systemSize"),
                name = o.optString("name"),
                model = o.optString("model"),
                isPresent = o.optBoolean("isPresent")
            )
        }
    }
}
