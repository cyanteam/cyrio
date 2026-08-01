package com.cyrio.core.model;

/**
 * 播放列表信息（用户面向数据模型）
 *
 * <p>由 Rust cyrio-core 通过 JNI 返回的 JSON 数据解析而来。
 * 播放列表内容使用 FIDL/ST10 二进制格式存储（Rust 端处理）。
 */
public class Playlist {

    /** 文件编号（设备内部的唯一编号，用于删除/添加歌曲/修复编码） */
    public int fileNo;

    /** 文件大小（字节，FIDL 二进制长度） */
    public long size;

    /** 歌单名（UTF-8） */
    public String name;

    /** 标题（UTF-8） */
    public String title;

    /** 所在内存单元（0=内置闪存, 1=SD 卡） */
    public byte memUnit;

    /**
     * 创建一个空的 {@code Playlist}
     */
    public Playlist() {
        this.fileNo = 0;
        this.size = 0;
        this.name = "";
        this.title = "";
        this.memUnit = 0;
    }

    @Override
    public String toString() {
        return "Playlist{"
                + "fileNo=" + fileNo
                + ", size=" + size
                + ", name='" + name + "'"
                + ", title='" + title + "'"
                + ", memUnit=" + memUnit
                + "}";
    }
}
