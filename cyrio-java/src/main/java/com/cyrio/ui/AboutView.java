package com.cyrio.ui;

import javafx.geometry.Pos;
import javafx.scene.control.Label;
import javafx.scene.layout.HBox;
import javafx.scene.layout.Priority;
import javafx.scene.layout.Region;
import javafx.scene.layout.VBox;

/**
 * 关于视图
 *
 * <p>显示应用名称、版本、技术栈和开源协议信息。
 *
 * <h3>布局结构</h3>
 * <ul>
 *   <li>应用名称（大号字体，rio-blue 色）</li>
 *   <li>版本号</li>
 *   <li>技术栈信息</li>
 *   <li>开源协议声明</li>
 * </ul>
 *
 * <p>纯展示页面，无交互操作。
 */
public class AboutView extends VBox {

    // ========================================================================
    // 构造
    // ========================================================================

    public AboutView() {
        this.getStyleClass().add("content-pane");
        this.setSpacing(16);
        this.setAlignment(Pos.CENTER);

        // 应用名称
        Label appName = new Label("Cyrio");
        appName.getStyleClass().add("about-title");

        // 副标题
        Label subtitle = new Label("Diamond Rio S-Series 管理工具");
        subtitle.getStyleClass().add("about-version");

        // 版本号
        Label version = new Label("Ver 0.1.0");
        version.getStyleClass().add("about-version");

        // 弹性间隔
        Region spacer1 = new Region();
        spacer1.setPrefHeight(10);

        // 技术栈
        VBox techStack = createTechStackSection();

        // 弹性间隔
        Region spacer2 = new Region();
        spacer2.setPrefHeight(10);

        // 开源协议
        Label license = new Label("开源软件，请勿商用");
        license.getStyleClass().add("about-info");

        // 项目说明
        Label description = new Label(
                "Cyrio 是一款跨平台的 Diamond Rio S-Series（S10/S30S/S35S/S50）\n" +
                "MP3 播放器管理工具，支持歌曲上传/下载/删除、歌单管理、\n" +
                "WebDAV 虚拟U盘、音频播放等功能。"
        );
        description.getStyleClass().add("about-info");

        // GitHub 链接（文本形式）
        Label repo = new Label("GitHub: hjelmn/rioutil (协议参考)");
        repo.getStyleClass().add("about-info");

        this.getChildren().addAll(
                appName,
                subtitle,
                version,
                spacer1,
                techStack,
                spacer2,
                description,
                license,
                repo
        );
    }

    // ========================================================================
    // 技术栈信息
    // ========================================================================

    /**
     * 创建技术栈信息区
     *
     * <p>以分组形式展示应用使用的核心技术。
     */
    private VBox createTechStackSection() {
        VBox section = new VBox();
        section.setSpacing(6);
        section.setAlignment(Pos.CENTER);

        Label title = new Label("技术栈");
        title.getStyleClass().add("settings-section-title");

        Label java = new Label("Java 17 + JavaFX 17 — 跨平台桌面 UI");
        java.getStyleClass().add("about-info");

        Label usb = new Label("usb4java (libusb) — USB 通信");
        usb.getStyleClass().add("about-info");

        Label webdav = new Label("JDK HttpServer — WebDAV 虚拟U盘");
        webdav.getStyleClass().add("about-info");

        Label audio = new Label("JavaFX MediaPlayer — 音频播放");
        audio.getStyleClass().add("about-info");

        Label protocol = new Label("Rio S-Series USB 协议 — 逆向自 rioutil");
        protocol.getStyleClass().add("about-info");

        section.getChildren().addAll(title, java, usb, webdav, audio, protocol);
        return section;
    }
}
