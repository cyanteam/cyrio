// ============================================================================
// 共享类型定义 — 现代/经典前端共用
// ============================================================================

export type MenuAction =
  | "songs"
  | "playlists"
  | "upload"
  | "device-info"
  | "sync"
  | "transmission"
  | "settings"
  | "about";

/** 应用设置 — 与 egui 版 AppSettings 字段保持一致 */
export type AppSettings = {
  /** 上传时是否应用 slug（中文转拼音） */
  upload_apply_slug: boolean;
  /** 上传时是否应用去词（移除无关词汇） */
  upload_apply_strip: boolean;
  /** 去除括号内容（含中英文括号） */
  strip_parentheses: boolean;
  /** 去除中文/英文引号包裹的歌词片段 */
  strip_quotes: boolean;
  /** 去除音质/分辨率标签：Hi-Res、无损、4K、高清、原创 等 */
  strip_quality_tags: boolean;
  /** 自定义停用词（每行一个） */
  custom_stop_words: string;
};

export const SETTINGS_KEY = "cyrio.settings";
export const DEFAULT_SETTINGS: AppSettings = {
  upload_apply_slug: false,
  upload_apply_strip: false,
  strip_parentheses: true,
  strip_quotes: true,
  strip_quality_tags: true,
  custom_stop_words: "",
};

/** 从 localStorage 加载设置（失败时返回默认值） */
export function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    const parsed = JSON.parse(raw);
    return { ...DEFAULT_SETTINGS, ...parsed };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

/** 把 AppSettings 转成上传时需要的 UploadTextOptions 对象（camelCase 字段名给 Tauri） */
export function settingsToTextOpts(s: AppSettings) {
  return {
    applySlug: s.upload_apply_slug,
    applyStrip: s.upload_apply_strip,
    stripParentheses: s.strip_parentheses,
    stripQuotes: s.strip_quotes,
    stripQualityTags: s.strip_quality_tags,
    customStopWords: s.custom_stop_words
      .split("\n")
      .map((w) => w.trim())
      .filter((w) => w.length > 0),
  };
}

export type SongInfo = {
  file_no: number;
  size: number;
  time: number;
  name: string;
  title: string;
  artist: string;
  album: string;
  bit_rate: number;
  mem_unit: number;
};

export type PlaylistInfo = {
  file_no: number;
  size: number;
  name: string;
  title: string;
  mem_unit: number;
};

export type StorageInfo = {
  mem_unit: number;
  present: boolean;
  size: number;
  used: number;
  free: number;
  size_formatted: string;
};

export type UsbDevice = {
  vid: string;
  pid: string;
  vid_num: number;
  pid_num: number;
  name: string;
  manufacturer: string;
  is_diamond: boolean;
};

export type BatchUploadResult = {
  path: string;
  success: boolean;
  file_no: number;
  error: string;
};

export type PlaybackState = {
  is_playing: boolean;
  position: number;
  duration: number;
  is_loading: boolean;
};

/** 批量操作预览结果（后端 PreviewResult 对应） */
export type PreviewResult = {
  file_no: number;
  mem_unit: number;
  original: string;
  new_title: string;
  changed: boolean;
};

/** 批量操作执行结果（后端 RenameResult 对应） */
export type RenameResult = {
  file_no: number;
  mem_unit: number;
  success: boolean;
  original: string;
  new_title: string;
  error: string;
};

/** 重命名进度事件 payload（rename-progress） */
export type RenameProgress = {
  current: number;
  total: number;
  current_title: string;
  phase: string;
};

/** WebDAV 服务器状态 */
export type WebDavStatus =
  | { type: "stopped" }
  | { type: "running"; addr: string }
  | { type: "error"; message: string };

/** 格式化时间 mm:ss */
export function formatTime(sec: number): string {
  if (!sec || sec < 0) return "0:00";
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** 格式化文件大小 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** 显示标题：title 为空时用 name 去掉路径前缀和扩展名 */
export function displayTitle(song: { title: string; name: string }): string {
  if (song.title) return song.title;
  return song.name
    .replace(/^D:\\/, "")
    .replace(/\.mp3$/i, "")
    .replace(/\.pls$/i, "");
}
