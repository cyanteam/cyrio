package com.cyrio.core.api;

import com.cyrio.core.model.Playlist;
import com.cyrio.core.model.Song;
import com.cyrio.core.model.StorageInfo;
import com.cyrio.jni.CyrioNative;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.util.ArrayList;
import java.util.List;

/**
 * Diamond Rio S-Series 设备高级 API（JNI 实现）
 *
 * <p>所有核心逻辑（USB 协议、文件传输、FIDL 解析等）由 Rust 的 cyrio-core 实现，
 * 本类通过 JNI 桥接调用，Java 仅负责参数转换和 JSON 解析。
 *
 * <h3>架构</h3>
 * <pre>
 * CyrioDevice (Java)  →  CyrioNative (JNI)  →  cyrio-jni (Rust)  →  cyrio-core
 * </pre>
 *
 * <h3>使用方式</h3>
 * <pre>{@code
 * CyrioDevice device = new CyrioDevice();
 * device.open();                      // 自动扫描 + 协议初始化
 * List<Song> songs = device.listSongs(MEM_UNIT_INTERNAL);
 * device.close();
 * }</pre>
 */
public class CyrioDevice {

    /** JSON 解析器（线程安全，可共享） */
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** 内存单元常量 */
    public static final byte MEM_UNIT_INTERNAL = 0;
    public static final byte MEM_UNIT_SDCARD = 1;

    // ========================================================================
    // 内部数据类
    // ========================================================================

    /**
     * 歌单内单首歌曲的信息（含在歌单中的序号和实际所在内存单元）
     */
    public static class PlaylistSong {
        public final Song song;
        public final int index;
        public final byte memUnit;

        public PlaylistSong(Song song, int index, byte memUnit) {
            this.song = song;
            this.index = index;
            this.memUnit = memUnit;
        }
    }

    /**
     * {@code createPlaylist} 的返回结果
     */
    public static class CreatePlaylistResult {
        public final int fileNo;
        public final Playlist playlist;

        public CreatePlaylistResult(int fileNo, Playlist playlist) {
            this.fileNo = fileNo;
            this.playlist = playlist;
        }
    }

    // ========================================================================
    // 成员变量
    // ========================================================================

    /** JNI 设备句柄（0 = 未连接） */
    private long handle = 0;

    // ========================================================================
    // 设备管理
    // ========================================================================

    /**
     * 打开设备（自动扫描 VID=0x045a，完成 USB 协议握手）
     *
     * @throws RuntimeException 打开失败
     */
    public void open() throws RuntimeException {
        handle = CyrioNative.openDevice();
        if (handle == 0) {
            throw new RuntimeException("打开设备失败：未找到 Diamond Rio 设备");
        }
    }

    /**
     * 以指定 VID/PID 强制打开设备
     */
    public void openWithVidPid(int vid, int pid) throws RuntimeException {
        handle = CyrioNative.openDeviceWithVidPid(vid, pid);
        if (handle == 0) {
            throw new RuntimeException("打开设备失败：VID=" + vid + " PID=" + pid);
        }
    }

    /**
     * 关闭设备并释放资源
     */
    public void close() {
        if (handle != 0) {
            CyrioNative.closeDevice(handle);
            handle = 0;
        }
    }

    /**
     * 设备是否已连接
     */
    public boolean isConnected() {
        return handle != 0 && CyrioNative.isConnected(handle);
    }

    /** 获取 JNI 设备句柄（内部使用） */
    public long getHandle() {
        return handle;
    }

    // ========================================================================
    // 文件 / 存储查询
    // ========================================================================

    /**
     * 列出内存单元中的所有歌曲
     *
     * @param memUnit 内存单元 (0=内置, 1=SD卡)
     * @return 歌曲列表
     */
    public List<Song> listSongs(byte memUnit) {
        String json = CyrioNative.listSongs(handle, memUnit);
        return parseSongList(json);
    }

    /**
     * 获取存储信息
     *
     * @param memUnit 内存单元
     * @return 存储信息
     */
    public StorageInfo getStorageInfo(byte memUnit) {
        String json = CyrioNative.getStorage(handle, memUnit);
        return parseStorageInfo(json);
    }

    // ========================================================================
    // 文件操作
    // ========================================================================

    /**
     * 上传 MP3 文件到设备
     *
     * @param memUnit    目标内存单元
     * @param filePath   本地 MP3 文件路径
     * @param applySlug  是否应用拼音转换
     * @param applyStrip 是否应用去词
     * @return 设备分配的新文件号 (＞0 成功, -1 失败)
     */
    public int uploadFile(byte memUnit, String filePath, boolean applySlug, boolean applyStrip) {
        return CyrioNative.uploadFile(handle, memUnit, filePath, applySlug, applyStrip);
    }

    /**
     * 下载文件到本地路径
     *
     * @param memUnit    内存单元
     * @param fileNo     文件号
     * @param outputPath 本地输出路径
     * @return true 成功
     */
    public boolean downloadFile(byte memUnit, int fileNo, String outputPath) {
        return CyrioNative.downloadFile(handle, memUnit, fileNo, outputPath);
    }

    /**
     * 删除设备上的文件
     *
     * @param memUnit 内存单元
     * @param fileNo  文件号
     * @return true 成功
     */
    public boolean deleteFile(byte memUnit, int fileNo) {
        return CyrioNative.deleteFile(handle, memUnit, fileNo);
    }

    // ========================================================================
    // 歌单操作
    // ========================================================================

    /**
     * 列出所有歌单
     *
     * @param memUnit 内存单元
     * @return 歌单列表
     */
    public List<Playlist> listPlaylists(byte memUnit) {
        String json = CyrioNative.listPlaylists(handle, memUnit);
        return parsePlaylistList(json);
    }

    /**
     * 创建新歌单
     *
     * @param name    歌单名称
     * @param memUnit 内存单元
     * @return 创建结果
     */
    public CreatePlaylistResult createPlaylist(String name, byte memUnit) {
        int fileNo = CyrioNative.createPlaylist(handle, name, memUnit);
        if (fileNo <= 0) {
            throw new RuntimeException("创建歌单失败: " + name);
        }
        Playlist playlist = new Playlist();
        playlist.fileNo = fileNo;
        playlist.name = name;
        playlist.title = name;
        playlist.memUnit = memUnit;
        return new CreatePlaylistResult(fileNo, playlist);
    }

    /**
     * 添加歌曲到歌单
     *
     * @param songFileNo      歌曲文件号
     * @param songMemUnit     歌曲所在内存单元
     * @param playlistFileNo  歌单文件号
     * @param playlistMemUnit 歌单所在内存单元
     * @return true 成功
     */
    public boolean addToPlaylist(int songFileNo, byte songMemUnit,
                                 int playlistFileNo, byte playlistMemUnit) {
        return CyrioNative.addToPlaylist(handle, songFileNo, songMemUnit,
                playlistFileNo, playlistMemUnit);
    }

    /**
     * 列出歌单内的歌曲
     *
     * @param playlistFileNo 歌单文件号
     * @param memUnit        歌单所在内存单元
     * @return 歌单内歌曲列表（含序号）
     */
    public List<PlaylistSong> listPlaylistSongs(int playlistFileNo, byte memUnit) {
        String json = CyrioNative.listPlaylistSongs(handle, playlistFileNo, memUnit);
        return parsePlaylistSongList(json);
    }

    /**
     * 从歌单中移除指定位置的歌曲
     *
     * @param playlistFileNo 歌单文件号
     * @param memUnit        歌单所在内存单元
     * @param index          条目索引 (0-based，来自 listPlaylistSongs 返回的 index)
     * @return true 成功
     */
    public boolean removeFromPlaylist(int playlistFileNo, byte memUnit, int index) {
        return CyrioNative.removeFromPlaylist(handle, playlistFileNo, memUnit, index);
    }

    // ========================================================================
    // 重命名 / 编码修复
    // ========================================================================

    /**
     * 重命名歌曲（修改 name 和 title 字段）
     *
     * @param fileNo  文件号
     * @param memUnit 内存单元
     * @param newName 新名称
     * @return true 成功
     */
    public boolean renameSong(int fileNo, byte memUnit, String newName) {
        return CyrioNative.renameSong(handle, fileNo, memUnit, newName);
    }

    /**
     * 修复歌曲编码（双重编码 → 正确 UTF-8）
     *
     * @param fileNo  文件号
     * @param memUnit 内存单元
     * @return true 成功
     */
    public boolean repairEncoding(int fileNo, byte memUnit) {
        return CyrioNative.repairEncoding(handle, fileNo, memUnit);
    }

    // ========================================================================
    // JSON 解析
    // ========================================================================

    /**
     * 解析歌曲列表 JSON
     */
    private static List<Song> parseSongList(String json) {
        List<Song> result = new ArrayList<>();
        if (json == null || json.isEmpty() || "[]".equals(json)) {
            return result;
        }
        try {
            JsonNode root = MAPPER.readTree(json);
            if (root.isArray()) {
                for (JsonNode node : root) {
                    result.add(parseSong(node));
                }
            }
        } catch (Exception e) {
            System.err.println("parseSongList error: " + e.getMessage());
        }
        return result;
    }

    /**
     * 解析单个歌曲 JSON 节点
     */
    private static Song parseSong(JsonNode node) {
        Song song = new Song();
        song.fileNo = node.path("fileNo").asInt(0);
        song.size = node.path("size").asLong(0);
        song.time = node.path("time").asInt(0);
        song.bitRate = node.path("bitRate").asInt(0);
        song.sampleRate = node.path("sampleRate").asInt(0);
        song.name = node.path("name").asText("");
        song.title = node.path("title").asText("");
        song.artist = node.path("artist").asText("");
        song.album = node.path("album").asText("");
        song.memUnit = (byte) node.path("memUnit").asInt(0);
        return song;
    }

    /**
     * 解析歌单列表 JSON
     */
    private static List<Playlist> parsePlaylistList(String json) {
        List<Playlist> result = new ArrayList<>();
        if (json == null || json.isEmpty() || "[]".equals(json)) {
            return result;
        }
        try {
            JsonNode root = MAPPER.readTree(json);
            if (root.isArray()) {
                for (JsonNode node : root) {
                    Playlist pl = new Playlist();
                    pl.fileNo = node.path("fileNo").asInt(0);
                    pl.size = node.path("size").asLong(0);
                    pl.name = node.path("name").asText("");
                    pl.title = node.path("title").asText("");
                    result.add(pl);
                }
            }
        } catch (Exception e) {
            System.err.println("parsePlaylistList error: " + e.getMessage());
        }
        return result;
    }

    /**
     * 解析存储信息 JSON
     */
    private static StorageInfo parseStorageInfo(String json) {
        StorageInfo info = new StorageInfo();
        if (json == null || json.isEmpty() || "{}".equals(json)) {
            return info;
        }
        try {
            JsonNode node = MAPPER.readTree(json);
            info.totalSize = node.path("totalSize").asLong(0);
            info.usedSize = node.path("usedSize").asLong(0);
            info.freeSize = node.path("freeSize").asLong(0);
            info.systemSize = node.path("systemSize").asLong(0);
            info.name = node.path("name").asText("");
            info.model = node.path("model").asText("");
            info.isPresent = node.path("isPresent").asBoolean(false);
        } catch (Exception e) {
            System.err.println("parseStorageInfo error: " + e.getMessage());
        }
        return info;
    }

    /**
     * 解析歌单内歌曲列表 JSON
     */
    private static List<PlaylistSong> parsePlaylistSongList(String json) {
        List<PlaylistSong> result = new ArrayList<>();
        if (json == null || json.isEmpty() || "[]".equals(json)) {
            return result;
        }
        try {
            JsonNode root = MAPPER.readTree(json);
            if (root.isArray()) {
                for (JsonNode node : root) {
                    Song song = parseSong(node);
                    int index = node.path("index").asInt(0);
                    byte memUnit = (byte) node.path("memUnit").asInt(0);
                    result.add(new PlaylistSong(song, index, memUnit));
                }
            }
        } catch (Exception e) {
            System.err.println("parsePlaylistSongList error: " + e.getMessage());
        }
        return result;
    }
}
