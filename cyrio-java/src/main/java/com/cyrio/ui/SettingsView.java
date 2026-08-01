package com.cyrio.ui;

import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.CheckBox;
import javafx.scene.control.ComboBox;
import javafx.scene.control.Label;
import javafx.scene.control.Slider;
import javafx.scene.control.TextField;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;
import javafx.util.StringConverter;

/**
 * 设置视图
 *
 * <p>应用全局设置，包括 WebDAV 服务器、文本处理、音频播放器等。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>WebDAV 设置区：开关 + 端口 + 挂载地址显示</li>
 *   <li>文本处理设置区：slug 分隔符 / 大小写 / 保留标点</li>
 *   <li>音频播放器设置区：音量滑块</li>
 *   <li>关于信息区：应用版本和开源协议</li>
 * </ul>
 *
 * <p>所有设置变更通过回调接口通知控制器层。
 */
public class SettingsView extends VBox {

    // ========================================================================
    // 回调接口
    // ========================================================================

    /** 设置变更回调 */
    public interface SettingsCallback {
        /** WebDAV 开关变更 */
        void onWebDavToggle(boolean enabled, int port);

        /** 音量变更 */
        void onVolumeChanged(double volume);

        /** Slug 设置变更 */
        void onSlugSettingsChanged(String separator, boolean toLowerCase, boolean keepPunctuation);
    }

    // ========================================================================
    // 字段
    // ========================================================================

    /** WebDAV 开关 */
    private final CheckBox webDavCheckBox = new CheckBox("启用 WebDAV 虚拟U盘");

    /** WebDAV 端口输入框 */
    private final TextField portField = new TextField("8765");

    /** WebDAV 状态标签 */
    private final Label webDavStatusLabel = new Label("未运行");

    /** WebDAV 挂载地址标签 */
    private final Label webDavUrlLabel = new Label("—");

    /** Slug 分隔符下拉框 */
    private final ComboBox<String> separatorCombo = new ComboBox<>();

    /** Slug 小写选项 */
    private final CheckBox lowerCaseCheckBox = new CheckBox("转换为小写");

    /** Slug 保留标点选项 */
    private final CheckBox keepPunctuationCheckBox = new CheckBox("保留标点符号");

    /** 音量滑块 */
    private final Slider volumeSlider = new Slider(0, 100, 100);

    /** 音量值标签 */
    private final Label volumeLabel = new Label("100%");

    /** 回调 */
    private SettingsCallback callback;

    // ========================================================================
    // 构造
    // ========================================================================

    public SettingsView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(12);

        // WebDAV 设置
        this.getChildren().add(createWebDavSection());

        // 文本处理设置
        this.getChildren().add(createTextProcessingSection());

        // 音频播放器设置
        this.getChildren().add(createAudioSection());

        // 关于信息
        this.getChildren().add(createAboutSection());
    }

    // ========================================================================
    // WebDAV 设置区
    // ========================================================================

    /**
     * 创建 WebDAV 设置区
     *
     * <p>包含：启用开关 + 端口输入 + 状态显示 + 挂载地址
     */
    private VBox createWebDavSection() {
        VBox section = new VBox();
        section.getStyleClass().add("settings-section");
        section.setSpacing(8);

        // 标题
        Label title = new Label("WebDAV 虚拟U盘");
        title.getStyleClass().add("settings-section-title");

        // 说明文本
        Label notice = new Label("将 Rio 设备虚拟成 WebDAV 网络驱动器，可通过 Finder/资源管理器挂载管理。");
        notice.getStyleClass().add("notice-label");
        notice.setWrapText(true);

        // 开关行
        HBox toggleRow = new HBox();
        toggleRow.getStyleClass().add("settings-row");

        webDavCheckBox.getStyleClass().add("settings-label");
        webDavCheckBox.setOnAction(e -> {
            boolean enabled = webDavCheckBox.isSelected();
            int port = parsePort(portField.getText());
            if (callback != null) {
                callback.onWebDavToggle(enabled, port);
            }
        });

        Label portLabel = new Label("端口:");
        portLabel.getStyleClass().add("settings-label");

        portField.setPrefWidth(80);
        portField.getStyleClass().add("search-field");
        portField.textProperty().addListener((obs, oldVal, newVal) -> {
            if (webDavCheckBox.isSelected()) {
                int port = parsePort(newVal);
                if (callback != null) {
                    callback.onWebDavToggle(true, port);
                }
            }
        });

        toggleRow.getChildren().addAll(webDavCheckBox, portLabel, portField);

        // 状态行
        HBox statusRow = new HBox();
        statusRow.getStyleClass().add("settings-row");

        Label statusTitle = new Label("状态:");
        statusTitle.getStyleClass().add("settings-label");
        statusTitle.setPrefWidth(80);

        webDavStatusLabel.getStyleClass().add("settings-value");

        statusRow.getChildren().addAll(statusTitle, webDavStatusLabel);

        // 挂载地址行
        HBox urlRow = new HBox();
        urlRow.getStyleClass().add("settings-row");

        Label urlTitle = new Label("挂载地址:");
        urlTitle.getStyleClass().add("settings-label");
        urlTitle.setPrefWidth(80);

        webDavUrlLabel.getStyleClass().add("settings-value");

        urlRow.getChildren().addAll(urlTitle, webDavUrlLabel);

        section.getChildren().addAll(title, notice, toggleRow, statusRow, urlRow);
        return section;
    }

    // ========================================================================
    // 文本处理设置区
    // ========================================================================

    /**
     * 创建文本处理设置区
     *
     * <p>包含：slug 分隔符 / 小写转换 / 保留标点
     */
    private VBox createTextProcessingSection() {
        VBox section = new VBox();
        section.getStyleClass().add("settings-section");
        section.setSpacing(8);

        // 标题
        Label title = new Label("Slug 文本处理");
        title.getStyleClass().add("settings-section-title");

        // 说明文本
        Label notice = new Label("上传时自动将中文标题转换为拼音/罗马字，改善设备显示。");
        notice.getStyleClass().add("notice-label");
        notice.setWrapText(true);

        // 分隔符行
        HBox sepRow = new HBox();
        sepRow.getStyleClass().add("settings-row");

        Label sepLabel = new Label("分隔符:");
        sepLabel.getStyleClass().add("settings-label");
        sepLabel.setPrefWidth(80);

        separatorCombo.getItems().addAll("连字符 (-)", "下划线 (_)", "空格", "无分隔");
        separatorCombo.getSelectionModel().selectFirst();
        separatorCombo.getStyleClass().add("search-field");
        separatorCombo.valueProperty().addListener((obs, oldVal, newVal) -> notifySlugChange());

        sepRow.getChildren().addAll(sepLabel, separatorCombo);

        // 选项行
        HBox optionsRow = new HBox();
        optionsRow.getStyleClass().add("settings-row");
        optionsRow.setSpacing(20);

        lowerCaseCheckBox.getStyleClass().add("settings-label");
        lowerCaseCheckBox.setSelected(true);
        lowerCaseCheckBox.setOnAction(e -> notifySlugChange());

        keepPunctuationCheckBox.getStyleClass().add("settings-label");
        keepPunctuationCheckBox.setOnAction(e -> notifySlugChange());

        optionsRow.getChildren().addAll(lowerCaseCheckBox, keepPunctuationCheckBox);

        section.getChildren().addAll(title, notice, sepRow, optionsRow);
        return section;
    }

    // ========================================================================
    // 音频播放器设置区
    // ========================================================================

    /**
     * 创建音频播放器设置区
     *
     * <p>包含：音量滑块
     */
    private VBox createAudioSection() {
        VBox section = new VBox();
        section.getStyleClass().add("settings-section");
        section.setSpacing(8);

        // 标题
        Label title = new Label("音频播放器");
        title.getStyleClass().add("settings-section-title");

        // 音量行
        HBox volRow = new HBox();
        volRow.getStyleClass().add("settings-row");

        Label volLabel = new Label("音量:");
        volLabel.getStyleClass().add("settings-label");
        volLabel.setPrefWidth(80);

        volumeSlider.getStyleClass().add("progress-bar");
        volumeSlider.setPrefWidth(250);
        volumeSlider.valueProperty().addListener((obs, oldVal, newVal) -> {
            int vol = newVal.intValue();
            volumeLabel.setText(vol + "%");
            if (callback != null) {
                callback.onVolumeChanged(vol / 100.0);
            }
        });

        // 弹性间隔
        Region spacer = new Region();
        HBox.setHgrow(spacer, Priority.ALWAYS);

        volRow.getChildren().addAll(volLabel, volumeSlider, spacer, volumeLabel);

        section.getChildren().addAll(title, volRow);
        return section;
    }

    // ========================================================================
    // 关于信息区
    // ========================================================================

    /**
     * 创建关于信息区
     *
     * <p>显示应用名称、版本、开源协议。
     */
    private VBox createAboutSection() {
        VBox section = new VBox();
        section.getStyleClass().add("settings-section");
        section.setSpacing(6);

        Label title = new Label("关于");
        title.getStyleClass().add("settings-section-title");

        Label appName = new Label("Cyrio — Diamond Rio S-Series 管理工具");
        appName.getStyleClass().add("settings-value");

        Label version = new Label("版本: 0.1.0");
        version.getStyleClass().add("settings-label");

        Label license = new Label("开源软件，请勿商用");
        license.getStyleClass().add("notice-label");

        section.getChildren().addAll(title, appName, version, license);
        return section;
    }

    // ========================================================================
    // 公共方法
    // ========================================================================

    /**
     * 更新 WebDAV 状态显示
     *
     * @param running 是否正在运行
     * @param url     访问 URL（如 http://127.0.0.1:8765）
     */
    public void setWebDavStatus(boolean running, String url) {
        webDavStatusLabel.setText(running ? "运行中" : "未运行");
        webDavUrlLabel.setText(running && url != null ? url : "—");
        webDavCheckBox.setSelected(running);
    }

    /**
     * 设置音量值
     *
     * @param volume 音量（0.0 ~ 1.0）
     */
    public void setVolume(double volume) {
        int vol = (int) Math.round(volume * 100);
        volumeSlider.setValue(vol);
        volumeLabel.setText(vol + "%");
    }

    // ========================================================================
    // 回调设置
    // ========================================================================

    public void setCallback(SettingsCallback callback) {
        this.callback = callback;
    }

    // ========================================================================
    // 工具方法
    // ========================================================================

    /**
     * 解析端口号
     */
    private static int parsePort(String text) {
        try {
            int port = Integer.parseInt(text.trim());
            if (port < 1 || port > 65535) {
                return 8765;
            }
            return port;
        } catch (NumberFormatException e) {
            return 8765;
        }
    }

    /**
     * 获取当前选中的分隔符
     */
    private String getSeparator() {
        String selected = separatorCombo.getValue();
        if (selected == null) return "-";
        return switch (selected) {
            case "连字符 (-)" -> "-";
            case "下划线 (_)" -> "_";
            case "空格" -> " ";
            case "无分隔" -> "";
            default -> "-";
        };
    }

    /**
     * 通知 Slug 设置变更
     */
    private void notifySlugChange() {
        if (callback != null) {
            callback.onSlugSettingsChanged(
                    getSeparator(),
                    lowerCaseCheckBox.isSelected(),
                    keepPunctuationCheckBox.isSelected());
        }
    }
}
