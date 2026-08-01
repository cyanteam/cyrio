package com.cyrio;

import javafx.application.Application;
import javafx.application.Platform;
import javafx.scene.Scene;
import javafx.scene.layout.BorderPane;
import javafx.scene.paint.Color;
import javafx.stage.Stage;
import javafx.stage.StageStyle;

import com.cyrio.core.CyrioController;
import com.cyrio.ui.MainWindow;

/**
 * Cyrio JavaFX 应用主入口
 *
 * <p>负责创建主窗口、加载样式表、初始化控制器。
 * 核心设备操作通过 JNI 调用 Rust cyrio-core 实现。
 *
 * <p>主窗口大小 1024x720，最小 640x480，无系统装饰（UNDECORATED）。
 */
public class CyrioApp extends Application {

    /** 应用版本号 */
    public static final String VERSION = "0.1.0";

    /** 应用名称 */
    public static final String APP_NAME = "Cyrio";

    /** 默认窗口宽度 */
    private static final double DEFAULT_WIDTH = 1024;

    /** 默认窗口高度 */
    private static final double DEFAULT_HEIGHT = 720;

    /** 最小窗口宽度 */
    private static final double MIN_WIDTH = 640;

    /** 最小窗口高度 */
    private static final double MIN_HEIGHT = 480;

    /** 控制器（UI ↔ Device 桥接） */
    private CyrioController controller;

    @Override
    public void start(Stage primaryStage) {
        // 创建主窗口组件
        MainWindow mainWindow = new MainWindow(primaryStage);

        // 绑定窗口控制回调（关闭/最小化/最大化）
        // 必须在控制器创建之前设置，确保按钮可用
        mainWindow.setWindowCallback(new MainWindow.WindowCallback() {
            @Override
            public void onMinimize() {
                primaryStage.setIconified(true);
            }

            @Override
            public void onMaximizeToggle() {
                primaryStage.setMaximized(!primaryStage.isMaximized());
            }

            @Override
            public void onClose() {
                if (controller != null) {
                    controller.shutdown();
                }
                primaryStage.close();
                Platform.exit();
            }
        });

        // 创建控制器并绑定回调
        controller = new CyrioController(mainWindow);

        // 使用 BorderPane 作为根容器
        BorderPane root = new BorderPane();
        root.setCenter(mainWindow);

        // 创建场景并加载样式表
        Scene scene = new Scene(root, DEFAULT_WIDTH, DEFAULT_HEIGHT);
        scene.setFill(Color.TRANSPARENT);
        scene.getStylesheets().add(
                getClass().getResource("/styles.css").toExternalForm());

        // 窗口无系统装饰（自定义标题栏）
        primaryStage.initStyle(StageStyle.UNDECORATED);
        primaryStage.setTitle(APP_NAME + " Ver " + VERSION);
        primaryStage.setScene(scene);
        primaryStage.setMinWidth(MIN_WIDTH);
        primaryStage.setMinHeight(MIN_HEIGHT);

        // 窗口关闭时清理资源
        primaryStage.setOnCloseRequest(e -> {
            if (controller != null) {
                controller.shutdown();
            }
        });

        // 显示窗口
        primaryStage.show();
    }

    /**
     * 应用程序入口
     *
     * @param args 命令行参数
     */
    public static void main(String[] args) {
        launch(args);
    }
}
