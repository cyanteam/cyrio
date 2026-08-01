package com.cyrio.ui;

import javafx.beans.property.SimpleStringProperty;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.ContextMenu;
import javafx.scene.control.Label;
import javafx.scene.control.ListCell;
import javafx.scene.control.ListView;
import javafx.scene.control.MenuItem;
import javafx.scene.control.SeparatorMenuItem;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableView;
import javafx.scene.control.TextField;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;

import com.cyrio.core.model.Playlist;
import com.cyrio.core.model.Song;

import java.util.ArrayList;
import java.util.List;

/**
 * 歌单视图
 *
 * <p>左右分栏布局：左侧为歌单列表，右侧为选中歌单内的歌曲列表。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>顶部工具栏：创建歌单 / 刷新 / 删除歌单</li>
 *   <li>左侧（200px）：歌单列表（ListView），显示歌单名称和歌曲数</li>
 *   <li>右侧（自适应）：歌曲表格，显示歌单内的歌曲</li>
 * </ul>
 *
 * <h3>功能</h3>
 * <ul>
 *   <li>创建新歌单：输入名称后在指定内存单元创建空歌单</li>
 *   <li>查看歌单内容：点击左侧歌单项，右侧显示歌曲列表</li>
 *   <li>从歌单移除歌曲：右键歌曲选择"从歌单移除"</li>
 *   <li>播放歌单内歌曲：双击歌曲行</li>
 * </ul>
 *
 * <p>所有设备操作通过回调接口暴露，不直接调用设备 API（解耦设计）。
 */
public class PlaylistsView extends VBox {

    // ========================================================================
    // 歌单行数据模型
    // ========================================================================

    /**
     * 歌单行包装类
     *
     * <p>为 ListView 提供显示数据，包含歌单名称和歌曲数。
     */
    public static class PlaylistRow {
        private final Playlist playlist;

        public PlaylistRow(Playlist playlist) {
            this.playlist = playlist;
        }

        public Playlist getPlaylist() {
            return playlist;
        }

        public String getDisplayName() {
            String name = (playlist.title != null && !playlist.title.isEmpty())
                    ? playlist.title
                    : (playlist.name != null && !playlist.name.isEmpty())
                    ? playlist.name : "(未命名歌单)";
            return name;
        }

        public String getStorageBadge() {
            return playlist.memUnit == 0 ? "内存" : "SD";
        }
    }

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 歌单操作回调 */
    public interface PlaylistActionCallback {
        /** 请求加载歌单列表 */
        void onLoadPlaylists();

        /** 创建新歌单 */
        void onCreatePlaylist(String name, byte memUnit);

        /** 删除歌单 */
        void onDeletePlaylist(Playlist playlist);

        /** 选中歌单变更，请求加载歌单内歌曲 */
        void onLoadPlaylistSongs(Playlist playlist);

        /** 播放歌单内歌曲 */
        void onPlaySong(Song song);

        /** 从歌单中移除歌曲 */
        void onRemoveFromPlaylist(Playlist playlist, Song song);
    }

    // ========================================================================
    // 字段
    // ========================================================================

    /** 歌单列表数据 */
    private final ObservableList<PlaylistRow> playlistList = FXCollections.observableArrayList();

    /** 歌单 ListView */
    private final ListView<PlaylistRow> playlistListView = new ListView<>(playlistList);

    /** 歌单内歌曲表格 */
    private final TableView<Song> songsTable = new TableView<>();

    /** 歌单内歌曲数据 */
    private final ObservableList<Song> playlistSongs = FXCollections.observableArrayList();

    /** 当前选中的歌单 */
    private PlaylistRow selectedPlaylist;

    /** 歌单名称输入框（创建新歌单用） */
    private final TextField newPlaylistNameField = new TextField();

    /** 回调 */
    private PlaylistActionCallback callback;

    /** 歌单数量标签 */
    private final Label countLabel = new Label("0 个歌单");

    /** 歌曲数量标签 */
    private final Label songCountLabel = new Label("0 首");

    // ========================================================================
    // 构造
    // ========================================================================

    public PlaylistsView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(8);

        // 顶部工具栏
        this.getChildren().add(createToolbar());

        // 左右分栏
        this.getChildren().add(createSplitLayout());
    }

    // ========================================================================
    // 顶部工具栏
    // ========================================================================

    /**
     * 创建顶部工具栏
     *
     * <p>包含：新歌单名称输入框 + 创建按钮 + 刷新按钮 + 删除按钮
     */
    private HBox createToolbar() {
        HBox toolbar = new HBox();
        toolbar.getStyleClass().add("batch-toolbar");

        // 新歌单名称输入框
        newPlaylistNameField.getStyleClass().add("search-field");
        newPlaylistNameField.setPromptText("输入歌单名称...");
        newPlaylistNameField.setPrefWidth(180);

        // 创建歌单按钮
        Button createBtn = new Button("创建歌单");
        createBtn.getStyleClass().addAll("batch-btn");
        createBtn.setOnAction(e -> {
            String name = newPlaylistNameField.getText().trim();
            if (!name.isEmpty() && callback != null) {
                // 默认在内置存储创建
                callback.onCreatePlaylist(name, (byte) 0);
                newPlaylistNameField.clear();
            }
        });

        // 刷新按钮
        Button refreshBtn = new Button("\u21BB 刷新");
        refreshBtn.getStyleClass().add("batch-btn");
        refreshBtn.setOnAction(e -> {
            if (callback != null) callback.onLoadPlaylists();
        });

        // 删除歌单按钮
        Button deleteBtn = new Button("删除歌单");
        deleteBtn.getStyleClass().addAll("batch-btn", "batch-btn-danger");
        deleteBtn.setOnAction(e -> {
            if (selectedPlaylist != null && callback != null) {
                callback.onDeletePlaylist(selectedPlaylist.getPlaylist());
            }
        });

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        // 歌单数量
        countLabel.getStyleClass().add("filter-label");

        toolbar.getChildren().addAll(
                newPlaylistNameField, createBtn, deleteBtn, refreshBtn,
                spacer, countLabel);

        return toolbar;
    }

    // ========================================================================
    // 左右分栏布局
    // ========================================================================

    /**
     * 创建左右分栏布局
     *
     * <p>左侧 200px 歌单列表，右侧自适应歌曲表格。
     */
    private HBox createSplitLayout() {
        HBox split = new HBox();
        split.getStyleClass().add("playlist-split");
        split.setSpacing(8);
        HBox.setHgrow(split, Priority.ALWAYS);

        // 左侧歌单列表
        VBox sidebar = createPlaylistSidebar();
        HBox.setHgrow(sidebar, Priority.NEVER);

        // 右侧歌曲表格
        VBox content = createSongsContent();
        HBox.setHgrow(content, Priority.ALWAYS);

        split.getChildren().addAll(sidebar, content);
        return split;
    }

    // ========================================================================
    // 左侧歌单列表
    // ========================================================================

    /**
     * 创建左侧歌单列表区域
     */
    private VBox createPlaylistSidebar() {
        VBox sidebar = new VBox();
        sidebar.getStyleClass().add("playlist-sidebar");
        sidebar.setPrefWidth(200);
        sidebar.setMinWidth(180);
        sidebar.setMaxWidth(240);
        VBox.setVgrow(sidebar, Priority.ALWAYS);

        // 歌单列表
        playlistListView.setCellFactory(lv -> new PlaylistListCell());
        playlistListView.getSelectionModel().selectedItemProperty().addListener(
                (obs, oldVal, newVal) -> {
                    selectedPlaylist = newVal;
                    if (newVal != null && callback != null) {
                        callback.onLoadPlaylistSongs(newVal.getPlaylist());
                    }
                });
        VBox.setVgrow(playlistListView, Priority.ALWAYS);

        sidebar.getChildren().add(playlistListView);
        return sidebar;
    }

    /**
     * 歌单列表单元格
     *
     * <p>显示歌单名称和存储 badge。
     */
    private static class PlaylistListCell extends ListCell<PlaylistRow> {
        @Override
        protected void updateItem(PlaylistRow row, boolean empty) {
            super.updateItem(row, empty);
            getStyleClass().removeAll("playlist-item-active");

            if (empty || row == null) {
                setGraphic(null);
                setText(null);
            } else {
                HBox content = new HBox();
                content.setSpacing(6);
                content.setAlignment(Pos.CENTER_LEFT);

                Label name = new Label(row.getDisplayName());
                name.getStyleClass().add("playlist-item");

                Label badge = new Label(row.getStorageBadge());
                badge.getStyleClass().add(row.getPlaylist().memUnit == 0
                        ? "mem-badge-internal" : "mem-badge-sd");

                content.getChildren().addAll(name, badge);
                setGraphic(content);
                setText(null);

                if (isSelected()) {
                    getStyleClass().add("playlist-item-active");
                }
            }
        }
    }

    // ========================================================================
    // 右侧歌曲表格
    // ========================================================================

    /**
     * 创建右侧歌曲列表区域
     */
    private VBox createSongsContent() {
        VBox content = new VBox();
        content.setSpacing(6);
        VBox.setVgrow(content, Priority.ALWAYS);

        // 标题行
        HBox header = new HBox();
        header.getStyleClass().add("filter-bar");
        header.setAlignment(Pos.CENTER_LEFT);

        Label titleLabel = new Label("歌单曲目");
        titleLabel.getStyleClass().add("pane-header-title");

        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        songCountLabel.getStyleClass().add("filter-label");

        header.getChildren().addAll(titleLabel, spacer, songCountLabel);

        // 歌曲表格
        setupSongsTable();
        VBox.setVgrow(songsTable, Priority.ALWAYS);

        content.getChildren().addAll(header, songsTable);
        return content;
    }

    /**
     * 配置歌曲表格列和交互
     */
    private void setupSongsTable() {
        songsTable.getStyleClass().add("song-table");
        songsTable.setItems(playlistSongs);
        songsTable.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);

        // 序号列
        TableColumn<Song, String> indexCol = new TableColumn<>("#");
        indexCol.setCellValueFactory(cell -> {
            int idx = playlistSongs.indexOf(cell.getValue()) + 1;
            return new SimpleStringProperty(String.valueOf(idx));
        });
        indexCol.setPrefWidth(40);
        indexCol.setMaxWidth(40);
        indexCol.setSortable(false);

        // 标题列
        TableColumn<Song, String> titleCol = new TableColumn<>("标题");
        titleCol.setCellValueFactory(cell -> {
            Song s = cell.getValue();
            String title = (s.title != null && !s.title.isEmpty()) ? s.title
                    : (s.name != null && !s.name.isEmpty()) ? stripExtension(s.name)
                    : "(无标题)";
            return new SimpleStringProperty(title);
        });
        titleCol.setMinWidth(180);
        titleCol.setPrefWidth(220);

        // 艺术家列
        TableColumn<Song, String> artistCol = new TableColumn<>("艺术家");
        artistCol.setCellValueFactory(cell -> {
            Song s = cell.getValue();
            return new SimpleStringProperty(
                    (s.artist != null && !s.artist.isEmpty()) ? s.artist : "—");
        });
        artistCol.setMinWidth(100);
        artistCol.setMaxWidth(220);
        artistCol.setPrefWidth(140);

        // 时长列
        TableColumn<Song, String> durationCol = new TableColumn<>("时长");
        durationCol.setCellValueFactory(cell -> {
            Song s = cell.getValue();
            return new SimpleStringProperty(s.time > 0 ? formatDuration(s.time) : "—");
        });
        durationCol.setPrefWidth(64);
        durationCol.setMaxWidth(64);
        durationCol.setSortable(false);

        // 存储列
        TableColumn<Song, String> memCol = new TableColumn<>("存储");
        memCol.setCellValueFactory(cell -> {
            Song s = cell.getValue();
            return new SimpleStringProperty(s.memUnit == 0 ? "内存" : "SD");
        });
        memCol.setPrefWidth(78);
        memCol.setMaxWidth(78);
        memCol.setSortable(false);

        songsTable.getColumns().addAll(indexCol, titleCol, artistCol, durationCol, memCol);

        // 双击播放
        songsTable.setRowFactory(tv -> {
            var row = new javafx.scene.control.TableRow<Song>();
            row.setOnMouseClicked(e -> {
                if (e.getClickCount() == 2 && !row.isEmpty() && callback != null) {
                    callback.onPlaySong(row.getItem());
                }
            });
            // 右键菜单
            row.setOnContextMenuRequested(e -> {
                if (!row.isEmpty()) {
                    showSongContextMenu(row.getItem(), e.getScreenX(), e.getScreenY());
                }
            });
            return row;
        });
    }

    /**
     * 显示歌曲右键上下文菜单
     */
    private void showSongContextMenu(Song song, double screenX, double screenY) {
        ContextMenu menu = new ContextMenu();
        menu.getStyleClass().add("context-menu");

        MenuItem playItem = new MenuItem("播放试听");
        playItem.setOnAction(e -> {
            if (callback != null) callback.onPlaySong(song);
        });

        MenuItem removeItem = new MenuItem("从歌单移除");
        removeItem.setOnAction(e -> {
            if (selectedPlaylist != null && callback != null) {
                callback.onRemoveFromPlaylist(selectedPlaylist.getPlaylist(), song);
            }
        });

        menu.getItems().addAll(playItem, new SeparatorMenuItem(), removeItem);
        menu.show(songsTable, screenX, screenY);
    }

    // ========================================================================
    // 公共方法
    // ========================================================================

    /**
     * 获取歌曲在当前歌单中的索引
     *
     * @param song 歌曲对象
     * @return 索引 (0-based)，未找到返回 -1
     */
    public int getSongIndex(Song song) {
        return playlistSongs.indexOf(song);
    }

    /**
     * 设置歌单列表数据
     *
     * @param playlists 歌单列表
     */
    public void setPlaylists(List<Playlist> playlists) {
        playlistList.clear();
        if (playlists != null) {
            for (Playlist p : playlists) {
                playlistList.add(new PlaylistRow(p));
            }
        }
        countLabel.setText(playlistList.size() + " 个歌单");
    }

    /**
     * 设置歌单内歌曲列表
     *
     * @param songs 歌曲列表
     */
    public void setPlaylistSongs(List<Song> songs) {
        playlistSongs.clear();
        if (songs != null) {
            playlistSongs.addAll(songs);
        }
        songCountLabel.setText(playlistSongs.size() + " 首");
    }

    /**
     * 清空所有数据
     */
    public void clearAll() {
        playlistList.clear();
        playlistSongs.clear();
        selectedPlaylist = null;
        countLabel.setText("0 个歌单");
        songCountLabel.setText("0 首");
    }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setCallback(PlaylistActionCallback callback) {
        this.callback = callback;
    }

    // ========================================================================
    // 工具方法
    // ========================================================================

    /**
     * 格式化时长（秒 → mm:ss 或 h:mm:ss）
     */
    private static String formatDuration(int seconds) {
        if (seconds <= 0) return "—";
        int h = seconds / 3600;
        int m = (seconds % 3600) / 60;
        int s = seconds % 60;
        if (h > 0) {
            return String.format("%d:%02d:%02d", h, m, s);
        }
        return String.format("%d:%02d", m, s);
    }

    /**
     * 去除文件扩展名
     */
    private static String stripExtension(String name) {
        if (name == null) return "";
        int dot = name.lastIndexOf('.');
        return dot > 0 ? name.substring(0, dot) : name;
    }
}
