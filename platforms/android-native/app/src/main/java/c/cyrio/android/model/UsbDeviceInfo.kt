package c.cyrio.android.model

/**
 * USB 设备信息
 */
data class UsbDeviceInfo(
    val vid: Int,
    val pid: Int,
    val name: String,
    val manufacturer: String,
    val serial: String
) {
    val vidHex: String get() = "0x%04x".format(vid)
    val pidHex: String get() = "0x%04x".format(pid)
    val isDiamond: Boolean get() = vid == 0x045a

    companion object {
        fun parseList(json: String): List<UsbDeviceInfo> {
            if (json.isBlank() || json.trim() == "[]") return emptyList()
            val arr = org.json.JSONArray(json)
            val result = ArrayList<UsbDeviceInfo>(arr.length())
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                result.add(UsbDeviceInfo(
                    vid = o.optInt("vid"),
                    pid = o.optInt("pid"),
                    name = o.optString("name"),
                    manufacturer = o.optString("manufacturer"),
                    serial = o.optString("serial")
                ))
            }
            return result
        }
    }
}
