package com.cyrio;

/**
 * JavaFX 启动器
 *
 * <p>JavaFX 17+ 要求主类不能直接继承 {@link javafx.application.Application}，
 * 否则 JVM 会报 "缺少 JavaFX 运行时组件" 错误。
 * 此启动器作为独立入口，间接调用 {@link CyrioApp}。
 */
public class Launcher {

    /**
     * 应用程序入口
     *
     * @param args 命令行参数
     */
    public static void main(String[] args) {
        CyrioApp.main(args);
    }
}
