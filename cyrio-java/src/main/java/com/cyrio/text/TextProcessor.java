package com.cyrio.text;

import com.cyrio.jni.CyrioNative;

/**
 * 文本处理入口：通过 JNI 调用 Rust cyrio-text 实现。
 *
 * <p>核心逻辑（拼音转换、假名罗马字、噪音词去除）全部在 Rust 中实现，
 * Java 仅做转发。保证与 Rust 版本完全一致的文本处理结果。
 *
 * <h2>处理顺序</h2>
 * <p>先 strip（去噪），再 slug（转拼音 / 罗马字）。
 *
 * <h2>示例</h2>
 * <pre>
 * processTitle("【洛天依 原创】Hi-Res 赛马", false, true) == "赛马"
 * processTitle("【洛天依 原创】赛马",       true, false) == "【Luo-Tian-Yi Yuan-Chuang】 Sai-Ma"
 * processTitle("【洛天依 原创】Hi-Res 赛马", true, true) == "Sai-Ma"
 * </pre>
 */
public final class TextProcessor {

    private TextProcessor() {
    }

    /**
     * 处理标题文本（先 strip 再 slug）
     *
     * @param title      原始标题
     * @param applySlug  是否应用 slug 转换（中文转拼音、假名转罗马字）
     * @param applyStrip 是否应用噪音词去除
     * @return 处理后的标题
     */
    public static String processTitle(String title, boolean applySlug, boolean applyStrip) {
        if (title == null || title.isEmpty()) {
            return "";
        }
        return CyrioNative.processTitle(title, applySlug, applyStrip);
    }

    /**
     * Slug 转换（中文→拼音，日文→罗马字）
     *
     * @param text 输入文本
     * @return 转换后的字符串
     */
    public static String toSlug(String text) {
        if (text == null || text.isEmpty()) {
            return "";
        }
        return CyrioNative.toSlug(text, "-", true, false);
    }

    /**
     * 去除标题噪音词
     *
     * @param text 输入文本
     * @return 去噪后的文本
     */
    public static String stripNoise(String text) {
        if (text == null || text.isEmpty()) {
            return "";
        }
        return CyrioNative.stripNoise(text);
    }
}
