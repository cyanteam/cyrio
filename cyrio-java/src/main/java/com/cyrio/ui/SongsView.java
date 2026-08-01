package com.cyrio.ui;

import javafx.beans.property.SimpleBooleanProperty;
import javafx.beans.property.SimpleIntegerProperty;
import javafx.beans.property.SimpleLongProperty;
import javafx.beans.property.SimpleObjectProperty;
import javafx.beans.property.SimpleStringProperty;
import javafx.collections.FXCollections;
import javafx.collections.ListChangeListener;
import javafx.collections.ObservableList;
import javafx.collections.transformation.FilteredList;
import javafx.collections.transformation.SortedList;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.CheckBox;
import javafx.scene.control.ContextMenu;
import javafx.scene.control.Label;
import javafx.scene.control.MenuItem;
import javafx.scene.control.SelectionMode;
import javafx.scene.control.SeparatorMenuItem;
import javafx.scene.control.TableCell;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableRow;
import javafx.scene.control.TableView;
import javafx.scene.control.TextField;
import javafx.scene.control.ToggleButton;
import javafx.scene.control.ToggleGroup;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;
import javafx.util.Callback;

import com.cyrio.core.model.Song;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * 歌曲视图
 *
 * <p>显示设备上的所有歌曲，支持批量操作和筛选。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>批量工具栏：全选 / 清空 / 删除(N) / 加入歌单(N) / 更多 / 刷新</li>
 *   <li>筛选栏：排序（名称/大小/时间） + 搜索框 + 数量显示</li>
 *   <li>歌曲表格：7 列结构（标题/艺术家/专辑/时长/大小/比特率/存储）</li>
 * </ul>
 *
 * <h3>表格交互</h3>
 * <ul>
 *   <li>单击选中行</li>
 *   <li>双击播放歌曲</li>
 *   <li>右键上下文菜单（播放/下载/删除/加入歌单/重命名）</li>
 *   <li>Shift+点击范围选择</li>
 * </ul>
 *
 * <p>对应 Rust 前端 {@code SongsPane} 组件。
 */
public class SongsView extends VBox {

    // ========================================================================
    // 排序枚举
    // ========================================================================

    /** 排序方式 */
    public enum SortKey {
        NAME("名称"),
        SIZE("大小"),
        TIME("时间");

        private final String label;

        SortKey(String label) {
            this.label = label;
        }

        public String getLabel() {
            return label;
        }
    }

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 歌曲操作回调 */
    public interface SongActionCallback {
        void onPlaySong(Song song);
        void onDeleteSongs(List<Song> songs);
        void onAddToPlaylist(List<Song> songs);
        void onDownloadSong(Song song);
        void onRenameSong(Song song);
        void onRefresh();
    }

    // ========================================================================
    // SongRow 包装类（用于 JavaFX TableView）
    // ========================================================================

    /**
     * 歌曲行数据模型
     *
     * <p>包装 {@link Song}，为 JavaFX TableView 提供属性绑定支持。
     * 每行带有一个 {@code checked} 属性用于批量勾选。
     */
    public static class SongRow {
        private final SimpleObjectProperty<Song> song;
        private final SimpleStringProperty title;
        private final SimpleStringProperty artist;
        private final SimpleStringProperty album;
        private final SimpleStringProperty duration;
        private final SimpleStringProperty size;
        private final SimpleStringProperty bitrate;
        private final SimpleStringProperty storage;
        private final SimpleBooleanProperty checked;
        private final SimpleIntegerProperty memUnit;

        public SongRow(Song song) {
            this.song = new SimpleObjectProperty<>(song);
            this.title = new SimpleStringProperty(
                    (song.title != null && !song.title.isEmpty()) ? song.title
                            : (song.name != null && !song.name.isEmpty()) ? stripExtension(song.name)
                            : "(无标题)");
            this.artist = new SimpleStringProperty(
                    (song.artist != null && !song.artist.isEmpty()) ? song.artist : "—");
            this.album = new SimpleStringProperty(
                    (song.album != null && !song.album.isEmpty()) ? song.album : "—");
            this.duration = new SimpleStringProperty(
                    song.time > 0 ? formatDuration(song.time) : "—");
            this.size = new SimpleStringProperty(formatSize(song.size));
            this.bitrate = new SimpleStringProperty(
                    song.bitRate > 0 ? song.bitRate + "kbps" : "—");
            this.storage = new SimpleStringProperty(
                    song.memUnit == 0 ? "内存" : "SD");
            this.memUnit = new SimpleIntegerProperty(song.memUnit);
            this.checked = new SimpleBooleanProperty(false);
        }

        public Song getSong() { return song.get(); }
        public String getTitle() { return title.get(); }
        public String getArtist() { return artist.get(); }
        public String getAlbum() { return album.get(); }
        public String getDuration() { return duration.get(); }
        public String getSize() { return size.get(); }
        public String getBitrate() { return bitrate.get(); }
        public String getStorage() { return storage.get(); }
        public int getMemUnit() { return memUnit.get(); }
        public boolean isChecked() { return checked.get(); }
        public void setChecked(boolean value) { checked.set(value); }
        public SimpleBooleanProperty checkedProperty() { return checked; }
    }

    // ========================================================================
    // 字段
    // ========================================================================

    /** 原始歌曲数据列表 */
    private final ObservableList<SongRow> allSongs = FXCollections.observableArrayList();

    /** 过滤后的歌曲列表 */
    private final FilteredList<SongRow> filteredSongs = new FilteredList<>(allSongs, p -> true);

    /** 排序后的歌曲列表 */
    private final SortedList<SongRow> sortedSongs = new SortedList<>(filteredSongs);

    /** TableView */
    private final TableView<SongRow> tableView = new TableView<>();

    /** 搜索框 */
    private final TextField searchField = new TextField();

    /** 当前排序方式 */
    private SortKey currentSort = SortKey.NAME;

    /** 数量显示标签 */
    private final Label countLabel = new Label("0 首");

    /** 删除按钮 */
    private final Button deleteBtn = new Button("删除");

    /** 加入歌单按钮 */
    private final Button addToPlaylistBtn = new Button("加入歌单");

    /** 全选按钮 */
    private final Button selectAllBtn = new Button("全选");

    /** 清空选择按钮 */
    private final Button clearBtn = new Button("清空");

    /** 回调 */
    private SongActionCallback callback;

    /** 上一次点击的行索引（用于 Shift 范围选择） */
    private int lastClickedIndex = -1;

    // ========================================================================
    // 构造
    // ========================================================================

    public SongsView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(8);

        // 批量工具栏
        this.getChildren().add(createBatchToolbar());

        // 筛选栏
        this.getChildren().add(createFilterBar());

        // 歌曲表格
        this.getChildren().add(createSongTable());

        // 监听勾选状态变化，更新按钮文本
        allSongs.addListener((ListChangeListener<SongRow>) c -> updateCount());
    }

    // ========================================================================
    // 批量工具栏
    // ========================================================================

    /**
     * 创建批量操作工具栏
     *
     * <p>包含：全选 / 清空 / 删除(N) / 加入歌单(N) / 更多 / 刷新
     */
    private HBox createBatchToolbar() {
        HBox toolbar = new HBox();
        toolbar.getStyleClass().add("batch-toolbar");

        // 全选
        selectAllBtn.getStyleClass().add("batch-btn");
        selectAllBtn.setOnAction(e -> selectAll());

        // 清空
        clearBtn.getStyleClass().add("batch-btn");
        clearBtn.setOnAction(e -> clearSelection());

        // 删除
        deleteBtn.getStyleClass().addAll("batch-btn", "batch-btn-danger");
        deleteBtn.setOnAction(e -> deleteSelected());

        // 加入歌单
        addToPlaylistBtn.getStyleClass().add("batch-btn");
        addToPlaylistBtn.setOnAction(e -> addToPlaylistSelected());

        // 更多按钮
        Button moreBtn = new Button("更多");
        moreBtn.getStyleClass().add("batch-btn");
        // 更多菜单（批量 slug/strip/修复编码等）
        ContextMenu moreMenu = new ContextMenu();
        MenuItem slugItem = new MenuItem("批量 Slug 转换");
        slugItem.setOnAction(e -> { /* 回调 */ });
        MenuItem stripItem = new MenuItem("批量去噪");
        stripItem.setOnAction(e -> { /* 回调 */ });
        MenuItem repairItem = new MenuItem("修复编码");
        repairItem.setOnAction(e -> { /* 回调 */ });
        moreMenu.getItems().addAll(slugItem, stripItem, new SeparatorMenuItem(), repairItem);
        moreBtn.setOnMouseClicked(e -> {
            moreMenu.show(moreBtn, e.getScreenX(), e.getScreenY());
        });

        // 刷新
        Button refreshBtn = new Button("\u21BB 刷新");
        refreshBtn.getStyleClass().add("batch-btn");
        refreshBtn.setOnAction(e -> {
            if (callback != null) callback.onRefresh();
        });

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        toolbar.getChildren().addAll(
                selectAllBtn, clearBtn, deleteBtn, addToPlaylistBtn, moreBtn,
                spacer, refreshBtn);

        return toolbar;
    }

    // ========================================================================
    // 筛选栏
    // ========================================================================

    /**
     * 创建筛选栏
     *
     * <p>包含：排序按钮组（名称/大小/时间） + 搜索框 + 数量显示
     */
    private HBox createFilterBar() {
        HBox filterBar = new HBox();
        filterBar.getStyleClass().add("filter-bar");
        filterBar.setAlignment(Pos.CENTER_LEFT);

        // 排序标签
        Label sortLabel = new Label("排序:");
        sortLabel.getStyleClass().add("filter-label");

        // 排序分段控件
        HBox segControl = new HBox();
        segControl.getStyleClass().add("seg-control");

        ToggleGroup sortGroup = new ToggleGroup();
        for (SortKey key : SortKey.values()) {
            ToggleButton sortBtn = new ToggleButton(key.getLabel());
            sortBtn.getStyleClass().add("seg-btn");
            sortBtn.setToggleGroup(sortGroup);
            if (key == currentSort) {
                sortBtn.setSelected(true);
                sortBtn.getStyleClass().add("seg-btn-active");
            }
            sortBtn.selectedProperty().addListener((obs, oldVal, newVal) -> {
                if (newVal) {
                    currentSort = key;
                    // 更新分段控件样式
                    for (var tb : sortGroup.getToggles()) {
                        var btn = (ToggleButton) tb;
                        btn.getStyleClass().remove("seg-btn-active");
                        if (btn.isSelected()) {
                            btn.getStyleClass().add("seg-btn-active");
                        }
                    }
                    applySort();
                }
            });
            segControl.getChildren().add(sortBtn);
        }

        // 搜索框
        searchField.getStyleClass().add("search-field");
        searchField.setPromptText("搜索歌曲...");
        searchField.textProperty().addListener((obs, oldVal, newVal) -> applyFilter());

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        // 数量显示
        countLabel.getStyleClass().add("filter-label");

        filterBar.getChildren().addAll(sortLabel, segControl, searchField, spacer, countLabel);

        return filterBar;
    }

    // ========================================================================
    // 歌曲表格
    // ========================================================================

    /**
     * 创建歌曲表格
     *
     * <p>7 列结构：标题 / 艺术家 / 专辑 / 时长 / 大小 / 比特率 / 存储
     */
    private TableView<SongRow> createSongTable() {
        tableView.getStyleClass().add("song-table");
        tableView.setItems(sortedSongs);
        sortedSongs.comparatorProperty().bind(tableView.comparatorProperty());
        tableView.getSelectionModel().setSelectionMode(SelectionMode.MULTIPLE);
        tableView.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        VBox.setVgrow(tableView, Priority.ALWAYS);

        // --- 列 1: 标题 (minWidth=180) ---
        TableColumn<SongRow, String> titleCol = new TableColumn<>("标题");
        titleCol.setCellValueFactory(cell -> cell.getValue().title);
        titleCol.getStyleClass().add("col-title");
        titleCol.setMinWidth(180);
        titleCol.setPrefWidth(200);
        titleCol.setSortable(true);

        // --- 列 2: 艺术家 (minWidth=100, maxWidth=220) ---
        TableColumn<SongRow, String> artistCol = new TableColumn<>("艺术家");
        artistCol.setCellValueFactory(cell -> cell.getValue().artist);
        artistCol.getStyleClass().add("col-artist");
        artistCol.setMinWidth(100);
        artistCol.setMaxWidth(220);
        artistCol.setPrefWidth(140);

        // --- 列 3: 专辑 ---
        TableColumn<SongRow, String> albumCol = new TableColumn<>("专辑");
        albumCol.setCellValueFactory(cell -> cell.getValue().album);
        albumCol.getStyleClass().add("col-album");
        albumCol.setMinWidth(100);
        albumCol.setPrefWidth(140);

        // --- 列 4: 时长 (minWidth=64, prefWidth=64) ---
        TableColumn<SongRow, String> durationCol = new TableColumn<>("时长");
        durationCol.setCellValueFactory(cell -> cell.getValue().duration);
        durationCol.getStyleClass().add("col-time");
        durationCol.setMinWidth(64);
        durationCol.setPrefWidth(64);
        durationCol.setMaxWidth(64);
        durationCol.setSortable(false);

        // --- 列 5: 大小 (prefWidth=78) ---
        TableColumn<SongRow, String> sizeCol = new TableColumn<>("大小");
        sizeCol.setCellValueFactory(cell -> cell.getValue().size);
        sizeCol.getStyleClass().add("col-size");
        sizeCol.setPrefWidth(78);
        sizeCol.setMaxWidth(78);
        sizeCol.setSortable(false);

        // --- 列 6: 比特率 (prefWidth=78) ---
        TableColumn<SongRow, String> bitrateCol = new TableColumn<>("比特率");
        bitrateCol.setCellValueFactory(cell -> cell.getValue().bitrate);
        bitrateCol.getStyleClass().add("col-bitrate");
        bitrateCol.setPrefWidth(78);
        bitrateCol.setMaxWidth(78);
        bitrateCol.setSortable(false);

        // --- 列 7: 存储 (prefWidth=78, 显示 badge) ---
        TableColumn<SongRow, String> memCol = new TableColumn<>("存储");
        memCol.setCellValueFactory(cell -> cell.getValue().storage);
        memCol.getStyleClass().add("col-mem");
        memCol.setPrefWidth(78);
        memCol.setMaxWidth(78);
        memCol.setSortable(false);
        memCol.setCellFactory(col -> new TableCell<>() {
            @Override
            protected void updateItem(String value, boolean empty) {
                super.updateItem(value, empty);
                if (empty || value == null) {
                    setGraphic(null);
                    setText(null);
                } else {
                    Label badge = new Label(value);
                    int memUnit = getTableRow() != null && getTableRow().getItem() != null
                            ? ((SongRow) getTableRow().getItem()).getMemUnit() : 0;
                    badge.getStyleClass().add(memUnit == 0
                            ? "mem-badge-internal" : "mem-badge-sd");
                    setGraphic(badge);
                    setText(null);
                }
            }
        });

        tableView.getColumns().addAll(titleCol, artistCol, albumCol,
                durationCol, sizeCol, bitrateCol, memCol);

        // --- 行样式与交互 ---
        tableView.setRowFactory(tv -> {
            TableRow<SongRow> row = new TableRow<>() {
                @Override
                protected void updateItem(SongRow item, boolean empty) {
                    super.updateItem(item, empty);
                    getStyleClass().remove("checked");
                    if (item != null && item.isChecked()) {
                        getStyleClass().add("checked");
                    }
                }
            };

            // 监听行数据的勾选状态
            row.itemProperty().addListener((obs, oldItem, newItem) -> {
                if (oldItem != null) {
                    oldItem.checkedProperty().removeListener(
                            (o, ov, nv) -> row.getStyleClass().setAll(
                                    row.getStyleClass().toArray(new String[0])));
                }
                if (newItem != null) {
                    newItem.checkedProperty().addListener((o, ov, nv) -> {
                        row.getStyleClass().remove("checked");
                        if (nv) {
                            row.getStyleClass().add("checked");
                        }
                    });
                    if (newItem.isChecked()) {
                        row.getStyleClass().add("checked");
                    }
                }
            });

            // 单击选中 + 勾选切换 / 双击播放
            row.setOnMouseClicked(e -> {
                if (row.isEmpty()) return;

                if (e.getClickCount() == 2) {
                    // 双击播放
                    if (callback != null) {
                        callback.onPlaySong(row.getItem().getSong());
                    }
                } else if (e.getClickCount() == 1) {
                    int index = row.getIndex();
                    SongRow item = row.getItem();

                    if (e.isShiftDown() && lastClickedIndex >= 0) {
                        // Shift+点击：范围选择
                        int start = Math.min(lastClickedIndex, index);
                        int end = Math.max(lastClickedIndex, index);
                        List<SongRow> items = tableView.getItems();
                        for (int i = start; i <= end && i < items.size(); i++) {
                            items.get(i).setChecked(true);
                        }
                    } else {
                        // 单击：切换勾选状态
                        item.setChecked(!item.isChecked());
                        tableView.getSelectionModel().select(item);
                    }
                    lastClickedIndex = index;
                    updateBatchButtons();
                }
            });

            // 右键上下文菜单
            row.setOnContextMenuRequested(e -> {
                if (!row.isEmpty()) {
                    showContextMenu(row.getItem(), e.getScreenX(), e.getScreenY());
                }
            });

            return row;
        });

        return tableView;
    }

    // ========================================================================
    // 右键上下文菜单
    // ========================================================================

    /**
     * 显示右键上下文菜单
     *
     * @param row 歌曲行
     * @param screenX 屏幕 X 坐标
     * @param screenY 屏幕 Y 坐标
     */
    private void showContextMenu(SongRow row, double screenX, double screenY) {
        ContextMenu menu = new ContextMenu();
        menu.getStyleClass().add("context-menu");

        MenuItem playItem = new MenuItem("播放试听");
        playItem.setOnAction(e -> {
            if (callback != null) callback.onPlaySong(row.getSong());
        });

        MenuItem downloadItem = new MenuItem("下载到本地");
        downloadItem.setOnAction(e -> {
            if (callback != null) callback.onDownloadSong(row.getSong());
        });

        MenuItem addToPlaylistItem = new MenuItem("加入歌单");
        addToPlaylistItem.setOnAction(e -> {
            if (callback != null) {
                List<Song> songs = new ArrayList<>();
                songs.add(row.getSong());
                callback.onAddToPlaylist(songs);
            }
        });

        MenuItem renameItem = new MenuItem("重命名");
        renameItem.setOnAction(e -> {
            if (callback != null) callback.onRenameSong(row.getSong());
        });

        MenuItem deleteItem = new MenuItem("删除");
        deleteItem.setOnAction(e -> {
            if (callback != null) {
                List<Song> songs = new ArrayList<>();
                songs.add(row.getSong());
                callback.onDeleteSongs(songs);
            }
        });

        menu.getItems().addAll(playItem, downloadItem, new SeparatorMenuItem(),
                addToPlaylistItem, renameItem, new SeparatorMenuItem(), deleteItem);

        menu.show(tableView, screenX, screenY);
    }

    // ========================================================================
    // 批量操作
    // ========================================================================

    /** 全选 */
    private void selectAll() {
        for (SongRow row : filteredSongs) {
            row.setChecked(true);
        }
        updateBatchButtons();
    }

    /** 清空选择 */
    private void clearSelection() {
        for (SongRow row : allSongs) {
            row.setChecked(false);
        }
        updateBatchButtons();
    }

    /** 删除选中歌曲 */
    private void deleteSelected() {
        if (callback == null) return;
        List<Song> selected = getCheckedSongs();
        if (selected.isEmpty()) return;
        callback.onDeleteSongs(selected);
    }

    /** 加入歌单（选中歌曲） */
    private void addToPlaylistSelected() {
        if (callback == null) return;
        List<Song> selected = getCheckedSongs();
        if (selected.isEmpty()) return;
        callback.onAddToPlaylist(selected);
    }

    /**
     * 获取所有勾选的歌曲
     */
    private List<Song> getCheckedSongs() {
        List<Song> result = new ArrayList<>();
        for (SongRow row : allSongs) {
            if (row.isChecked()) {
                result.add(row.getSong());
            }
        }
        return result;
    }

    /**
     * 更新批量操作按钮状态和文本
     */
    private void updateBatchButtons() {
        int count = getCheckedCount();
        deleteBtn.setText(count > 0 ? "删除(" + count + ")" : "删除");
        addToPlaylistBtn.setText(count > 0 ? "加入歌单(" + count + ")" : "加入歌单");
        deleteBtn.setDisable(count == 0);
        addToPlaylistBtn.setDisable(count == 0);
        updateCount();
    }

    /**
     * 获取勾选数量
     */
    private int getCheckedCount() {
        int count = 0;
        for (SongRow row : allSongs) {
            if (row.isChecked()) count++;
        }
        return count;
    }

    /**
     * 更新数量显示
     */
    private void updateCount() {
        int total = allSongs.size();
        int shown = filteredSongs.size();
        if (total == shown) {
            countLabel.setText(total + " 首");
        } else {
            countLabel.setText(shown + " / " + total + " 首");
        }
    }

    // ========================================================================
    // 筛选与排序
    // ========================================================================

    /**
     * 应用搜索筛选
     */
    private void applyFilter() {
        String keyword = searchField.getText().trim().toLowerCase();
        if (keyword.isEmpty()) {
            filteredSongs.setPredicate(p -> true);
        } else {
            filteredSongs.setPredicate(row -> {
                Song s = row.getSong();
                String searchText = (s.title + " " + s.name + " " + s.artist + " " + s.album)
                        .toLowerCase();
                return searchText.contains(keyword);
            });
        }
        updateCount();
    }

    /**
     * 应用排序
     */
    private void applySort() {
        Comparator<SongRow> comparator = switch (currentSort) {
            case NAME -> Comparator.comparing(SongRow::getTitle,
                    Comparator.nullsLast(String::compareToIgnoreCase));
            case SIZE -> Comparator.comparingLong(row -> row.getSong().size);
            case TIME -> Comparator.comparingInt(row -> row.getSong().time);
        };
        tableView.getSortOrder().clear();
        // 使用 SortedList 的 comparator
        sortedSongs.setComparator(comparator);
    }

    // ========================================================================
    // 公共方法
    // ========================================================================

    /**
     * 设置歌曲列表数据
     *
     * @param songs 歌曲列表
     */
    public void setSongs(List<Song> songs) {
        allSongs.clear();
        for (Song song : songs) {
            SongRow row = new SongRow(song);
            allSongs.add(row);
        }
        applySort();
        updateCount();
    }

    /**
     * 清空歌曲列表
     */
    public void clearSongs() {
        allSongs.clear();
        updateCount();
    }

    /**
     * 获取当前显示的所有歌曲
     */
    public List<Song> getAllSongs() {
        List<Song> result = new ArrayList<>();
        for (SongRow row : allSongs) {
            result.add(row.getSong());
        }
        return result;
    }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setCallback(SongActionCallback callback) {
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
     * 格式化文件大小（字节 → KB/MB）
     */
    private static String formatSize(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return String.format("%.1f KB", bytes / 1024.0);
        return String.format("%.1f MB", bytes / (1024.0 * 1024.0));
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
