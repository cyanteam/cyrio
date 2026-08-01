package com.cyrio.ui;

import javafx.beans.property.SimpleStringProperty;
import javafx.beans.property.StringProperty;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Node;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.StackPane;
import javafx.scene.layout.VBox;
import javafx.stage.Stage;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * 主窗口容器
 *
 * <p>包含三个主要区域：
 * <ol>
 *   <li>标题栏（28px 高，#39c5bb 背景，3 个红绿灯按钮，支持窗口拖拽）</li>
 *   <li>菜单栏（8 个菜单项水平排列，当前选中项高亮）</li>
 *   <li>内容区（根据选中的菜单显示不同视图）</li>
 * </ol>
 *
 * <p>所有 UI 操作通过回调接口暴露，不直接调用设备 API（解耦设计）。
 * 控制器层负责将回调连接到实际的设备操作。
 *
 * <p>对应 Rust 前端 {@code CyrioLauncher.tsx} 的主布局逻辑。
 */
public class MainWindow extends VBox {

    // ========================================================================
    // 菜单定义
    // ========================================================================

    /** 菜单动作枚举 */
    public enum MenuAction {
        SONGS("歌曲"),
        PLAYLISTS("歌单"),
        UPLOAD("上传"),
        SYNC("同步"),
        TRANSMISSION("传输"),
        DEVICE("设备"),
        SETTINGS("设置"),
        ABOUT("关于");

        private final String label;

        MenuAction(String label) {
            this.label = label;
        }

        public String getLabel() {
            return label;
        }
    }

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 窗口控制回调 */
    public interface WindowCallback {
        void onMinimize();
        void onMaximizeToggle();
        void onClose();
    }

    /** 菜单切换回调 */
    public interface MenuSwitchCallback {
        void onMenuSwitched(MenuAction action);
    }

    /** 设备连接/断开回调 */
    public interface DeviceConnectCallback {
        void onConnect();
        void onDisconnect();
    }

    // ========================================================================
    // 字段
    // ========================================================================

    private final Stage stage;

    /** 标题栏标题文本属性 */
    private final StringProperty titleText = new SimpleStringProperty("Cyrio Ver 0.1.0 开源软件，请勿商用");

    /** 当前选中的菜单 */
    private MenuAction currentMenu = MenuAction.SONGS;

    /** 菜单按钮映射 */
    private final Map<MenuAction, Button> menuButtons = new LinkedHashMap<>();

    /** 内容区容器 */
    private final StackPane contentArea = new StackPane();

    /** 各视图实例 */
    private final SongsView songsView;
    private final PlaylistsView playlistsView;
    private final UploadView uploadView;
    private final SyncView syncView;
    private final TransmissionView transmissionView;
    private final DeviceView deviceView;
    private final SettingsView settingsView;
    private final AboutView aboutView;

    /** 设备状态栏组件 */
    private final Label deviceStatusLabel = new Label("未连接");
    private final Region deviceStatusDot = new Region();
    private final Label internalStorageLabel = new Label("");
    private final Label sdStorageLabel = new Label("");

    /** 回调 */
    private WindowCallback windowCallback;
    private MenuSwitchCallback menuSwitchCallback;
    private DeviceConnectCallback deviceConnectCallback;

    /** 窗口拖拽坐标 */
    private double dragOffsetX;
    private double dragOffsetY;

    /** 是否已连接设备 */
    private boolean connected = false;

    // ========================================================================
    // 构造
    // ========================================================================

    /**
     * 创建主窗口
     *
     * @param stage JavaFX Stage（用于窗口拖拽控制）
     */
    public MainWindow(Stage stage) {
        this.stage = stage;

        // 创建各视图
        this.songsView = new SongsView();
        this.playlistsView = new PlaylistsView();
        this.uploadView = new UploadView();
        this.syncView = new SyncView();
        this.transmissionView = new TransmissionView();
        this.deviceView = new DeviceView();
        this.settingsView = new SettingsView();
        this.aboutView = new AboutView();

        // 设置 VBox 属性
        this.getStyleClass().add("root");
        this.setSpacing(0);

        // 构建各区域
        this.getChildren().addAll(
                createTitleBar(),
                createMenuBar(),
                createDeviceStatusBar(),
                createContentArea()
        );

        // 默认显示歌曲视图
        switchTo(MenuAction.SONGS);
    }

    // ========================================================================
    // 标题栏
    // ========================================================================

    /**
     * 创建自定义标题栏
     *
     * <p>28px 高，#39c5bb 背景，3 个红绿灯按钮（关闭/最小化/最大化）。
     * 支持鼠标拖拽移动窗口。
     */
    private HBox createTitleBar() {
        HBox titleBar = new HBox();
        titleBar.getStyleClass().add("title-bar");
        titleBar.setAlignment(Pos.CENTER_LEFT);

        // 标题文本
        Label titleLabel = new Label();
        titleLabel.getStyleClass().add("title-bar-label");
        titleLabel.textProperty().bind(titleText);
        titleLabel.setMaxWidth(Double.MAX_VALUE);

        // 最小化按钮
        Button minimizeBtn = new Button("\u2013"); // EN DASH
        minimizeBtn.getStyleClass().addAll("traffic-btn");
        minimizeBtn.setOnAction(e -> {
            if (windowCallback != null) windowCallback.onMinimize();
        });

        // 最大化/还原按钮
        Button maximizeBtn = new Button("\u25A1"); // WHITE SQUARE
        maximizeBtn.getStyleClass().addAll("traffic-btn");
        maximizeBtn.setOnAction(e -> {
            if (windowCallback != null) windowCallback.onMaximizeToggle();
        });

        // 关闭按钮
        Button closeBtn = new Button("\u2715"); // MULTIPLICATION X
        closeBtn.getStyleClass().addAll("traffic-btn", "traffic-btn-close");
        closeBtn.setOnAction(e -> {
            if (windowCallback != null) windowCallback.onClose();
        });

        // 按钮容器
        HBox buttonsBox = new HBox(minimizeBtn, maximizeBtn, closeBtn);
        buttonsBox.getStyleClass().add("title-bar-buttons");

        titleBar.getChildren().addAll(titleLabel, buttonsBox);
        HBox.setHgrow(titleLabel, Priority.ALWAYS);

        // 窗口拖拽（点击标题栏空白区域并拖动）
        titleBar.setOnMousePressed(e -> {
            dragOffsetX = e.getScreenX() - stage.getX();
            dragOffsetY = e.getScreenY() - stage.getY();
        });
        titleBar.setOnMouseDragged(e -> {
            stage.setX(e.getScreenX() - dragOffsetX);
            stage.setY(e.getScreenY() - dragOffsetY);
        });

        return titleBar;
    }

    // ========================================================================
    // 菜单栏
    // ========================================================================

    /**
     * 创建菜单栏
     *
     * <p>水平排列 8 个菜单按钮，当前选中项高亮。
     * 未连接设备时部分菜单不可用。
     */
    private HBox createMenuBar() {
        HBox menuBar = new HBox();
        menuBar.getStyleClass().add("menu-bar");
        menuBar.setAlignment(Pos.CENTER_LEFT);

        for (MenuAction action : MenuAction.values()) {
            Button btn = new Button(action.getLabel());
            btn.getStyleClass().add("menu-item");
            btn.setOnAction(e -> switchTo(action));
            menuButtons.put(action, btn);
            menuBar.getChildren().add(btn);
        }

        // 添加弹性间隔（将菜单推到左侧）
        Region spacer = new Region();
        spacer.setMaxWidth(Double.MAX_VALUE);
        HBox.setHgrow(spacer, Priority.ALWAYS);
        menuBar.getChildren().add(spacer);

        // 连接/断开按钮
        Button connectBtn = new Button("连接设备");
        connectBtn.getStyleClass().addAll("btn", "btn-primary");
        connectBtn.setOnAction(e -> {
            if (connected) {
                if (deviceConnectCallback != null) deviceConnectCallback.onDisconnect();
            } else {
                if (deviceConnectCallback != null) deviceConnectCallback.onConnect();
            }
        });
        connectBtn.setId("connectBtn");
        menuBar.getChildren().add(connectBtn);

        return menuBar;
    }

    // ========================================================================
    // 设备状态栏
    // ========================================================================

    /**
     * 创建设备状态栏
     *
     * <p>横向排列，显示连接状态和存储信息（内置存储/SD 卡的容量使用情况）。
     */
    private HBox createDeviceStatusBar() {
        HBox statusBar = new HBox();
        statusBar.getStyleClass().add("device-status-bar");
        statusBar.setAlignment(Pos.CENTER_LEFT);
        statusBar.setPadding(new Insets(6, 12, 6, 12));

        // 状态点
        deviceStatusDot.getStyleClass().addAll("device-status-dot", "device-status-dot-disconnected");

        // 状态标签
        Label statusLabelTitle = new Label("设备:");
        statusLabelTitle.getStyleClass().add("device-status-label");
        deviceStatusLabel.getStyleClass().add("device-status-value");

        // 内置存储标签
        Label internalTitle = new Label("内置存储:");
        internalTitle.getStyleClass().add("device-status-label");
        internalStorageLabel.getStyleClass().add("device-status-value");

        // SD 卡标签
        Label sdTitle = new Label("SD 卡:");
        sdTitle.getStyleClass().add("device-status-label");
        sdStorageLabel.getStyleClass().add("device-status-value");

        statusBar.getChildren().addAll(
                deviceStatusDot,
                statusLabelTitle,
                deviceStatusLabel,
                createSpacer(),
                internalTitle,
                internalStorageLabel,
                createSpacer(),
                sdTitle,
                sdStorageLabel
        );

        return statusBar;
    }

    /**
     * 创建弹性间隔组件
     */
    private Region createSpacer() {
        Region spacer = new Region();
        spacer.setMaxWidth(Double.MAX_VALUE);
        HBox.setHgrow(spacer, Priority.ALWAYS);
        return spacer;
    }

    // ========================================================================
    // 内容区
    // ========================================================================

    /**
     * 创建内容区容器
     *
     * <p>使用 StackPane 作为视图切换容器，根据菜单选择显示对应视图。
     */
    private StackPane createContentArea() {
        contentArea.getStyleClass().add("content-area");
        StackPane.setMargin(contentArea, new Insets(0));

        // 添加所有视图到内容区
        contentArea.getChildren().addAll(
                songsView,
                playlistsView,
                uploadView,
                syncView,
                transmissionView,
                deviceView,
                settingsView,
                aboutView
        );

        return contentArea;
    }

    // ========================================================================
    // 视图切换
    // ========================================================================

    /**
     * 切换到指定菜单视图
     *
     * @param action 菜单动作
     */
    public void switchTo(MenuAction action) {
        this.currentMenu = action;

        // 更新菜单按钮高亮状态
        for (Map.Entry<MenuAction, Button> entry : menuButtons.entrySet()) {
            Button btn = entry.getValue();
            btn.getStyleClass().remove("menu-item-active");
            if (entry.getKey() == action) {
                btn.getStyleClass().add("menu-item-active");
            }
        }

        // 显示对应视图，隐藏其他
        for (Node node : contentArea.getChildren()) {
            node.setVisible(false);
            node.setManaged(false);
        }

        Node target = getView(action);
        if (target != null) {
            target.setVisible(true);
            target.setManaged(true);
        }

        // 更新标题栏文本
        updateTitleText(action);

        // 触发菜单切换回调
        if (menuSwitchCallback != null) {
            menuSwitchCallback.onMenuSwitched(action);
        }
    }

    /**
     * 根据菜单动作获取对应视图
     */
    private Node getView(MenuAction action) {
        return switch (action) {
            case SONGS -> songsView;
            case PLAYLISTS -> playlistsView;
            case UPLOAD -> uploadView;
            case SYNC -> syncView;
            case TRANSMISSION -> transmissionView;
            case DEVICE -> deviceView;
            case SETTINGS -> settingsView;
            case ABOUT -> aboutView;
        };
    }

    /**
     * 更新标题栏文本
     *
     * <p>格式：[设备型号] [正在传输] 页面名 Cyrio Ver 0.1.0 开源软件，请勿商用
     */
    private void updateTitleText(MenuAction action) {
        StringBuilder sb = new StringBuilder();
        if (connected) {
            sb.append("[Rio S50] ");
        }
        sb.append(action.getLabel());
        sb.append(" Cyrio Ver 0.1.0 开源软件，请勿商用");
        titleText.set(sb.toString());
    }

    // ========================================================================
    // 公共方法：更新设备状态
    // ========================================================================

    /**
     * 更新设备连接状态
     *
     * @param connected 是否已连接
     * @param deviceName 设备名称（如 "Rio S50"）
     */
    public void setDeviceConnected(boolean connected, String deviceName) {
        this.connected = connected;

        // 更新状态点
        deviceStatusDot.getStyleClass().removeAll(
                "device-status-dot-connected", "device-status-dot-disconnected");
        deviceStatusDot.getStyleClass().add(connected
                ? "device-status-dot-connected" : "device-status-dot-disconnected");

        // 更新状态文本
        deviceStatusLabel.setText(connected ? (deviceName != null ? deviceName : "已连接") : "未连接");

        // 更新连接按钮文本
        for (Node node : ((HBox) this.getChildren().get(1)).getChildren()) {
            if (node instanceof Button btn && "connectBtn".equals(btn.getId())) {
                btn.setText(connected ? "断开设备" : "连接设备");
                break;
            }
        }

        // 更新标题栏
        updateTitleText(currentMenu);
    }

    /**
     * 更新存储信息显示
     *
     * @param internalText 内置存储信息文本
     * @param sdText SD 卡存储信息文本
     */
    public void setStorageInfo(String internalText, String sdText) {
        internalStorageLabel.setText(internalText != null ? internalText : "—");
        sdStorageLabel.setText(sdText != null ? sdText : "—");
    }

    // ========================================================================
    // 视图访问器
    // ========================================================================

    public SongsView getSongsView() { return songsView; }
    public PlaylistsView getPlaylistsView() { return playlistsView; }
    public UploadView getUploadView() { return uploadView; }
    public SyncView getSyncView() { return syncView; }
    public TransmissionView getTransmissionView() { return transmissionView; }
    public DeviceView getDeviceView() { return deviceView; }
    public SettingsView getSettingsView() { return settingsView; }
    public AboutView getAboutView() { return aboutView; }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setWindowCallback(WindowCallback callback) {
        this.windowCallback = callback;
    }

    public void setMenuSwitchCallback(MenuSwitchCallback callback) {
        this.menuSwitchCallback = callback;
    }

    public void setDeviceConnectCallback(DeviceConnectCallback callback) {
        this.deviceConnectCallback = callback;
    }
}
