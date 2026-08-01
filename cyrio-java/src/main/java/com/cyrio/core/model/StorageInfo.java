package com.cyrio.core.model;

/**
 * 存储空间信息（用户面向数据模型）
 *
 * <p>由 Rust cyrio-core 通过 JNI 返回的 JSON 数据解析而来。
 * 描述一个内存单元（内置闪存或 SD 卡）的容量信息。
 * S-Series 中所有大小字段单位是字节，显示时 /1024/1024 得 MB。
 *
 * <p>若请求不存在的内存单元（如未插 SD 卡时查询单元 1），
 * Rust 端返回全 0，{@code isPresent} 为 {@code false}。
 */
public class StorageInfo {

    /** 总大小（字节） */
    public long totalSize;

    /** 已用字节数 */
    public long usedSize;

    /** 空闲字节数 */
    public long freeSize;

    /** 系统保留字节数 */
    public long systemSize;

    /** 内存单元名（如 "Internal Memory"） */
    public String name;

    /** 型号字符串 */
    public String model;

    /** 该内存单元是否存在（size > 0） */
    public boolean isPresent;

    /**
     * 创建一个空的 {@code StorageInfo}
     */
    public StorageInfo() {
        this.totalSize = 0;
        this.usedSize = 0;
        this.freeSize = 0;
        this.systemSize = 0;
        this.name = "";
        this.model = "";
        this.isPresent = false;
    }

    /**
     * 返回内存单元大小的 MB 字符串（人类可读）
     *
     * @return 如 "10.0MB used / 50.0MB free / 64.0MB total"
     */
    public String formatSize() {
        double totalMb = totalSize / (1024.0 * 1024.0);
        double freeMb = freeSize / (1024.0 * 1024.0);
        double usedMb = usedSize / (1024.0 * 1024.0);
        return String.format("%.1fMB used / %.1fMB free / %.1fMB total", usedMb, freeMb, totalMb);
    }

    @Override
    public String toString() {
        return "StorageInfo{"
                + "totalSize=" + totalSize
                + ", usedSize=" + usedSize
                + ", freeSize=" + freeSize
                + ", systemSize=" + systemSize
                + ", name='" + name + "'"
                + ", model='" + model + "'"
                + ", isPresent=" + isPresent
                + "}";
    }
}
