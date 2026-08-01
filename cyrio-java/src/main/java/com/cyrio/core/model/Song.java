package com.cyrio.core.model;

/**
 * 歌曲信息（用户面向数据模型）
 *
 * <p>由 Rust cyrio-core 通过 JNI 返回的 JSON 数据解析而来。
 * {@code bitRate} 已在 Rust 端从设备内部的 {@code kbps << 7} 格式转换为 kbps。
 *
 * <p>仅保留用户关心的元数据，不含设备协议细节。
 */
public class Song {

    /** 文件编号（设备内部的唯一编号，用于下载/删除/加入歌单） */
    public int fileNo;

    /** 文件大小（字节） */
    public long size;

    /** 时长（秒） */
    public int time;

    /** 比特率（kbps） */
    public int bitRate;

    /** 采样率（Hz，如 44100） */
    public int sampleRate;

    /** 文件名（UTF-8） */
    public String name;

    /** 标题（UTF-8） */
    public String title;

    /** 艺术家（UTF-8） */
    public String artist;

    /** 专辑（UTF-8） */
    public String album;

    /** 所在内存单元（0=内置闪存, 1=SD 卡） */
    public byte memUnit;

    /**
     * 创建一个空的 {@code Song}
     */
    public Song() {
        this.fileNo = 0;
        this.size = 0;
        this.time = 0;
        this.bitRate = 0;
        this.sampleRate = 0;
        this.name = "";
        this.title = "";
        this.artist = "";
        this.album = "";
        this.memUnit = 0;
    }

    @Override
    public String toString() {
        return "Song{"
                + "fileNo=" + fileNo
                + ", size=" + size
                + ", time=" + time
                + ", bitRate=" + bitRate
                + ", sampleRate=" + sampleRate
                + ", name='" + name + "'"
                + ", title='" + title + "'"
                + ", artist='" + artist + "'"
                + ", album='" + album + "'"
                + ", memUnit=" + memUnit
                + "}";
    }
}
