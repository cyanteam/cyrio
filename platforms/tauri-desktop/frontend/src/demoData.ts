// ============================================================================
// 演示模式：模拟设备数据（无需连接真实 Rio 设备）
// ============================================================================

import type { SongInfo, PlaylistInfo, StorageInfo } from "./types";

/** 模拟内置存储歌曲列表 */
const DEMO_SONGS_INTERNAL: SongInfo[] = [
  { file_no: 1, size: 4232103, time: 178, name: "D:\\track001.mp3", title: "夜空中最亮的星", artist: "逃跑计划", album: "世界", bit_rate: 192000, mem_unit: 0 },
  { file_no: 2, size: 3856044, time: 162, name: "D:\\track002.mp3", title: "海阔天空", artist: "Beyond", album: "乐与怒", bit_rate: 192000, mem_unit: 0 },
  { file_no: 3, size: 5120980, time: 215, name: "D:\\track003.mp3", title: "光辉岁月", artist: "Beyond", album: "命运派对", bit_rate: 192000, mem_unit: 0 },
  { file_no: 4, size: 4456821, time: 187, name: "D:\\track004.mp3", title: "平凡之路", artist: "朴树", album: "猎户星座", bit_rate: 192000, mem_unit: 0 },
  { file_no: 5, size: 3998742, time: 168, name: "D:\\track005.mp3", title: "那些年", artist: "胡夏", album: "那些年，我们一起追的女孩", bit_rate: 192000, mem_unit: 0 },
  { file_no: 6, size: 4681230, time: 196, name: "D:\\track006.mp3", title: "稻香", artist: "周杰伦", album: "魔杰座", bit_rate: 192000, mem_unit: 0 },
  { file_no: 7, size: 3920561, time: 165, name: "D:\\track007.mp3", title: "晴天", artist: "周杰伦", album: "叶惠美", bit_rate: 192000, mem_unit: 0 },
  { file_no: 8, size: 4780923, time: 201, name: "D:\\track008.mp3", title: "七里香", artist: "周杰伦", album: "七里香", bit_rate: 192000, mem_unit: 0 },
  { file_no: 9, size: 3560214, time: 149, name: "D:\\track009.mp3", title: "倔强", artist: "五月天", album: "神的孩子都在跳舞", bit_rate: 192000, mem_unit: 0 },
  { file_no: 10, size: 4102456, time: 173, name: "D:\\track010.mp3", title: "知足", artist: "五月天", album: "知足", bit_rate: 192000, mem_unit: 0 },
  { file_no: 11, size: 4489102, time: 189, name: "D:\\track011.mp3", title: "突然好想你", artist: "五月天", album: "后青春期的诗", bit_rate: 192000, mem_unit: 0 },
  { file_no: 12, size: 3876550, time: 163, name: "D:\\track012.mp3", title: "蓝莲花", artist: "许巍", album: "时光·漫步", bit_rate: 192000, mem_unit: 0 },
  { file_no: 13, size: 4210387, time: 177, name: "D:\\track013.mp3", title: "曾经的你", artist: "许巍", album: "每一刻都是崭新的", bit_rate: 192000, mem_unit: 0 },
  { file_no: 14, size: 3998201, time: 168, name: "D:\\track014.mp3", title: "故乡", artist: "许巍", album: "那一年", bit_rate: 192000, mem_unit: 0 },
  { file_no: 15, size: 5120192, time: 215, name: "D:\\track015.mp3", title: "成都", artist: "赵雷", album: "无法长大", bit_rate: 192000, mem_unit: 0 },
  { file_no: 16, size: 3550102, time: 149, name: "D:\\track016.mp3", title: "理想三旬", artist: "陈鸿宇", album: "一如年少模样", bit_rate: 192000, mem_unit: 0 },
  { file_no: 17, size: 4230567, time: 178, name: "D:\\track017.mp3", title: "南山南", artist: "马頔", album: "孤岛", bit_rate: 192000, mem_unit: 0 },
  { file_no: 18, size: 3880923, time: 163, name: "D:\\track018.mp3", title: "董小姐", artist: "宋冬野", album: "安和桥北", bit_rate: 192000, mem_unit: 0 },
  { file_no: 19, size: 4450789, time: 187, name: "D:\\track019.mp3", title: "斑马斑马", artist: "宋冬野", album: "安和桥北", bit_rate: 192000, mem_unit: 0 },
  { file_no: 20, size: 3920156, time: 165, name: "D:\\track020.mp3", title: "莉莉安", artist: "宋冬野", album: "安和桥北", bit_rate: 192000, mem_unit: 0 },
];

/** 模拟 SD 卡歌曲列表 */
const DEMO_SONGS_SD: SongInfo[] = [
  { file_no: 1, size: 4560234, time: 192, name: "D:\\track101.mp3", title: "Bohemian Rhapsody", artist: "Queen", album: "A Night at the Opera", bit_rate: 320000, mem_unit: 1 },
  { file_no: 2, size: 3890512, time: 163, name: "D:\\track102.mp3", title: "Hotel California", artist: "Eagles", album: "Hotel California", bit_rate: 320000, mem_unit: 1 },
  { file_no: 3, size: 4120890, time: 173, name: "D:\\track103.mp3", title: "Stairway to Heaven", artist: "Led Zeppelin", album: "Led Zeppelin IV", bit_rate: 320000, mem_unit: 1 },
  { file_no: 4, size: 5230145, time: 219, name: "D:\\track104.mp3", title: "Imagine", artist: "John Lennon", album: "Imagine", bit_rate: 320000, mem_unit: 1 },
  { file_no: 5, size: 3980567, time: 167, name: "D:\\track105.mp3", title: "Let It Be", artist: "The Beatles", album: "Let It Be", bit_rate: 320000, mem_unit: 1 },
  { file_no: 6, size: 4450123, time: 187, name: "D:\\track106.mp3", title: "Yesterday", artist: "The Beatles", album: "Help!", bit_rate: 320000, mem_unit: 1 },
  { file_no: 7, size: 4670890, time: 196, name: "D:\\track107.mp3", title: "Hey Jude", artist: "The Beatles", album: "Hey Jude", bit_rate: 320000, mem_unit: 1 },
  { file_no: 8, size: 3560456, time: 149, name: "D:\\track108.mp3", title: "Wonderwall", artist: "Oasis", album: "(What's the Story) Morning Glory?", bit_rate: 192000, mem_unit: 1 },
  { file_no: 9, size: 4100789, time: 172, name: "D:\\track109.mp3", title: "Creep", artist: "Radiohead", album: "Pablo Honey", bit_rate: 192000, mem_unit: 1 },
  { file_no: 10, size: 4890234, time: 205, name: "D:\\track110.mp3", title: "Smells Like Teen Spirit", artist: "Nirvana", album: "Nevermind", bit_rate: 192000, mem_unit: 1 },
];

/** 模拟歌单列表 */
const DEMO_PLAYLISTS_INTERNAL: PlaylistInfo[] = [
  { file_no: 1, size: 42321030, name: "D:\\list001.pls", title: "华语经典", mem_unit: 0 },
  { file_no: 2, size: 38560440, name: "D:\\list002.pls", title: "民谣合集", mem_unit: 0 },
];

const DEMO_PLAYLISTS_SD: PlaylistInfo[] = [
  { file_no: 1, size: 45602340, name: "D:\\list101.pls", title: "欧美摇滚", mem_unit: 1 },
];

/** 模拟存储信息 */
const DEMO_STORAGE_INTERNAL: StorageInfo = {
  mem_unit: 0,
  present: true,
  size: 67108864, // 64MB
  used: 42321032,
  free: 24787832,
  size_formatted: "64.0 MB",
  free_formatted: "23.6 MB",
  used_formatted: "40.4 MB",
  usage_percent: 63.1,
};

const DEMO_STORAGE_SD: StorageInfo = {
  mem_unit: 1,
  present: true,
  size: 536870912, // 512MB
  used: 42805678,
  free: 494065234,
  size_formatted: "512.0 MB",
  free_formatted: "471.2 MB",
  used_formatted: "40.8 MB",
  usage_percent: 8.0,
};

/** 模拟歌单内歌曲（返回所有歌曲中的一部分） */
function getDemoPlaylistSongs(memUnit: number): SongInfo[] {
  if (memUnit === 0) return DEMO_SONGS_INTERNAL.slice(0, 5);
  return DEMO_SONGS_SD.slice(0, 3);
}

/**
 * 演示模式下的 invoke 拦截器。
 * 匹配命令名返回模拟数据，不匹配则返回空值。
 */
export async function demoInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    // ---- 数据查询类（返回模拟数据） ----
    case "list_songs": {
      const memUnit = (args?.memUnit as number) ?? 0;
      return (memUnit === 0 ? DEMO_SONGS_INTERNAL : DEMO_SONGS_SD) as unknown as T;
    }
    case "list_playlists": {
      const memUnit = (args?.memUnit as number) ?? 0;
      return (memUnit === 0 ? DEMO_PLAYLISTS_INTERNAL : DEMO_PLAYLISTS_SD) as unknown as T;
    }
    case "list_playlist_songs": {
      const memUnit = (args?.memUnit as number) ?? 0;
      return getDemoPlaylistSongs(memUnit) as unknown as T;
    }
    case "get_storage": {
      const memUnit = (args?.memUnit as number) ?? 0;
      return (memUnit === 0 ? DEMO_STORAGE_INTERNAL : DEMO_STORAGE_SD) as unknown as T;
    }
    case "is_connected":
      return true as unknown as T;
    case "list_usb_devices":
      return [] as unknown as T;
    case "get_webdav_status":
      return { type: "stopped" } as unknown as T;
    case "get_playback_state":
      return { is_playing: false, position: 0, duration: 0, is_loading: false } as unknown as T;
    case "get_song_detail": {
      // 根据 fileNo 和 memUnit 找到对应歌曲
      const fileNo = (args?.fileNo as number) ?? 1;
      const memUnit = (args?.memUnit as number) ?? 0;
      const pool = memUnit === 0 ? DEMO_SONGS_INTERNAL : DEMO_SONGS_SD;
      const song = pool.find((s) => s.file_no === fileNo) ?? pool[0];
      return {
        basic: song,
        technical: {
          duration: song.time,
          sample_rate: 44100,
          bit_rate: song.bit_rate,
          layer: 3,
          channels: 2,
        },
        id3: {
          title: song.title,
          artist: song.artist,
          album: song.album,
          year: "",
          genre: "",
          track: String(song.file_no),
          composer: "",
        },
        cover_art: null,
        mod_date: 0,
      } as unknown as T;
    }
    case "expand_paths":
      return [] as unknown as T;
    case "list_sync_rules":
      return [] as unknown as T;
    case "run_sync":
      return { added: [], deleted: [], skipped: [], errors: [] } as unknown as T;

    // ---- 预览类（返回空数组） ----
    case "preview_slug":
    case "preview_strip":
    case "preview_repair_encoding":
      return [] as unknown as T;

    // ---- 批量操作类（返回空数组） ----
    case "batch_slug_songs":
    case "batch_strip_songs":
    case "batch_slug_all_songs":
    case "batch_strip_all_songs":
    case "repair_all_songs_encoding":
    case "repair_selected_encoding":
    case "upload_song_batch":
      return [] as unknown as T;

    // ---- 操作类（返回 undefined，不影响演示） ----
    case "open_device_force":
    case "open_device":
    case "close_device":
    case "delete_song":
    case "delete_playlist":
    case "upload_file":
    case "rename_song":
    case "slug_song":
    case "strip_song":
    case "repair_encoding":
    case "repair_song_encoding":
    case "download_song":
    case "play_song":
    case "pause_audio":
    case "resume_audio":
    case "stop_audio":
    case "add_song_to_playlist":
    case "create_playlist":
    case "update_tray_tooltip":
    case "toggle_webdav":
    case "start_webdav":
    case "stop_webdav":
    case "mount_webdav":
    case "add_sync_rule":
    case "delete_sync_rule":
      return undefined as unknown as T;

    default:
      // 未实现的命令返回空值，避免崩溃
      return undefined as unknown as T;
  }
}

/** 是否处于演示模式（全局标志，由 CyrioLauncher 设置） */
let _demoMode = false;

export function setDemoMode(enabled: boolean) {
  _demoMode = enabled;
}

export function isDemoMode(): boolean {
  return _demoMode;
}
