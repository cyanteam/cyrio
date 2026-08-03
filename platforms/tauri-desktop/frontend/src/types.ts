// ============================================================================
// 共享类型定义（供 CyrioLauncher 和 demoData 使用）
// ============================================================================

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
  free_formatted?: string;
  used_formatted?: string;
  usage_percent?: number;
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
