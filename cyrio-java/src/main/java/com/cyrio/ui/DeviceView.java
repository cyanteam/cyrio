package com.cyrio.ui;

import javafx.application.Platform;
import javafx.collections.FXCollections;
import javafx.collections.ObservableList;
import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ListView;
import javafx.scene.control.ProgressIndicator;
import javafx.scene.control.TextField;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;

import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;

/**
 * 设备连接视图
 *
 * <p>自动扫描 Diamond USB 设备（VID=0x045a），8 秒间隔。
 * 提供设备列表、连接按钮和强制添加设备功能。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>扫描状态指示器（旋转图标 + 文本）</li>
 *   <li>设备列表（ListView，每项显示设备名称和 VID/PID）</li>
 *   <li>连接按钮 + 强制添加设备区域</li>
 *   <li>已连接设备信息面板（型号、固件版本、存储容量）</li>
 * </ul>
 *
 * <p>所有设备操作通过回调接口暴露，不直接调用 USB/设备 API（解耦设计）。
 */
public class DeviceView extends VBox {

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 设备信息（扫描结果） */
    public static class ScannedDevice {
        public final int vid;
        public final int pid;
        public final String name;
        public final String manufacturer;
        public final String serial;

        public ScannedDevice(int vid, int pid, String name, String manufacturer, String serial) {
            this.vid = vid;
            this.pid = pid;
            this.name = name != null ? name : "";
            this.manufacturer = manufacturer != null ? manufacturer : "";
            this.serial = serial != null ? serial : "";
        }

        /** 获取产品名，若为空则返回 VID/PID 格式的默认名 */
        public String getDisplayName() {
            if (name != null && !name.isEmpty()) {
                return name;
            }
            return String.format("USB 设备 (VID=0x%04x PID=0x%04x)", vid, pid);
        }

        /** 获取 VID/PID 字符串 */
        public String getVidPidString() {
            return String.format("VID=0x%04x PID=0x%04x", vid, pid);
        }

        /** 是否为 Diamond Rio 设备 */
        public boolean isDiamond() {
            return vid == 0x045a;
        }
    }

    /** 设备扫描回调 */
    public interface ScanCallback {
        /**
         * 请求扫描 USB 设备列表
         *
         * @return 扫描到的设备列表（回调在后台线程执行，返回值会被传递到 UI 线程）
         */
        List<ScannedDevice> onScanDevices();
    }

    /** 设备连接回调 */
    public interface ConnectCallback {
        /** 通过选中的设备连接 */
        void onConnectDevice(ScannedDevice device);

        /** 强制以指定 VID/PID 连接 */
        void onForceConnect(int vid, int pid);

        /** 断开连接 */
        void onDisconnect();
    }

    // ========================================================================
    // 字段
    // ========================================================================

    /** 扫描间隔（毫秒） */
    private static final long SCAN_INTERVAL_MS = 8000;

    /** 扫描线程池 */
    private final ScheduledExecutorService scanExecutor =
            Executors.newSingleThreadScheduledExecutor(r -> {
                Thread t = new Thread(r, "cyrio-device-scan");
                t.setDaemon(true);
                return t;
            });

    /** 扫描定时任务 */
    private ScheduledFuture<?> scanFuture;

    /** 扫描回调 */
    private ScanCallback scanCallback;

    /** 连接回调 */
    private ConnectCallback connectCallback;

    /** 设备列表数据 */
    private final ObservableList<ScannedDevice> deviceList = FXCollections.observableArrayList();

    /** 设备列表视图 */
    private final ListView<ScannedDevice> deviceListView = new ListView<>(deviceList);

    /** 扫描状态指示器 */
    private final ProgressIndicator scanIndicator = new ProgressIndicator();

    /** 扫描状态文本 */
    private final Label scanStatusLabel = new Label("正在扫描设备...");

    /** 连接按钮 */
    private final Button connectBtn = new Button("连接设备");

    /** 断开按钮 */
    private final Button disconnectBtn = new Button("断开设备");

    /** 强制添加 - VID 输入框 */
    private final TextField vidField = new TextField();

    /** 强制添加 - PID 输入框 */
    private final TextField pidField = new TextField();

    /** 强制连接按钮 */
    private final Button forceConnectBtn = new Button("强制添加");

    /** 已连接设备信息面板 */
    private final VBox deviceInfoPanel = new VBox();

    /** 设备型号标签 */
    private final Label modelLabel = new Label("—");

    /** 固件版本标签 */
    private final Label firmwareLabel = new Label("—");

    /** 内置存储标签 */
    private final Label internalStorageLabel = new Label("—");

    /** SD 卡存储标签 */
    private final Label sdStorageLabel = new Label("—");

    /** 是否已连接 */
    private boolean connected = false;

    // ========================================================================
    // 构造
    // ========================================================================

    public DeviceView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(12);

        // 扫描状态行
        this.getChildren().add(createScanStatusRow());

        // 设备列表
        this.getChildren().add(createDeviceListSection());

        // 操作按钮区
        this.getChildren().add(createActionButtons());

        // 强制添加区域
        this.getChildren().add(createForceConnectSection());

        // 已连接设备信息
        this.getChildren().add(createDeviceInfoPanel());

        // 启动自动扫描
        startAutoScan();
    }

    // ========================================================================
    // 扫描状态行
    // ========================================================================

    /**
     * 创建扫描状态指示行
     *
     * <p>左侧为旋转进度指示器 + 状态文本，右侧为手动刷新按钮。
     */
    private HBox createScanStatusRow() {
        HBox row = new HBox();
        row.getStyleClass().add("pane-header");
        row.setAlignment(Pos.CENTER_LEFT);

        // 扫描进度指示器（小圆圈旋转）
        scanIndicator.setProgress(ProgressIndicator.INDETERMINATE_PROGRESS);
        scanIndicator.setPrefSize(16, 16);
        scanIndicator.setMinSize(16, 16);
        scanIndicator.setMaxSize(16, 16);

        // 状态文本
        scanStatusLabel.getStyleClass().add("pane-header-title");
        scanStatusLabel.setText("正在扫描 Diamond 设备...");

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        // 手动刷新按钮
        Button refreshBtn = new Button("\u21BB 重新扫描");
        refreshBtn.getStyleClass().addAll("btn");
        refreshBtn.setOnAction(e -> triggerScan());

        row.getChildren().addAll(scanIndicator, scanStatusLabel, spacer, refreshBtn);

        return row;
    }

    // ========================================================================
    // 设备列表
    // ========================================================================

    /**
     * 创建设备列表区域
     */
    private VBox createDeviceListSection() {
        VBox section = new VBox();
        section.setSpacing(6);
        VBox.setVgrow(section, Priority.ALWAYS);

        // 区域标题
        Label title = new Label("发现设备");
        title.getStyleClass().add("settings-section-title");

        // ListView 配置
        deviceListView.getStyleClass().add("device-list-view");
        deviceListView.setCellFactory(lv -> new DeviceListCell());
        deviceListView.setPrefHeight(200);
        VBox.setVgrow(deviceListView, Priority.ALWAYS);

        section.getChildren().addAll(title, deviceListView);
        return section;
    }

    /**
     * 设备列表单元格
     *
     * <p>每行显示设备名称（大号字体）和 VID/PID 信息（小号灰色字体）。
     * Diamond 设备额外显示品牌标记。
     */
    private static class DeviceListCell extends javafx.scene.control.ListCell<ScannedDevice> {
        @Override
        protected void updateItem(ScannedDevice device, boolean empty) {
            super.updateItem(device, empty);
            if (empty || device == null) {
                setGraphic(null);
                setText(null);
            } else {
                VBox content = new VBox();
                content.setSpacing(2);

                // 设备名
                Label name = new Label(device.getDisplayName());
                name.getStyleClass().add("device-name");

                // VID/PID 信息
                Label vidpid = new Label(device.getVidPidString());
                vidpid.getStyleClass().add("device-vidpid");

                // 如果是 Diamond 设备，添加品牌标记
                if (device.isDiamond()) {
                    Label brand = new Label("Diamond Rio");
                    brand.getStyleClass().addAll("mem-badge-internal");
                    HBox nameRow = new HBox(8, name, brand);
                    nameRow.setAlignment(Pos.CENTER_LEFT);
                    content.getChildren().addAll(nameRow, vidpid);
                } else {
                    content.getChildren().addAll(name, vidpid);
                }

                setGraphic(content);
                setText(null);
            }
        }
    }

    // ========================================================================
    // 操作按钮区
    // ========================================================================

    /**
     * 创建操作按钮区
     *
     * <p>包含连接设备/断开设备按钮。
     */
    private HBox createActionButtons() {
        HBox row = new HBox();
        row.setSpacing(8);
        row.setAlignment(Pos.CENTER_LEFT);

        // 连接按钮
        connectBtn.getStyleClass().addAll("btn", "btn-primary");
        connectBtn.setOnAction(e -> {
            ScannedDevice selected = deviceListView.getSelectionModel().getSelectedItem();
            if (selected != null && connectCallback != null) {
                connectCallback.onConnectDevice(selected);
            }
        });

        // 断开按钮
        disconnectBtn.getStyleClass().addAll("btn", "btn-danger");
        disconnectBtn.setOnAction(e -> {
            if (connectCallback != null) {
                connectCallback.onDisconnect();
            }
        });
        disconnectBtn.setDisable(true);

        row.getChildren().addAll(connectBtn, disconnectBtn);
        return row;
    }

    // ========================================================================
    // 强制添加设备
    // ========================================================================

    /**
     * 创建强制添加设备区域
     *
     * <p>当自动识别失败时，用户可手动输入 VID/PID 强制连接。
     */
    private VBox createForceConnectSection() {
        VBox section = new VBox();
        section.getStyleClass().add("settings-section");
        section.setSpacing(8);

        // 区域标题
        Label title = new Label("强制添加设备");
        title.getStyleClass().add("settings-section-title");

        // 说明文本
        Label notice = new Label("当自动识别失败时，可手动输入 VID/PID 强制连接。");
        notice.getStyleClass().add("notice-label");
        notice.setWrapText(true);

        // 输入行
        HBox inputRow = new HBox();
        inputRow.setSpacing(8);
        inputRow.setAlignment(Pos.CENTER_LEFT);

        Label vidLabel = new Label("VID:");
        vidLabel.getStyleClass().add("settings-label");
        vidField.setPromptText("0x045a");
        vidField.setPrefWidth(80);
        vidField.getStyleClass().add("search-field");

        Label pidLabel = new Label("PID:");
        pidLabel.getStyleClass().add("settings-label");
        pidField.setPromptText("0x5006");
        pidField.setPrefWidth(80);
        pidField.getStyleClass().add("search-field");

        forceConnectBtn.getStyleClass().addAll("btn");
        forceConnectBtn.setOnAction(e -> {
            int vid = parseHexOrDecimal(vidField.getText().trim());
            int pid = parseHexOrDecimal(pidField.getText().trim());
            if (vid > 0 && pid > 0 && connectCallback != null) {
                connectCallback.onForceConnect(vid, pid);
            }
        });

        inputRow.getChildren().addAll(vidLabel, vidField, pidLabel, pidField, forceConnectBtn);

        section.getChildren().addAll(title, notice, inputRow);
        return section;
    }

    // ========================================================================
    // 已连接设备信息
    // ========================================================================

    /**
     * 创建已连接设备信息面板
     *
     * <p>显示型号、固件版本、存储容量等信息。
     */
    private VBox createDeviceInfoPanel() {
        deviceInfoPanel.getStyleClass().add("settings-section");
        deviceInfoPanel.setSpacing(8);
        deviceInfoPanel.setVisible(false);
        deviceInfoPanel.setManaged(false);

        // 标题
        Label title = new Label("设备信息");
        title.getStyleClass().add("settings-section-title");

        // 信息行
        deviceInfoPanel.getChildren().addAll(
                title,
                createInfoRow("型号:", modelLabel),
                createInfoRow("固件版本:", firmwareLabel),
                createInfoRow("内置存储:", internalStorageLabel),
                createInfoRow("SD 卡:", sdStorageLabel)
        );

        return deviceInfoPanel;
    }

    /**
     * 创建信息行
     */
    private HBox createInfoRow(String labelText, Label valueLabel) {
        HBox row = new HBox();
        row.getStyleClass().add("settings-row");

        Label label = new Label(labelText);
        label.getStyleClass().add("settings-label");
        label.setPrefWidth(90);

        valueLabel.getStyleClass().add("settings-value");

        row.getChildren().addAll(label, valueLabel);
        return row;
    }

    // ========================================================================
    // 自动扫描
    // ========================================================================

    /**
     * 启动自动扫描（8 秒间隔）
     *
     * <p>使用后台线程池定期调用回调扫描 USB 设备列表，
     * 结果在 JavaFX Application Thread 上更新 UI。
     */
    private void startAutoScan() {
        if (scanFuture != null) {
            scanFuture.cancel(false);
        }

        // 立即扫描一次
        triggerScan();

        // 定时扫描
        scanFuture = scanExecutor.scheduleAtFixedRate(
                this::doScanInBackground,
                SCAN_INTERVAL_MS,
                SCAN_INTERVAL_MS,
                TimeUnit.MILLISECONDS
        );
    }

    /**
     * 停止自动扫描
     */
    public void stopAutoScan() {
        if (scanFuture != null) {
            scanFuture.cancel(false);
            scanFuture = null;
        }
    }

    /**
     * 触发一次扫描（立即执行）
     */
    private void triggerScan() {
        scanExecutor.submit(this::doScanInBackground);
    }

    /**
     * 在后台线程执行扫描
     */
    private void doScanInBackground() {
        if (scanCallback == null) {
            Platform.runLater(() -> {
                scanStatusLabel.setText("未设置扫描回调");
                scanIndicator.setProgress(0);
            });
            return;
        }

        try {
            List<ScannedDevice> result = scanCallback.onScanDevices();

            Platform.runLater(() -> {
                deviceList.clear();
                if (result != null) {
                    deviceList.addAll(result);
                }

                // 更新状态文本
                int diamondCount = 0;
                if (result != null) {
                    for (ScannedDevice d : result) {
                        if (d.isDiamond()) diamondCount++;
                    }
                }

                if (diamondCount > 0) {
                    scanStatusLabel.setText("发现 " + diamondCount + " 台 Diamond 设备");
                } else if (result != null && !result.isEmpty()) {
                    scanStatusLabel.setText("未发现 Diamond 设备（" + result.size() + " 台其他 USB 设备）");
                } else {
                    scanStatusLabel.setText("未发现任何 USB 设备，等待重新扫描...");
                }
            });
        } catch (Exception e) {
            Platform.runLater(() -> {
                scanStatusLabel.setText("扫描失败: " + e.getMessage());
            });
        }
    }

    // ========================================================================
    // 公共方法
    // ========================================================================

    /**
     * 设置已连接状态
     *
     * <p>连接后显示设备信息面板，切换按钮状态。
     *
     * @param connected 是否已连接
     * @param model     设备型号
     * @param firmware  固件版本
     * @param internalInfo 内置存储信息文本
     * @param sdInfo    SD 卡存储信息文本
     */
    public void setConnected(boolean connected, String model, String firmware,
                             String internalInfo, String sdInfo) {
        this.connected = connected;

        if (connected) {
            // 显示设备信息
            deviceInfoPanel.setVisible(true);
            deviceInfoPanel.setManaged(true);

            modelLabel.setText(model != null ? model : "—");
            firmwareLabel.setText(firmware != null ? firmware : "—");
            internalStorageLabel.setText(internalInfo != null ? internalInfo : "—");
            sdStorageLabel.setText(sdInfo != null ? sdInfo : "—");

            // 按钮状态
            connectBtn.setDisable(true);
            disconnectBtn.setDisable(false);

            // 停止扫描
            scanStatusLabel.setText("设备已连接");
            scanIndicator.setProgress(1.0);
        } else {
            // 隐藏设备信息
            deviceInfoPanel.setVisible(false);
            deviceInfoPanel.setManaged(false);

            // 按钮状态
            connectBtn.setDisable(false);
            disconnectBtn.setDisable(true);

            // 恢复扫描
            scanIndicator.setProgress(ProgressIndicator.INDETERMINATE_PROGRESS);
            triggerScan();
        }
    }

    /**
     * 清理资源（关闭扫描线程池）
     *
     * <p>应在应用关闭时调用。
     */
    public void cleanup() {
        stopAutoScan();
        scanExecutor.shutdownNow();
    }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setScanCallback(ScanCallback callback) {
        this.scanCallback = callback;
    }

    public void setConnectCallback(ConnectCallback callback) {
        this.connectCallback = callback;
    }

    // ========================================================================
    // 工具方法
    // ========================================================================

    /**
     * 解析十六进制或十进制数字
     *
     * <p>支持 "0x045a" 和 "1114" 两种格式。
     *
     * @param text 输入文本
     * @return 解析后的数值，解析失败返回 0
     */
    private static int parseHexOrDecimal(String text) {
        if (text == null || text.isEmpty()) {
            return 0;
        }
        try {
            text = text.trim();
            if (text.toLowerCase().startsWith("0x")) {
                return Integer.parseInt(text.substring(2), 16);
            }
            return Integer.parseInt(text);
        } catch (NumberFormatException e) {
            return 0;
        }
    }
}
