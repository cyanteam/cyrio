package com.cyrio.ui;

import javafx.beans.property.SimpleDoubleProperty;
import javafx.beans.property.SimpleStringProperty;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.control.SelectionMode;
import javafx.scene.control.TableCell;
import javafx.scene.control.TableColumn;
import javafx.scene.control.TableView;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;

import java.util.ArrayList;
import java.util.List;

/**
 * 传输视图
 *
 * <p>显示文件传输队列，包括上传和下载任务，支持暂停/继续/取消操作。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>顶部工具栏：清空已完成 / 全部暂停 / 全部继续 / 刷新</li>
 *   <li>传输队列表格：文件名 / 方向 / 进度 / 状态 / 操作</li>
 * </ul>
 *
 * <p>所有传输操作通过回调接口暴露，不直接调用设备 API（解耦设计）。
 */
public class TransmissionView extends VBox {

    // ========================================================================
    // 传输方向枚举
    // ========================================================================

    /** 传输方向 */
    public enum TransferDirection {
        UPLOAD("上传"),
        DOWNLOAD("下载");

        private final String label;

        TransferDirection(String label) {
            this.label = label;
        }

        public String getLabel() {
            return label;
        }
    }

    // ========================================================================
    // 传输状态枚举
    // ========================================================================

    /** 传输状态 */
    public enum TransferStatus {
        PENDING("等待中"),
        TRANSFERRING("传输中"),
        PAUSED("已暂停"),
        DONE("已完成"),
        FAILED("失败"),
        CANCELED("已取消");

        private final String label;

        TransferStatus(String label) {
            this.label = label;
        }

        public String getLabel() {
            return label;
        }

        public boolean isFinal() {
            return this == DONE || this == FAILED || this == CANCELED;
        }
    }

    // ========================================================================
    // 传输项数据模型
    // ========================================================================

    /**
     * 传输项行包装类
     */
    public static class TransferItem {
        private final SimpleStringProperty fileName;
        private final SimpleStringProperty direction;
        private final SimpleDoubleProperty progress;
        private final SimpleStringProperty status;
        private TransferDirection directionEnum;
        private TransferStatus statusEnum;

        public TransferItem(String fileName, TransferDirection direction) {
            this.fileName = new SimpleStringProperty(fileName);
            this.direction = new SimpleStringProperty(direction.getLabel());
            this.progress = new SimpleDoubleProperty(0);
            this.status = new SimpleStringProperty(TransferStatus.PENDING.getLabel());
            this.directionEnum = direction;
            this.statusEnum = TransferStatus.PENDING;
        }

        public String getFileName() { return fileName.get(); }
        public String getDirection() { return direction.get(); }
        public double getProgress() { return progress.get(); }
        public String getStatus() { return status.get(); }
        public TransferDirection getDirectionEnum() { return directionEnum; }
        public TransferStatus getStatusEnum() { return statusEnum; }

        public void setProgress(double value) { progress.set(value); }
        public void setStatus(TransferStatus status) {
            this.statusEnum = status;
            this.status.set(status.getLabel());
        }
    }

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 传输操作回调 */
    public interface TransferActionCallback {
        void onPause(TransferItem item);
        void onResume(TransferItem item);
        void onCancel(TransferItem item);
    }

    // ========================================================================
    // 字段
    // ========================================================================

    /** 传输队列数据 */
    private final ObservableList<TransferItem> transferList = FXCollections.observableArrayList();

    /** 传输队列表格 */
    private final TableView<TransferItem> transferTable = new TableView<>(transferList);

    /** 回调 */
    private TransferActionCallback callback;

    /** 队列数量标签 */
    private final Label countLabel = new Label("0 个任务");

    // ========================================================================
    // 构造
    // ========================================================================

    public TransmissionView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(8);

        // 顶部工具栏
        this.getChildren().add(createToolbar());

        // 传输队列表格
        this.getChildren().add(createTransferTable());
    }

    // ========================================================================
    // 顶部工具栏
    // ========================================================================

    /**
     * 创建顶部工具栏
     *
     * <p>包含：清空已完成 / 全部暂停 / 全部继续 / 刷新
     */
    private HBox createToolbar() {
        HBox toolbar = new HBox();
        toolbar.getStyleClass().add("batch-toolbar");

        // 清空已完成按钮
        Button clearBtn = new Button("清空已完成");
        clearBtn.getStyleClass().add("batch-btn");
        clearBtn.setOnAction(e -> {
            var toRemove = new ArrayList<TransferItem>();
            for (TransferItem item : transferList) {
                if (item.getStatusEnum().isFinal()) {
                    toRemove.add(item);
                }
            }
            transferList.removeAll(toRemove);
            updateCount();
        });

        // 全部暂停按钮
        Button pauseAllBtn = new Button("全部暂停");
        pauseAllBtn.getStyleClass().add("batch-btn");
        pauseAllBtn.setOnAction(e -> {
            for (TransferItem item : transferList) {
                if (item.getStatusEnum() == TransferStatus.TRANSFERRING && callback != null) {
                    callback.onPause(item);
                }
            }
        });

        // 全部继续按钮
        Button resumeAllBtn = new Button("全部继续");
        resumeAllBtn.getStyleClass().add("batch-btn");
        resumeAllBtn.setOnAction(e -> {
            for (TransferItem item : transferList) {
                if (item.getStatusEnum() == TransferStatus.PAUSED && callback != null) {
                    callback.onResume(item);
                }
            }
        });

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        // 数量标签
        countLabel.getStyleClass().add("filter-label");

        toolbar.getChildren().addAll(clearBtn, pauseAllBtn, resumeAllBtn, spacer, countLabel);
        return toolbar;
    }

    // ========================================================================
    // 传输队列表格
    // ========================================================================

    /**
     * 创建传输队列表格
     */
    private TableView<TransferItem> createTransferTable() {
        transferTable.getStyleClass().add("song-table");
        transferTable.setColumnResizePolicy(TableView.CONSTRAINED_RESIZE_POLICY);
        transferTable.getSelectionModel().setSelectionMode(SelectionMode.MULTIPLE);
        VBox.setVgrow(transferTable, Priority.ALWAYS);

        // 文件名列
        TableColumn<TransferItem, String> nameCol = new TableColumn<>("文件名");
        nameCol.setCellValueFactory(cell -> cell.getValue().fileName);
        nameCol.setMinWidth(180);
        nameCol.setPrefWidth(240);

        // 方向列
        TableColumn<TransferItem, String> dirCol = new TableColumn<>("方向");
        dirCol.setCellValueFactory(cell -> cell.getValue().direction);
        dirCol.setPrefWidth(60);
        dirCol.setMaxWidth(80);
        dirCol.setCellFactory(col -> new TableCell<>() {
            @Override
            protected void updateItem(String value, boolean empty) {
                super.updateItem(value, empty);
                if (empty || value == null) {
                    setText(null);
                    getStyleClass().removeAll("transfer-direction", "transfer-direction-download");
                } else {
                    setText(value);
                    getStyleClass().removeAll("transfer-direction", "transfer-direction-download");
                    if ("下载".equals(value)) {
                        getStyleClass().add("transfer-direction-download");
                    } else {
                        getStyleClass().add("transfer-direction");
                    }
                }
            }
        });

        // 进度列
        TableColumn<TransferItem, Number> progressCol = new TableColumn<>("进度");
        progressCol.setCellValueFactory(cell -> cell.getValue().progress);
        progressCol.setPrefWidth(160);
        progressCol.setCellFactory(col -> new TableCell<>() {
            private final ProgressBar bar = new ProgressBar(0);
            {
                bar.getStyleClass().add("progress-bar");
                bar.setPrefWidth(140);
            }

            @Override
            protected void updateItem(Number value, boolean empty) {
                super.updateItem(value, empty);
                if (empty || value == null) {
                    setGraphic(null);
                } else {
                    bar.setProgress(value.doubleValue());
                    setGraphic(bar);
                }
            }
        });

        // 状态列
        TableColumn<TransferItem, String> statusCol = new TableColumn<>("状态");
        statusCol.setCellValueFactory(cell -> cell.getValue().status);
        statusCol.setPrefWidth(80);
        statusCol.setMaxWidth(100);
        statusCol.setCellFactory(col -> new TableCell<>() {
            @Override
            protected void updateItem(String value, boolean empty) {
                super.updateItem(value, empty);
                if (empty || value == null) {
                    setText(null);
                    getStyleClass().removeAll(
                            "transfer-status-pending", "transfer-status-done", "transfer-status-failed");
                } else {
                    setText(value);
                    getStyleClass().removeAll(
                            "transfer-status-pending", "transfer-status-done", "transfer-status-failed");
                    switch (value) {
                        case "已完成" -> getStyleClass().add("transfer-status-done");
                        case "失败", "已取消" -> getStyleClass().add("transfer-status-failed");
                        default -> getStyleClass().add("transfer-status-pending");
                    }
                }
            }
        });

        // 操作列
        TableColumn<TransferItem, Void> actionCol = new TableColumn<>("操作");
        actionCol.setPrefWidth(120);
        actionCol.setCellFactory(col -> new TableCell<>() {
            private final Button pauseBtn = new Button("暂停");
            private final Button resumeBtn = new Button("继续");
            private final Button cancelBtn = new Button("取消");
            private final HBox buttons = new HBox(4, pauseBtn, resumeBtn, cancelBtn);

            {
                pauseBtn.getStyleClass().add("batch-btn");
                resumeBtn.getStyleClass().add("batch-btn");
                cancelBtn.getStyleClass().addAll("batch-btn", "batch-btn-danger");

                pauseBtn.setOnAction(e -> {
                    TransferItem item = getTableView().getItems().get(getIndex());
                    if (callback != null) callback.onPause(item);
                });
                resumeBtn.setOnAction(e -> {
                    TransferItem item = getTableView().getItems().get(getIndex());
                    if (callback != null) callback.onResume(item);
                });
                cancelBtn.setOnAction(e -> {
                    TransferItem item = getTableView().getItems().get(getIndex());
                    if (callback != null) callback.onCancel(item);
                });
            }

            @Override
            protected void updateItem(Void value, boolean empty) {
                super.updateItem(value, empty);
                if (empty) {
                    setGraphic(null);
                } else {
                    TransferItem item = getTableView().getItems().get(getIndex());
                    TransferStatus status = item.getStatusEnum();

                    // 根据状态显示/隐藏按钮
                    pauseBtn.setVisible(status == TransferStatus.TRANSFERRING);
                    resumeBtn.setVisible(status == TransferStatus.PAUSED);
                    cancelBtn.setVisible(!status.isFinal());

                    setGraphic(buttons);
                }
            }
        });

        transferTable.getColumns().addAll(nameCol, dirCol, progressCol, statusCol, actionCol);

        return transferTable;
    }

    // ========================================================================
    // 公共方法
    // ========================================================================

    /**
     * 添加传输项到队列
     *
     * @param fileName  文件名
     * @param direction 传输方向（上传/下载）
     */
    public void addTransferItem(String fileName, TransferDirection direction) {
        transferList.add(new TransferItem(fileName, direction));
        updateCount();
    }

    /**
     * 更新传输项的进度
     *
     * @param index    传输项索引
     * @param progress 进度（0.0 ~ 1.0）
     */
    public void updateProgress(int index, double progress) {
        if (index >= 0 && index < transferList.size()) {
            transferList.get(index).setProgress(progress);
        }
    }

    /**
     * 更新传输项的状态
     *
     * @param index  传输项索引
     * @param status 新状态
     */
    public void setStatus(int index, TransferStatus status) {
        if (index >= 0 && index < transferList.size()) {
            transferList.get(index).setStatus(status);
            transferTable.refresh();
        }
    }

    /**
     * 清空所有传输项
     */
    public void clearAll() {
        transferList.clear();
        updateCount();
    }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setCallback(TransferActionCallback callback) {
        this.callback = callback;
    }

    // ========================================================================
    // 工具方法
    // ========================================================================

    /**
     * 更新数量标签
     */
    private void updateCount() {
        int total = transferList.size();
        int active = 0;
        for (TransferItem item : transferList) {
            if (!item.getStatusEnum().isFinal()) {
                active++;
            }
        }
        countLabel.setText(active + " 个活跃 / " + total + " 个任务");
    }
}
