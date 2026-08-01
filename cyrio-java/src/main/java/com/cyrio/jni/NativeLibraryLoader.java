package com.cyrio.jni;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Locale;

/**
 * Native 动态库加载器
 *
 * <p>从 JAR 内嵌的 resources/native/ 目录中提取 Rust 编译的 cdylib，
 * 写入临时文件后通过 {@link System#load(String)} 加载。
 *
 * <h3>支持的平台</h3>
 * <ul>
 *   <li>macOS: lib{name}.dylib (aarch64 / x86_64)</li>
 *   <li>Linux: lib{name}.so (aarch64 / x86_64)</li>
 *   <li>Windows: {name}.dll (x86_64)</li>
 * </ul>
 *
 * <h3>资源路径规则</h3>
 * <p>JAR 内的资源路径为 {@code native/<os>/<arch>/<filename>}，例如：
 * <ul>
 *   <li>{@code native/macos/aarch64/libcyrio_jni.dylib}</li>
 *   <li>{@code native/linux/x86_64/libcyrio_jni.so}</li>
 *   <li>{@code native/windows/x86_64/cyrio_jni.dll}</li>
 * </ul>
 *
 * <p>若 JAR 内未找到对应资源，回退到 {@code java.library.path} 搜索（开发模式）。
 */
public final class NativeLibraryLoader {

    private NativeLibraryLoader() {
    }

    /**
     * 加载 native 库
     *
     * <p>优先从 JAR 资源提取，失败时回退到 {@link System#loadLibrary(String)}。
     *
     * @param name 库名（不含平台前缀和后缀，如 "cyrio_jni"）
     */
    public static void load(String name) {
        // 1. 尝试从 JAR 资源加载
        if (tryLoadFromResource(name)) {
            return;
        }

        // 2. 回退：从 java.library.path 搜索（开发模式）
        try {
            System.loadLibrary(name);
            return;
        } catch (UnsatisfiedLinkError e) {
            // 继续尝试其他方式
        }

        // 3. 最后尝试：从项目 target/release 目录加载（开发模式）
        if (tryLoadFromDevTarget(name)) {
            return;
        }

        throw new UnsatisfiedLinkError(
                "无法加载 native 库: " + name + "\n" +
                "请确保已编译 cyrio-jni (cargo build --release -p cyrio-jni)\n" +
                "或将动态库放入 java.library.path");
    }

    // ========================================================================
    // 从 JAR 资源加载
    // ========================================================================

    /**
     * 尝试从 JAR 内嵌资源加载 native 库
     *
     * @param name 库名
     * @return true 加载成功
     */
    private static boolean tryLoadFromResource(String name) {
        String resourcePath = getResourcePath(name);
        if (resourcePath == null) {
            return false;
        }

        InputStream in = NativeLibraryLoader.class.getResourceAsStream(resourcePath);
        if (in == null) {
            return false;
        }

        try {
            // 写入临时文件
            Path tempFile = createTempFile(name);
            try (InputStream is = in; OutputStream os = Files.newOutputStream(tempFile)) {
                is.transferTo(os);
            }

            // 设置可执行权限（Unix 系）
            try {
                tempFile.toFile().setExecutable(true, false);
            } catch (Exception ignored) {
            }

            // 加载
            System.load(tempFile.toAbsolutePath().toString());
            return true;

        } catch (IOException | UnsatisfiedLinkError e) {
            return false;
        }
    }

    // ========================================================================
    // 从开发目录加载
    // ========================================================================

    /**
     * 尝试从 Rust 项目 target/release 目录加载（仅开发模式）
     */
    private static boolean tryLoadFromDevTarget(String name) {
        String fileName = getNativeFileName(name);
        String rustTargetDir = findRustTargetDir();
        if (rustTargetDir == null) {
            return false;
        }

        Path libPath = Paths.get(rustTargetDir, fileName);
        if (!Files.exists(libPath)) {
            return false;
        }

        try {
            System.load(libPath.toAbsolutePath().toString());
            return true;
        } catch (UnsatisfiedLinkError e) {
            return false;
        }
    }

    /**
     * 查找 Rust 项目的 target/release 目录
     *
     * <p>搜索策略：
     * <ol>
     *   <li>当前工作目录/target/release</li>
     *   <li>上级目录/target/release（Maven 项目在 cyrio-java/ 子目录）</li>
     *   <li>用户目录/BACKFILE/project/rust/cyrio-rs/target/release</li>
     * </ol>
     */
    private static String findRustTargetDir() {
        String userDir = System.getProperty("user.dir");

        // 1. 当前目录/target/release
        Path p1 = Paths.get(userDir, "target", "release");
        if (Files.isDirectory(p1)) {
            return p1.toString();
        }

        // 2. 上级目录/target/release（Maven 项目）
        Path p2 = Paths.get(userDir, "..", "target", "release");
        if (Files.isDirectory(p2)) {
            return p2.toAbsolutePath().toString();
        }

        // 3. 已知项目路径
        Path p3 = Paths.get(System.getProperty("user.home"),
                "BACKFILE", "project", "rust", "cyrio-rs", "target", "release");
        if (Files.isDirectory(p3)) {
            return p3.toString();
        }

        return null;
    }

    // ========================================================================
    // 平台检测
    // ========================================================================

    /**
     * 获取 JAR 内 native 库的资源路径
     *
     * @param name 库名（如 "cyrio_jni"）
     * @return 资源路径（如 "/native/macos/aarch64/libcyrio_jni.dylib"），不支持返回 null
     */
    private static String getResourcePath(String name) {
        String os = getOsName();
        String arch = getArchName();
        String fileName = getNativeFileName(name);
        if (os == null || arch == null || fileName == null) {
            return null;
        }
        return "/native/" + os + "/" + arch + "/" + fileName;
    }

    /**
     * 获取 OS 名称（资源路径用）
     *
     * @return "macos" / "linux" / "windows"，未知返回 null
     */
    private static String getOsName() {
        String os = System.getProperty("os.name").toLowerCase(Locale.ENGLISH);
        if (os.contains("mac") || os.contains("darwin")) {
            return "macos";
        }
        if (os.contains("linux")) {
            return "linux";
        }
        if (os.contains("windows")) {
            return "windows";
        }
        return null;
    }

    /**
     * 获取 CPU 架构（资源路径用）
     *
     * @return "aarch64" / "x86_64"，未知返回 null
     */
    private static String getArchName() {
        String arch = System.getProperty("os.arch").toLowerCase(Locale.ENGLISH);
        if (arch.contains("aarch64") || arch.contains("arm64")) {
            return "aarch64";
        }
        if (arch.contains("amd64") || arch.contains("x86_64") || arch.contains("x64")) {
            return "x86_64";
        }
        if (arch.contains("i386") || arch.contains("x86")) {
            return "x86";
        }
        return null;
    }

    /**
     * 获取平台对应的动态库文件名
     *
     * @param name 库名（如 "cyrio_jni"）
     * @return 文件名（如 "libcyrio_jni.dylib" / "libcyrio_jni.so" / "cyrio_jni.dll"）
     */
    private static String getNativeFileName(String name) {
        String os = System.getProperty("os.name").toLowerCase(Locale.ENGLISH);
        if (os.contains("mac") || os.contains("darwin")) {
            return "lib" + name + ".dylib";
        }
        if (os.contains("linux")) {
            return "lib" + name + ".so";
        }
        if (os.contains("windows")) {
            return name + ".dll";
        }
        return null;
    }

    /**
     * 创建临时文件用于存放 native 库
     */
    private static Path createTempFile(String name) throws IOException {
        String fileName = getNativeFileName(name);
        Path tempDir = Paths.get(System.getProperty("java.io.tmpdir"), "cyrio-native");
        Files.createDirectories(tempDir);

        Path tempFile = tempDir.resolve(fileName + "." + ProcessHandle.current().pid());
        if (Files.exists(tempFile)) {
            Files.delete(tempFile);
        }

        return tempFile;
    }
}
