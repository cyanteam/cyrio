package com.cyrio.core;

import com.cyrio.core.api.CyrioDevice;
import com.cyrio.core.model.Playlist;
import com.cyrio.core.model.Song;
import com.cyrio.core.model.StorageInfo;
import com.cyrio.jni.CyrioNative;
import com.cyrio.ui.DeviceView;
import com.cyrio.ui.MainWindow;
import com.cyrio.ui.PlaylistsView;
import com.cyrio.ui.SongsView;
import com.cyrio.ui.UploadView;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import javafx.application.Platform;
import javafx.scene.control.ChoiceDialog;
import javafx.scene.control.TextInputDialog;
import javafx.stage.FileChooser;

import java.io.File;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * 控制器：连接 UI 视图与 CyrioDevice
 *
 * <p>实现各视图的回调接口，在后台线程执行设备操作，
 * 结果在 JavaFX Application Thread 上更新 UI。
 *
 * <h3>线程模型</h3>
 * <ul>
 *   <li>UI 事件 → 回调 → 提交到后台线程池</li>
 *   <li>后台线程执行 JNI 调用（阻塞式）</li>
 *   <li>结果通过 Platform.runLater() 回到 UI 线程</li>
 * </ul>
 */
public class CyrioController {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** 后台操作线程池（单线程，避免 USB 并发冲突） */
    private final ExecutorService executor = Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "cyrio-device-ops");
        t.setDaemon(true);
        return t;
    });

    /** 设备实例 */
    private CyrioDevice device;

    /** 主窗口 */
    private final MainWindow mainWindow;

    /** 是否应用 slug/strip（来自设置） */
    private boolean applySlug = true;
    private boolean applyStrip = true;

    /**
     * 创建控制器并绑定到主窗口
     */
    public CyrioController(MainWindow mainWindow) {
        this.mainWindow = mainWindow;
        wireUpCallbacks();
    }

    // ========================================================================
    // 绑定回调
    // ========================================================================

    private void wireUpCallbacks() {
        // 设备视图
        mainWindow.getDeviceView().setScanCallback(this::onScanDevices);
        mainWindow.getDeviceView().setConnectCallback(new DeviceView.ConnectCallback() {
            @Override
            public void onConnectDevice(DeviceView.ScannedDevice dev) {
                connectDevice(dev.vid, dev.pid);
            }

            @Override
            public void onForceConnect(int vid, int pid) {
                connectDevice(vid, pid);
            }

            @Override
            public void onDisconnect() {
                disconnectDevice();
            }
        });

        // 主窗口连接按钮
        mainWindow.setDeviceConnectCallback(new MainWindow.DeviceConnectCallback() {
            @Override
            public void onConnect() {
                connectDevice(0x045a, 0);
            }

            @Override
            public void onDisconnect() {
                disconnectDevice();
            }
        });

        // 歌曲视图
        mainWindow.getSongsView().setCallback(new SongsView.SongActionCallback() {
            @Override
            public void onPlaySong(Song song) {
                playSong(song);
            }

            @Override
            public void onDeleteSongs(List<Song> songs) {
                deleteSongs(songs);
            }

            @Override
            public void onAddToPlaylist(List<Song> songs) {
                showAddToPlaylistDialog(songs);
            }

            @Override
            public void onDownloadSong(Song song) {
                showDownloadDialog(song);
            }

            @Override
            public void onRenameSong(Song song) {
                showRenameDialog(song);
            }

            @Override
            public void onRefresh() {
                refreshSongs();
            }
        });

        // 歌单视图
        mainWindow.getPlaylistsView().setCallback(new PlaylistsView.PlaylistActionCallback() {
            @Override
            public void onLoadPlaylists() {
                refreshPlaylists();
            }

            @Override
            public void onCreatePlaylist(String name, byte memUnit) {
                createPlaylist(name, memUnit);
            }

            @Override
            public void onDeletePlaylist(Playlist playlist) {
                deletePlaylist(playlist);
            }

            @Override
            public void onLoadPlaylistSongs(Playlist playlist) {
                loadPlaylistSongs(playlist);
            }

            @Override
            public void onPlaySong(Song song) {
                playSong(song);
            }

            @Override
            public void onRemoveFromPlaylist(Playlist playlist, Song song) {
                removeFromPlaylist(playlist, song);
            }
        });

        // 上传视图
        mainWindow.getUploadView().setCallback(this::uploadFiles);
    }

    // ========================================================================
    // 设备扫描
    // ========================================================================

    /**
     * 扫描 USB 设备（在后台线程执行）
     */
    private List<DeviceView.ScannedDevice> onScanDevices() {
        String json = CyrioNative.listUsbDevices();
        List<DeviceView.ScannedDevice> result = new ArrayList<>();
        if (json == null || json.isEmpty() || "[]".equals(json)) {
            return result;
        }
        try {
            JsonNode root = MAPPER.readTree(json);
            if (root.isArray()) {
                for (JsonNode node : root) {
                    int vid = node.path("vid").asInt(0);
                    int pid = node.path("pid").asInt(0);
                    String name = node.path("name").asText("");
                    String manufacturer = node.path("manufacturer").asText("");
                    String serial = node.path("serial").asText("");
                    result.add(new DeviceView.ScannedDevice(vid, pid, name, manufacturer, serial));
                }
            }
        } catch (Exception e) {
            System.err.println("扫描 USB 设备失败: " + e.getMessage());
        }
        return result;
    }

    // ========================================================================
    // 设备连接 / 断开
    // ========================================================================

    /**
     * 连接设备
     */
    private void connectDevice(int vid, int pid) {
        executor.submit(() -> {
            try {
                device = new CyrioDevice();
                if (pid > 0) {
                    device.openWithVidPid(vid, pid);
                } else {
                    device.open();
                }

                // 获取存储信息
                StorageInfo internal = device.getStorageInfo((byte) 0);
                StorageInfo sdCard = device.getStorageInfo((byte) 1);

                String internalText = internal.isPresent
                        ? String.format("%.1fMB / %.1fMB",
                        internal.usedSize / 1048576.0, internal.totalSize / 1048576.0)
                        : "—";
                String sdText = sdCard.isPresent
                        ? String.format("%.1fMB / %.1fMB",
                        sdCard.usedSize / 1048576.0, sdCard.totalSize / 1048576.0)
                        : "未插入";

                String modelName = internal.model != null && !internal.model.isEmpty()
                        ? internal.model : "Rio S-Series";

                Platform.runLater(() -> {
                    mainWindow.setDeviceConnected(true, modelName);
                    mainWindow.setStorageInfo(internalText, sdText);
                    mainWindow.getDeviceView().setConnected(true, modelName, "—",
                            internalText, sdText);
                    // 自动刷新歌曲列表
                    refreshSongs();
                    refreshPlaylists();
                });

            } catch (Exception e) {
                Platform.runLater(() -> {
                    mainWindow.setDeviceConnected(false, null);
                    System.err.println("连接设备失败: " + e.getMessage());
                });
            }
        });
    }

    /**
     * 断开设备
     */
    private void disconnectDevice() {
        executor.submit(() -> {
            if (device != null) {
                try {
                    device.close();
                } catch (Exception e) {
                    System.err.println("关闭设备失败: " + e.getMessage());
                }
                device = null;
            }
            Platform.runLater(() -> {
                mainWindow.setDeviceConnected(false, null);
                mainWindow.setStorageInfo("—", "—");
                mainWindow.getDeviceView().setConnected(false, null, null,
                        null, null);
            });
        });
    }

    // ========================================================================
    // 歌曲操作
    // ========================================================================

    /**
     * 刷新歌曲列表
     */
    private void refreshSongs() {
        if (device == null || !device.isConnected()) {
            return;
        }
        executor.submit(() -> {
            try {
                // 先加载内置存储，再加载 SD 卡（避免 USB 锁竞争）
                List<Song> allSongs = new ArrayList<>();
                allSongs.addAll(device.listSongs((byte) 0));
                allSongs.addAll(device.listSongs((byte) 1));

                Platform.runLater(() -> {
                    mainWindow.getSongsView().setSongs(allSongs);
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("加载歌曲失败: " + e.getMessage());
                });
            }
        });
    }

    /**
     * 删除歌曲
     */
    private void deleteSongs(List<Song> songs) {
        if (device == null || songs == null || songs.isEmpty()) {
            return;
        }
        executor.submit(() -> {
            int success = 0;
            int failed = 0;
            for (Song song : songs) {
                try {
                    if (device.deleteFile(song.memUnit, song.fileNo)) {
                        success++;
                    } else {
                        failed++;
                    }
                } catch (Exception e) {
                    failed++;
                    System.err.println("删除失败: " + e.getMessage());
                }
            }
            final int ok = success;
            final int fail = failed;
            Platform.runLater(() -> {
                System.out.println("删除完成: 成功 " + ok + " 失败 " + fail);
                refreshSongs();
            });
        });
    }

    // ========================================================================
    // 歌单操作
    // ========================================================================

    /**
     * 刷新歌单列表
     */
    private void refreshPlaylists() {
        if (device == null || !device.isConnected()) {
            return;
        }
        executor.submit(() -> {
            try {
                List<Playlist> allPlaylists = new ArrayList<>();
                allPlaylists.addAll(device.listPlaylists((byte) 0));
                allPlaylists.addAll(device.listPlaylists((byte) 1));

                Platform.runLater(() -> {
                    mainWindow.getPlaylistsView().setPlaylists(allPlaylists);
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("加载歌单失败: " + e.getMessage());
                });
            }
        });
    }

    /**
     * 创建歌单
     */
    private void createPlaylist(String name, byte memUnit) {
        if (device == null) {
            return;
        }
        executor.submit(() -> {
            try {
                CyrioDevice.CreatePlaylistResult result = device.createPlaylist(name, memUnit);
                Platform.runLater(() -> {
                    System.out.println("歌单创建成功: " + name + " (fileNo=" + result.fileNo + ")");
                    refreshPlaylists();
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("创建歌单失败: " + e.getMessage());
                });
            }
        });
    }

    /**
     * 删除歌单
     */
    private void deletePlaylist(Playlist playlist) {
        if (device == null || playlist == null) {
            return;
        }
        executor.submit(() -> {
            try {
                device.deleteFile(playlist.memUnit, playlist.fileNo);
                Platform.runLater(() -> {
                    System.out.println("歌单已删除: " + playlist.name);
                    refreshPlaylists();
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("删除歌单失败: " + e.getMessage());
                });
            }
        });
    }

    /**
     * 加载歌单内歌曲
     */
    private void loadPlaylistSongs(Playlist playlist) {
        if (device == null || playlist == null) {
            return;
        }
        executor.submit(() -> {
            try {
                List<CyrioDevice.PlaylistSong> songs =
                        device.listPlaylistSongs(playlist.fileNo, playlist.memUnit);
                List<Song> songList = new ArrayList<>();
                for (CyrioDevice.PlaylistSong ps : songs) {
                    songList.add(ps.song);
                }
                Platform.runLater(() -> {
                    mainWindow.getPlaylistsView().setPlaylistSongs(songList);
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("加载歌单歌曲失败: " + e.getMessage());
                });
            }
        });
    }

    /**
     * 从歌单中移除歌曲
     *
     * <p>通过 JNI 调用 Rust 的 remove_from_playlist：
     * 下载歌单 FIDL → 移除指定索引条目 → 覆盖回设备。
     */
    private void removeFromPlaylist(Playlist playlist, Song song) {
        if (device == null || playlist == null || song == null) {
            return;
        }

        // 获取歌曲在歌单中的索引
        int index = mainWindow.getPlaylistsView().getSongIndex(song);
        if (index < 0) {
            Platform.runLater(() -> {
                System.err.println("无法找到歌曲在歌单中的位置");
            });
            return;
        }

        executor.submit(() -> {
            try {
                boolean ok = device.removeFromPlaylist(
                        playlist.fileNo, playlist.memUnit, index);
                Platform.runLater(() -> {
                    if (ok) {
                        System.out.println("已从歌单移除: " + song.title);
                        loadPlaylistSongs(playlist);
                    } else {
                        System.err.println("从歌单移除失败");
                    }
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("从歌单移除失败: " + e.getMessage());
                });
            }
        });
    }

    // ========================================================================
    // 上传
    // ========================================================================

    /**
     * 上传文件
     */
    private void uploadFiles(List<File> files, byte memUnit, boolean slug, boolean strip) {
        if (device == null || files == null || files.isEmpty()) {
            return;
        }
        this.applySlug = slug;
        this.applyStrip = strip;

        executor.submit(() -> {
            int success = 0;
            int failed = 0;
            for (File file : files) {
                try {
                    int fileNo = device.uploadFile(memUnit, file.getAbsolutePath(), slug, strip);
                    if (fileNo > 0) {
                        success++;
                    } else {
                        failed++;
                    }
                } catch (Exception e) {
                    failed++;
                    System.err.println("上传失败: " + file.getName() + " - " + e.getMessage());
                }
            }
            final int ok = success;
            final int fail = failed;
            Platform.runLater(() -> {
                System.out.println("上传完成: 成功 " + ok + " 失败 " + fail);
                refreshSongs();
            });
        });
    }

    // ========================================================================
    // 歌曲播放（下载到临时文件 → JavaFX MediaPlayer）
    // ========================================================================

    /** JavaFX MediaPlayer 实例（单曲模式） */
    private javafx.scene.media.MediaPlayer mediaPlayer;

    /**
     * 播放歌曲：从设备下载到临时文件，用 JavaFX MediaPlayer 播放
     */
    private void playSong(Song song) {
        if (device == null || song == null) {
            return;
        }

        // 停止当前播放
        if (mediaPlayer != null) {
            mediaPlayer.stop();
            mediaPlayer.dispose();
            mediaPlayer = null;
        }

        executor.submit(() -> {
            try {
                // 下载到临时文件
                String title = (song.title != null && !song.title.isEmpty())
                        ? song.title : song.name;
                File tmp = File.createTempFile("cyrio_play_", ".mp3");
                tmp.deleteOnExit();

                boolean ok = device.downloadFile(song.memUnit, song.fileNo, tmp.getAbsolutePath());
                if (!ok) {
                    Platform.runLater(() -> {
                        System.err.println("下载播放文件失败: " + title);
                    });
                    return;
                }

                Platform.runLater(() -> {
                    try {
                        javafx.scene.media.Media media = new javafx.scene.media.Media(
                                tmp.toURI().toString());
                        mediaPlayer = new javafx.scene.media.MediaPlayer(media);
                        mediaPlayer.setOnError(() -> {
                            System.err.println("播放错误: " + mediaPlayer.getError());
                        });
                        mediaPlayer.play();
                        System.out.println("正在播放: " + title);
                    } catch (Exception e) {
                        System.err.println("初始化播放器失败: " + e.getMessage());
                    }
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("播放失败: " + e.getMessage());
                });
            }
        });
    }

    // ========================================================================
    // 下载歌曲（弹出文件保存对话框）
    // ========================================================================

    /**
     * 弹出文件保存对话框，下载歌曲到本地
     */
    private void showDownloadDialog(Song song) {
        if (device == null || song == null) {
            return;
        }

        String title = (song.title != null && !song.title.isEmpty())
                ? song.title : song.name;

        FileChooser fileChooser = new FileChooser();
        fileChooser.setTitle("保存歌曲");
        fileChooser.setInitialFileName(title + ".mp3");
        fileChooser.getExtensionFilters().add(
                new FileChooser.ExtensionFilter("MP3 文件", "*.mp3"));

        File target = fileChooser.showSaveDialog(null);
        if (target == null) {
            return;
        }

        executor.submit(() -> {
            try {
                boolean ok = device.downloadFile(song.memUnit, song.fileNo, target.getAbsolutePath());
                Platform.runLater(() -> {
                    if (ok) {
                        System.out.println("下载完成: " + target.getName());
                    } else {
                        System.err.println("下载失败: " + title);
                    }
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("下载失败: " + e.getMessage());
                });
            }
        });
    }

    // ========================================================================
    // 重命名歌曲（弹出文本输入对话框）
    // ========================================================================

    /**
     * 弹出重命名对话框，修改歌曲 title
     */
    private void showRenameDialog(Song song) {
        if (device == null || song == null) {
            return;
        }

        String currentTitle = (song.title != null && !song.title.isEmpty())
                ? song.title : song.name;

        TextInputDialog dialog = new TextInputDialog(currentTitle);
        dialog.setTitle("重命名歌曲");
        dialog.setHeaderText("修改歌曲标题");
        dialog.setContentText("新标题:");

        Optional<String> result = dialog.showAndWait();
        if (result.isEmpty()) {
            return;
        }

        String newName = result.get().trim();
        if (newName.isEmpty() || newName.equals(currentTitle)) {
            return;
        }

        executor.submit(() -> {
            try {
                boolean ok = device.renameSong(song.fileNo, song.memUnit, newName);
                Platform.runLater(() -> {
                    if (ok) {
                        System.out.println("重命名成功: " + newName);
                        refreshSongs();
                    } else {
                        System.err.println("重命名失败");
                    }
                });
            } catch (Exception e) {
                Platform.runLater(() -> {
                    System.err.println("重命名失败: " + e.getMessage());
                });
            }
        });
    }

    // ========================================================================
    // 加入歌单（弹出歌单选择对话框）
    // ========================================================================

    /**
     * 弹出歌单选择对话框，将歌曲加入选中的歌单
     */
    private void showAddToPlaylistDialog(List<Song> songs) {
        if (device == null || songs == null || songs.isEmpty()) {
            return;
        }

        // 获取所有歌单
        List<Playlist> allPlaylists = new ArrayList<>();
        try {
            allPlaylists.addAll(device.listPlaylists((byte) 0));
            allPlaylists.addAll(device.listPlaylists((byte) 1));
        } catch (Exception e) {
            System.err.println("加载歌单列表失败: " + e.getMessage());
            return;
        }

        if (allPlaylists.isEmpty()) {
            System.err.println("设备上没有歌单，请先创建歌单");
            return;
        }

        // 弹出选择对话框
        ChoiceDialog<Playlist> dialog = new ChoiceDialog<>(allPlaylists.get(0), allPlaylists);
        dialog.setTitle("加入歌单");
        dialog.setHeaderText("选择目标歌单（" + songs.size() + " 首歌曲）");
        dialog.setContentText("歌单:");

        Optional<Playlist> result = dialog.showAndWait();
        if (result.isEmpty()) {
            return;
        }

        Playlist targetPlaylist = result.get();
        executor.submit(() -> {
            int success = 0;
            int failed = 0;
            for (Song song : songs) {
                try {
                    boolean ok = device.addToPlaylist(
                            song.fileNo, song.memUnit,
                            targetPlaylist.fileNo, targetPlaylist.memUnit);
                    if (ok) {
                        success++;
                    } else {
                        failed++;
                    }
                } catch (Exception e) {
                    failed++;
                    System.err.println("加入歌单失败: " + song.title + " - " + e.getMessage());
                }
            }
            final int ok = success;
            final int fail = failed;
            Platform.runLater(() -> {
                System.out.println("加入歌单完成: 成功 " + ok + " 失败 " + fail
                        + " → " + targetPlaylist.name);
            });
        });
    }

    // ========================================================================
    // 清理
    // ========================================================================

    /**
     * 关闭控制器，释放资源
     */
    public void shutdown() {
        executor.submit(() -> {
            if (device != null) {
                try {
                    device.close();
                } catch (Exception ignored) {
                }
                device = null;
            }
        });
        executor.shutdown();
    }
}
