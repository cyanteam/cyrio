package com.cyrio.ui;

import javafx.beans.property.SimpleStringProperty;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.ComboBox;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.SelectionMode;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableView;
import javafx.scene.control.TextField;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;

import java.io.File;
import java.util.ArrayList;
import java.util.List;

/**
 * 同步视图
 *
 * <p>管理同步规则，将本地目录与设备存储进行双向同步。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>顶部工具栏：添加规则 / 删除规则 / 执行同步</li>
 *   <li>规则列表表格：源目录 / 目标存储 / 文件类型 / 状态</li>
 *   <li>底部进度条：同步进度</li>
 * </ul>
 *
 * <p>所有设备操作通过回调接口暴露，不直接调用设备 API（解耦设计）。
 */
public class SyncView extends VBox {

    // ========================================================================
    // 同步规则数据模型
    // ========================================================================

    /**
     * 同步规则行包装类
     */
    public static class SyncRuleRow {
        private final SimpleStringProperty sourceDir;
        private final SimpleStringProperty targetStorage;
        private final SimpleStringProperty fileTypes;
        private final SimpleStringProperty status;

        public SyncRuleRow(String sourceDir, String targetStorage, String fileTypes) {
            this.sourceDir = new SimpleStringProperty(sourceDir);
            this.targetStorage = new SimpleStringProperty(targetStorage);
            this.fileTypes = new SimpleStringProperty(fileTypes);
            this.status = new SimpleStringProperty("就绪");
        }

        public String getSourceDir() { return sourceDir.get(); }
        public String getTargetStorage() { return targetStorage.get(); }
        public String getFileTypes() { return fileTypes.get(); }
        public String getStatus() { return status.get(); }
        public void setStatus(String value) { status.set(value); }
    }

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 同步操作回调 */
    public interface SyncActionCallback {
        /**
         * 执行同步
         *
         * @param rules 要同步的规则列表
         */
        void onExecuteSync(List<SyncRuleRow> rules);

        /** 保存规则（持久化） */
        void onSaveRules(List<SyncRuleRow> rules);

        /** 加载已保存的规则 */
        void onLoadRules();
    }

    // ========================================================================
    // 字段
    // ========================================================================

    /** 规则列表数据 */
    private final ObservableList<SyncRuleRow> ruleList = FXCollections.observableArrayList();

    /** 规则列表表格 */
    private final TableView<SyncRuleRow> ruleTable = new TableView<>(ruleList);

    /** 源目录输入框 */
    private final TextField sourceDirField = new TextField();

    /** 目标存储下拉框 */
    private final ComboBox<String> targetStorageCombo = new ComboBox<>();

    /** 文件类型输入框 */
    private final TextField fileTypesField = new TextField();

    /** 同步进度条 */
    private final ProgressBar progressBar = new ProgressBar(0);

    /** 进度文本 */
    private final Label progressLabel = new Label("就绪");

    /** 回调 */
    private SyncActionCallback callback;

    // ========================================================================
    // 构造
    // ========================================================================

    public SyncView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(8);

        // 顶部工具栏
        this.getChildren().add(createToolbar());

        // 添加规则区
        this.getChildren().add(createAddRuleSection());

        // 规则列表
        this.getChildren().add(createRuleTable());

        // 底部进度
        this.getChildren().add(createProgressSection());
    }

    // ========================================================================
    // 顶部工具栏
    // ========================================================================

    /**
     * 创建顶部工具栏
     *
     * <p>包含：删除选中规则 / 执行同步 / 保存规则
     */
    private HBox createToolbar() {
        HBox toolbar = new HBox();
        toolbar.getStyleClass().add("batch-toolbar");

        // 删除规则按钮
        Button deleteBtn = new Button("删除选中规则");
        deleteBtn.getStyleClass().addAll("batch-btn", "batch-btn-danger");
        deleteBtn.setOnAction(e -> {
            var selected = new ArrayList<>(ruleTable.getSelectionModel().getSelectedItems());
            ruleList.removeAll(selected);
        });

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        // 保存规则按钮
        Button saveBtn = new Button("保存规则");
        saveBtn.getStyleClass().add("batch-btn");
        saveBtn.setOnAction(e -> {
            if (callback != null) {
                callback.onSaveRules(new ArrayList<>(ruleList));
            }
        });

        // 执行同步按钮
        Button syncBtn = new Button("\u21BB 执行同步");
        syncBtn.getStyleClass().addAll("btn", "btn-primary");
        syncBtn.setOnAction(e -> {
            if (!ruleList.isEmpty() && callback != null) {
                progressBar.setProgress(0);
                progressLabel.setText("正在同步...");
                callback.onExecuteSync(new ArrayList<>(ruleList));
            }
        });

        toolbar.getChildren().addAll(deleteBtn, spacer, saveBtn, syncBtn);
        return toolbar;
    }

    // ========================================================================
    // 添加规则区
    // ========================================================================

    /**
     * 创建添加规则区域
     *
     * <p>包含：源目录输入 + 浏览按钮 + 目标存储下拉 + 文件类型 + 添加按钮
     */
    private VBox createAddRuleSection() {
        VBox section = new VBox();
        section.getStyleClass().add("settings-section");
        section.setSpacing(8);

        // 标题
        Label title = new Label("添加同步规则");
        title.getStyleClass().add("settings-section-title");

        // 输入行
        HBox inputRow = new HBox();
        inputRow.setSpacing(8);
        inputRow.setAlignment(Pos.CENTER_LEFT);

        // 源目录
        Label sourceLabel = new Label("源目录:");
        sourceLabel.getStyleClass().add("settings-label");
        sourceDirField.setPromptText("选择本地目录...");
        sourceDirField.setPrefWidth(250);
        sourceDirField.getStyleClass().add("search-field");

        // 浏览按钮
        Button browseBtn = new Button("浏览");
        browseBtn.getStyleClass().add("batch-btn");
        browseBtn.setOnAction(e -> browseDirectory());

        // 目标存储
        Label targetLabel = new Label("目标存储:");
        targetLabel.getStyleClass().add("settings-label");
        targetStorageCombo.getItems().addAll("内置存储", "SD 卡");
        targetStorageCombo.getSelectionModel().selectFirst();
        targetStorageCombo.getStyleClass().add("search-field");

        // 文件类型
        Label typesLabel = new Label("文件类型:");
        typesLabel.getStyleClass().add("settings-label");
        fileTypesField.setPromptText("*.mp3,*.wav");
        fileTypesField.setPrefWidth(120);
        fileTypesField.getStyleClass().add("search-field");
        fileTypesField.setText("*.mp3");

        // 添加按钮
        Button addBtn = new Button("+ 添加规则");
        addBtn.getStyleClass().addAll("btn");
        addBtn.setOnAction(e -> addRule());

        inputRow.getChildren().addAll(
                sourceLabel, sourceDirField, browseBtn,
                targetLabel, targetStorageCombo,
                typesLabel, fileTypesField,
                addBtn);

        section.getChildren().addAll(title, inputRow);
        return section;
    }

    // ========================================================================
    // 规则列表表格
    // ========================================================================

    /**
     * 创建规则列表表格
     */
    private VBox createRuleTable() {
        VBox section = new VBox();
        section.setSpacing(6);
        VBox.setVgrow(section, Priority.ALWAYS);

        ruleTable.getStyleClass().add("song-table");
        ruleTable.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        ruleTable.getSelectionModel().setSelectionMode(SelectionMode.MULTIPLE);

        // 源目录列
        TableColumn<SyncRuleRow, String> sourceCol = new TableColumn<>("源目录");
        sourceCol.setCellValueFactory(cell -> cell.getValue().sourceDir);
        sourceCol.setMinWidth(200);
        sourceCol.setPrefWidth(280);

        // 目标存储列
        TableColumn<SyncRuleRow, String> targetCol = new TableColumn<>("目标存储");
        targetCol.setCellValueFactory(cell -> cell.getValue().targetStorage);
        targetCol.setPrefWidth(100);
        targetCol.setMaxWidth(120);

        // 文件类型列
        TableColumn<SyncRuleRow, String> typesCol = new TableColumn<>("文件类型");
        typesCol.setCellValueFactory(cell -> cell.getValue().fileTypes);
        typesCol.setPrefWidth(120);

        // 状态列
        TableColumn<SyncRuleRow, String> statusCol = new TableColumn<>("状态");
        statusCol.setCellValueFactory(cell -> cell.getValue().status);
        statusCol.setPrefWidth(100);

        ruleTable.getColumns().addAll(sourceCol, targetCol, typesCol, statusCol);

        section.getChildren().add(ruleTable);
        return section;
    }

    // ========================================================================
    // 底部进度区
    // ========================================================================

    /**
     * 创建底部进度区域
     */
    private HBox createProgressSection() {
        HBox section = new HBox();
        section.getStyleClass().add("filter-bar");
        section.setAlignment(Pos.CENTER_LEFT);
        section.setSpacing(10);

        progressBar.getStyleClass().add("progress-bar");
        progressBar.setPrefWidth(300);
        progressBar.setPrefHeight(18);

        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        progressLabel.getStyleClass().add("filter-label");

        section.getChildren().addAll(progressBar, spacer, progressLabel);
        return section;
    }

    // ========================================================================
    // 操作方法
    // ========================================================================

    /**
     * 打开目录选择器
     */
    private void browseDirectory() {
        javafx.stage.DirectoryChooser chooser = new javafx.stage.DirectoryChooser();
        chooser.setTitle("选择同步源目录");
        File selected = chooser.showDialog(this.getScene().getWindow());
        if (selected != null) {
            sourceDirField.setText(selected.getAbsolutePath());
        }
    }

    /**
     * 添加同步规则
     */
    private void addRule() {
        String source = sourceDirField.getText().trim();
        String target = targetStorageCombo.getValue();
        String types = fileTypesField.getText().trim();

        if (source.isEmpty() || target == null) {
            return;
        }

        ruleList.add(new SyncRuleRow(source, target, types.isEmpty() ? "*.mp3" : types));
        sourceDirField.clear();
    }

    // ========================================================================
    // 公共方法
    // ========================================================================

    /**
     * 更新同步进度
     *
     * @param current  已完成数量
     * @param total    总数量
     * @param message  当前操作描述
     */
    public void updateProgress(int current, int total, String message) {
        double progress = total > 0 ? (double) current / total : 0;
        progressBar.setProgress(progress);
        progressLabel.setText("同步 " + current + "/" + total + ": " + message);
    }

    /**
     * 同步完成
     */
    public void syncCompleted() {
        progressBar.setProgress(1.0);
        progressLabel.setText("同步完成");
        for (SyncRuleRow row : ruleList) {
            row.setStatus("已同步");
        }
    }

    /**
     * 设置规则列表
     */
    public void setRules(List<SyncRuleRow> rules) {
        ruleList.clear();
        if (rules != null) {
            ruleList.addAll(rules);
        }
    }

    /**
     * 更新规则状态
     */
    public void setRuleStatus(int index, String status) {
        if (index >= 0 && index < ruleList.size()) {
            ruleList.get(index).setStatus(status);
        }
    }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setCallback(SyncActionCallback callback) {
        this.callback = callback;
    }
}
