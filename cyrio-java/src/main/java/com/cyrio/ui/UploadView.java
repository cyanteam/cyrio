package com.cyrio.ui;

import javafx.beans.property.SimpleStringProperty;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.CheckBox;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.RadioButton;
import javafx.scene.control.SelectionMode;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableView;
import javafx.scene.control.ToggleGroup;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;
import javafx.stage.FileChooser;

import java.io.File;
import java.util.ArrayList;
import java.util.List;

/**
 * 上传视图
 *
 * <p>将本地音频文件上传到 Rio 设备的指定存储单元。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>顶部工具栏：添加文件 / 清空列表 / 开始上传</li>
 *   <li>上传选项：应用 slug / 应用 strip / 目标存储（内置/SD 卡）</li>
 *   <li>文件列表表格：文件名 / 大小 / 格式 / 状态</li>
 *   <li>底部进度条：总上传进度</li>
 * </ul>
 *
 * <p>所有设备操作通过回调接口暴露，不直接调用设备 API（解耦设计）。
 */
public class UploadView extends VBox {

    // ========================================================================
    // 上传文件行数据模型
    // ========================================================================

    /**
     * 上传文件行包装类
     */
    public static class UploadFileRow {
        private final File file;
        private final SimpleStringProperty name;
        private final SimpleStringProperty size;
        private final SimpleStringProperty format;
        private final SimpleStringProperty status;

        public UploadFileRow(File file) {
            this.file = file;
            this.name = new SimpleStringProperty(file.getName());
            this.size = new SimpleStringProperty(formatSize(file.length()));
            this.format = new SimpleStringProperty(getFileFormat(file.getName()));
            this.status = new SimpleStringProperty("等待上传");
        }

        public File getFile() { return file; }
        public String getName() { return name.get(); }
        public String getSize() { return size.get(); }
        public String getFormat() { return format.get(); }
        public String getStatus() { return status.get(); }
        public void setStatus(String value) { status.set(value); }
    }

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 上传操作回调 */
    public interface UploadActionCallback {
        /**
         * 上传文件列表
         *
         * @param files    要上传的文件列表
         * @param memUnit  目标内存单元（0=内置, 1=SD 卡）
         * @param applySlug 是否应用 slug 转换
         * @param applyStrip 是否应用噪音去除
         */
        void onUploadFiles(List<File> files, byte memUnit, boolean applySlug, boolean applyStrip);
    }

    // ========================================================================
    // 字段
    // ========================================================================

    /** 文件列表数据 */
    private final ObservableList<UploadFileRow> fileList = FXCollections.observableArrayList();

    /** 文件列表表格 */
    private final TableView<UploadFileRow> fileTable = new TableView<>(fileList);

    /** 应用 slug 选项 */
    private final CheckBox slugCheckBox = new CheckBox("应用 Slug（中文转拼音）");

    /** 应用 strip 选项 */
    private final CheckBox stripCheckBox = new CheckBox("应用去噪（去除噪音词）");

    /** 目标存储选择组 */
    private final ToggleGroup storageGroup = new ToggleGroup();

    /** 上传进度条 */
    private final ProgressBar progressBar = new ProgressBar(0);

    /** 进度文本 */
    private final Label progressLabel = new Label("就绪");

    /** 回调 */
    private UploadActionCallback callback;

    // ========================================================================
    // 构造
    // ========================================================================

    public UploadView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(8);

        // 顶部工具栏
        this.getChildren().add(createToolbar());

        // 上传选项
        this.getChildren().add(createUploadOptions());

        // 文件列表
        this.getChildren().add(createFileTable());

        // 底部进度
        this.getChildren().add(createProgressSection());
    }

    // ========================================================================
    // 顶部工具栏
    // ========================================================================

    /**
     * 创建顶部工具栏
     *
     * <p>包含：添加文件 / 清空列表 / 开始上传 按钮
     */
    private HBox createToolbar() {
        HBox toolbar = new HBox();
        toolbar.getStyleClass().add("batch-toolbar");

        // 添加文件按钮
        Button addBtn = new Button("+ 添加文件");
        addBtn.getStyleClass().add("batch-btn");
        addBtn.setOnAction(e -> addFiles());

        // 清空列表按钮
        Button clearBtn = new Button("清空列表");
        clearBtn.getStyleClass().add("batch-btn");
        clearBtn.setOnAction(e -> fileList.clear());

        // 移除选中按钮
        Button removeBtn = new Button("移除选中");
        removeBtn.getStyleClass().add("batch-btn");
        removeBtn.setOnAction(e -> {
            var selected = new ArrayList<>(fileTable.getSelectionModel().getSelectedItems());
            fileList.removeAll(selected);
        });

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        // 开始上传按钮
        Button uploadBtn = new Button("\u2191 开始上传");
        uploadBtn.getStyleClass().addAll("btn", "btn-primary");
        uploadBtn.setOnAction(e -> startUpload());

        toolbar.getChildren().addAll(addBtn, removeBtn, clearBtn, spacer, uploadBtn);
        return toolbar;
    }

    // ========================================================================
    // 上传选项
    // ========================================================================

    /**
     * 创建上传选项区域
     *
     * <p>包含：应用 slug / 应用 strip / 目标存储（内置/SD 卡）
     */
    private HBox createUploadOptions() {
        HBox options = new HBox();
        options.getStyleClass().add("filter-bar");
        options.setAlignment(Pos.CENTER_LEFT);
        options.setSpacing(16);

        // slug 选项
        slugCheckBox.getStyleClass().add("settings-label");
        slugCheckBox.setSelected(true);

        // strip 选项
        stripCheckBox.getStyleClass().add("settings-label");

        // 分隔线
        Label sep = new Label("|");
        sep.getStyleClass().add("filter-label");

        // 目标存储
        Label storageLabel = new Label("目标存储:");
        storageLabel.getStyleClass().add("filter-label");

        RadioButton internalRadio = new RadioButton("内置存储");
        internalRadio.getStyleClass().add("settings-label");
        internalRadio.setToggleGroup(storageGroup);
        internalRadio.setSelected(true);
        internalRadio.setUserData((byte) 0);

        RadioButton sdRadio = new RadioButton("SD 卡");
        sdRadio.getStyleClass().add("settings-label");
        sdRadio.setToggleGroup(storageGroup);
        sdRadio.setUserData((byte) 1);

        options.getChildren().addAll(
                slugCheckBox, stripCheckBox, sep, storageLabel,
                internalRadio, sdRadio);

        return options;
    }

    // ========================================================================
    // 文件列表表格
    // ========================================================================

    /**
     * 创建文件列表表格
     */
    private VBox createFileTable() {
        VBox section = new VBox();
        section.setSpacing(6);
        VBox.setVgrow(section, Priority.ALWAYS);

        fileTable.getStyleClass().add("song-table");
        fileTable.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        fileTable.getSelectionModel().setSelectionMode(SelectionMode.MULTIPLE);

        // 文件名列
        TableColumn<UploadFileRow, String> nameCol = new TableColumn<>("文件名");
        nameCol.setCellValueFactory(cell -> cell.getValue().name);
        nameCol.setMinWidth(200);
        nameCol.setPrefWidth(300);

        // 大小列
        TableColumn<UploadFileRow, String> sizeCol = new TableColumn<>("大小");
        sizeCol.setCellValueFactory(cell -> cell.getValue().size);
        sizeCol.setPrefWidth(90);
        sizeCol.setMaxWidth(100);

        // 格式列
        TableColumn<UploadFileRow, String> formatCol = new TableColumn<>("格式");
        formatCol.setCellValueFactory(cell -> cell.getValue().format);
        formatCol.setPrefWidth(70);
        formatCol.setMaxWidth(80);

        // 状态列
        TableColumn<UploadFileRow, String> statusCol = new TableColumn<>("状态");
        statusCol.setCellValueFactory(cell -> cell.getValue().status);
        statusCol.setPrefWidth(120);

        fileTable.getColumns().addAll(nameCol, sizeCol, formatCol, statusCol);

        section.getChildren().add(fileTable);
        return section;
    }

    // ========================================================================
    // 底部进度区
    // ========================================================================

    /**
     * 创建底部进度区域
     *
     * <p>包含进度条和进度文本。
     */
    private HBox createProgressSection() {
        HBox section = new HBox();
        section.getStyleClass().add("filter-bar");
        section.setAlignment(Pos.CENTER_LEFT);
        section.setSpacing(10);

        // 进度条
        progressBar.getStyleClass().add("progress-bar");
        progressBar.setPrefWidth(300);
        progressBar.setPrefHeight(18);

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        // 进度文本
        progressLabel.getStyleClass().add("filter-label");

        section.getChildren().addAll(progressBar, spacer, progressLabel);
        return section;
    }

    // ========================================================================
    // 文件操作
    // ========================================================================

    /**
     * 打开文件选择器添加音频文件
     *
     * <p>支持 MP3、WAV、WMA、M4A 等格式，允许多选。
     */
    private void addFiles() {
        FileChooser chooser = new FileChooser();
        chooser.setTitle("选择音频文件");
        chooser.getExtensionFilters().addAll(
                new FileChooser.ExtensionFilter("音频文件", "*.mp3", "*.wav", "*.wma", "*.m4a", "*.aac"),
                new FileChooser.ExtensionFilter("MP3 文件", "*.mp3"),
                new FileChooser.ExtensionFilter("所有文件", "*.*")
        );

        List<File> selected = chooser.showOpenMultipleDialog(this.getScene().getWindow());
        if (selected != null) {
            for (File f : selected) {
                fileList.add(new UploadFileRow(f));
            }
        }
    }

    /**
     * 开始上传
     *
     * <p>收集选中的文件和上传选项，触发回调。
     */
    private void startUpload() {
        if (fileList.isEmpty() || callback == null) {
            return;
        }

        // 获取选中的目标存储
        byte memUnit = 0; // 默认内置存储
        var selectedToggle = storageGroup.getSelectedToggle();
        if (selectedToggle != null && selectedToggle.getUserData() != null) {
            memUnit = (Byte) selectedToggle.getUserData();
        }

        // 收集文件列表
        List<File> files = new ArrayList<>();
        for (UploadFileRow row : fileList) {
            files.add(row.getFile());
        }

        boolean applySlug = slugCheckBox.isSelected();
        boolean applyStrip = stripCheckBox.isSelected();

        // 更新进度
        progressBar.setProgress(0);
        progressLabel.setText("正在上传 0/" + files.size() + "...");

        // 触发回调
        callback.onUploadFiles(files, memUnit, applySlug, applyStrip);
    }

    // ========================================================================
    // 公共方法
    // ========================================================================

    /**
     * 更新上传进度
     *
     * @param current  已完成数量
     * @param total    总数量
     * @param fileName 正在上传的文件名
     */
    public void updateProgress(int current, int total, String fileName) {
        double progress = total > 0 ? (double) current / total : 0;
        progressBar.setProgress(progress);
        progressLabel.setText("正在上传 " + current + "/" + total + ": " + fileName);
    }

    /**
     * 上传完成
     */
    public void uploadCompleted() {
        progressBar.setProgress(1.0);
        progressLabel.setText("上传完成");
        // 更新所有行状态
        for (UploadFileRow row : fileList) {
            row.setStatus("上传成功");
        }
    }

    /**
     * 更新单行上传状态
     *
     * @param index  行索引
     * @param status 状态文本
     */
    public void setRowStatus(int index, String status) {
        if (index >= 0 && index < fileList.size()) {
            fileList.get(index).setStatus(status);
        }
    }

    /**
     * 清空文件列表
     */
    public void clearFiles() {
        fileList.clear();
        progressBar.setProgress(0);
        progressLabel.setText("就绪");
    }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setCallback(UploadActionCallback callback) {
        this.callback = callback;
    }

    // ========================================================================
    // 工具方法
    // ========================================================================

    /**
     * 格式化文件大小（字节 → KB/MB/GB）
     */
    private static String formatSize(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return String.format("%.1f KB", bytes / 1024.0);
        if (bytes < 1024L * 1024 * 1024) return String.format("%.1f MB", bytes / (1024.0 * 1024.0));
        return String.format("%.2f GB", bytes / (1024.0 * 1024.0 * 1024.0));
    }

    /**
     * 从文件名提取格式（扩展名，大写）
     */
    private static String getFileFormat(String fileName) {
        if (fileName == null) return "—";
        int dot = fileName.lastIndexOf('.');
        if (dot < 0 || dot == fileName.length() - 1) return "—";
        return fileName.substring(dot + 1).toUpperCase();
    }
}
