import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DeviceSvg, type DeviceModel } from "./DeviceSvg";

// ============================================================================
// 移动端检测 — Android/iOS 时切换为移动布局
// ============================================================================

/** 检测当前是否运行在移动端（Android/iOS） */
function useIsMobile(): boolean {
  const [isMobile, setIsMobile] = useState(false);
  useEffect(() => {
    // Tauri 2.0: 通过 user agent 检测 Android
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes("android") || ua.includes("iphone") || ua.includes("ipad")) {
      setIsMobile(true);
    }
  }, []);
  return isMobile;
}

// ============================================================================
// 类型定义
// ============================================================================

type MenuAction =
  | "songs"
  | "playlists"
  | "upload"
  | "device-info"
  | "sync"
  | "transmission"
  | "settings"
  | "about";

// ============================================================================
// 应用设置（localStorage 持久化）
// ============================================================================

/** 应用设置 — 与 egui 版 AppSettings 字段保持一致 */
type AppSettings = {
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

const SETTINGS_KEY = "cyrio.settings";
const DEFAULT_SETTINGS: AppSettings = {
  upload_apply_slug: false,
  upload_apply_strip: false,
  strip_parentheses: true,
  strip_quotes: true,
  strip_quality_tags: true,
  custom_stop_words: "",
};

/** 从 localStorage 加载设置（失败时返回默认值） */
function loadSettings(): AppSettings {
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
function settingsToTextOpts(s: AppSettings): {
  applySlug: boolean;
  applyStrip: boolean;
  stripParentheses: boolean;
  stripQuotes: boolean;
  stripQualityTags: boolean;
  customStopWords: string[];
} {
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

type SongInfo = {
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

type PlaylistInfo = {
  file_no: number;
  size: number;
  name: string;
  title: string;
  mem_unit: number;
};

type StorageInfo = {
  mem_unit: number;
  present: boolean;
  size: number;
  used: number;
  free: number;
  size_formatted: string;
};

type UsbDevice = {
  vid: string;
  pid: string;
  vid_num: number;
  pid_num: number;
  name: string;
  manufacturer: string;
  is_diamond: boolean;
};

type BatchUploadResult = {
  path: string;
  success: boolean;
  file_no: number;
  error: string;
};

type PlaybackState = {
  is_playing: boolean;
  position: number;
  duration: number;
  is_loading: boolean;
};

/** 批量操作预览结果（后端 PreviewResult 对应） */
type PreviewResult = {
  file_no: number;
  mem_unit: number;
  original: string;
  new_title: string;
  changed: boolean;
};

/** 批量操作执行结果（后端 RenameResult 对应） */
type RenameResult = {
  file_no: number;
  mem_unit: number;
  success: boolean;
  original: string;
  new_title: string;
  error: string;
};

/** 重命名进度事件 payload（rename-progress） */
type RenameProgress = {
  current: number;
  total: number;
  current_title: string;
  phase: string;
};

/** 批量操作对话框状态 */
type BatchPreviewState = {
  /** 操作阶段：preview（预览中）| running（执行中）| done（完成） */
  phase: "preview" | "running" | "done";
  /** 操作类型标题 */
  title: string;
  /** 预览结果列表 */
  previews: PreviewResult[];
  /** 执行进度（phase=running 时有效） */
  progress: { current: number; total: number; currentTitle: string } | null;
  /** 执行结果（phase=done 时有效） */
  results: { success: number; failed: number; skipped: number } | null;
  /** 错误信息 */
  error: string | null;
} | null;

type SongDetail = {
  basic: SongInfo;
  technical: {
    duration: number;
    sample_rate: number;
    bit_rate: number;
    layer: number;
    channels: number;
  } | null;
  id3: {
    title: string;
    artist: string;
    album: string;
    year: string;
    genre: string;
    track: string;
    composer: string;
  };
  cover_art: number[] | null;
  mod_date: number;
};

type SyncRule = {
  id: string;
  local_path: string;
  mem_unit: number;
  playlist_file_no: number | null;
  last_sync_at: number | null;
};

type SyncResult = {
  added: string[];
  deleted: string[];
  skipped: string[];
  errors: string[];
};

type WebDavStatus =
  | { type: "stopped" }
  | { type: "running"; addr: string }
  | { type: "error"; message: string };

type ContextMenuState = {
  x: number;
  y: number;
  song: SongInfo;
} | null;

type ConfirmState = {
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
  onConfirm: () => void;
} | null;

/** 单个上传文件项 */
type UploadFileItem = {
  name: string;
  transferred: number;
  total: number;
  status: "pending" | "uploading" | "done" | "failed";
  error?: string;
};

const MENU_ITEMS: { action: MenuAction; label: string }[] = [
  { action: "songs", label: "歌曲" },
  { action: "playlists", label: "歌单" },
  { action: "upload", label: "上传" },
  { action: "sync", label: "同步" },
  { action: "device-info", label: "设备" },
  { action: "transmission", label: "传输" },
  { action: "settings", label: "设置" },
  { action: "about", label: "关于" },
];

// ============================================================================
// 移动端导航图标 — Material 风格 SVG（24x24, stroke 当前色）
// ============================================================================

/** 菜单项对应的 SVG 图标路径 */
const MENU_ICONS: Record<MenuAction, JSX.Element> = {
  songs: (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <path d="M9 18V5l12-2v13" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
      <circle cx="6" cy="18" r="3" stroke="currentColor" strokeWidth="2"/>
      <circle cx="18" cy="16" r="3" stroke="currentColor" strokeWidth="2"/>
    </svg>
  ),
  playlists: (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <path d="M3 6h13M3 12h13M3 18h9" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
      <path d="M19 12v6M19 12l3 3M19 12l-3 3" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  ),
  upload: (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
      <path d="M17 8l-5-5-5 5M12 3v12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  ),
  sync: (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <path d="M21 2v6h-6M3 12a9 9 0 0115-6.7L21 8M3 22v-6h6M21 12a9 9 0 01-15 6.7L3 16" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  ),
  "device-info": (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <rect x="5" y="2" width="14" height="20" rx="2" stroke="currentColor" strokeWidth="2"/>
      <path d="M12 18h.01" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    </svg>
  ),
  transmission: (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <path d="M12 3v14M12 3l-4 4M12 3l4 4M5 17v2a2 2 0 002 2h10a2 2 0 002-2v-2" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  ),
  settings: (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <circle cx="12" cy="12" r="3" stroke="currentColor" strokeWidth="2"/>
      <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 11-4 0v-.09a1.65 1.65 0 00-1-1.51 1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 11-2.83-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 110-4h.09a1.65 1.65 0 001.51-1 1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 112.83-2.83l.06.06a1.65 1.65 0 001.82.33h0a1.65 1.65 0 001-1.51V3a2 2 0 114 0v.09a1.65 1.65 0 001 1.51h0a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 112.83 2.83l-.06.06a1.65 1.65 0 00-.33 1.82v0a1.65 1.65 0 001.51 1H21a2 2 0 110 4h-.09a1.65 1.65 0 00-1.51 1z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>
  ),
  about: (
    <svg viewBox="0 0 24 24" fill="none" width="24" height="24">
      <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2"/>
      <path d="M12 16v-4M12 8h.01" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    </svg>
  ),
};

const AUTO_SCAN_INTERVAL_MS = 8000;

/** 字节数格式化（如 1.2 MB / 5.0 MB） */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Phase 5b: hash 路由 hook
 * 双向同步 `window.location.hash` ↔ route state
 * - hash 格式：`#/songs`、`#/playlists` 等
 * - `navigate(null)` 清除 hash（回到空白页）
 * - 浏览器前进/后退自动同步
 */
function useHashRoute() {
  const parseHash = (): string | null => {
    const hash = window.location.hash;
    const match = hash.match(/^#\/?([\w-]+)/);
    return match ? match[1] : null;
  };

  const [route, setRoute] = useState<string | null>(parseHash);

  useEffect(() => {
    const onHashChange = () => setRoute(parseHash());
    window.addEventListener('hashchange', onHashChange);
    return () => window.removeEventListener('hashchange', onHashChange);
  }, []);

  const navigate = useCallback((path: string | null) => {
    if (path === null) {
      history.replaceState(null, '', window.location.pathname + window.location.search);
      setRoute(null);
    } else {
      window.location.hash = `/${path}`;
    }
  }, []);

  return { route, navigate };
}

/** 检查 route 是否为有效的 MenuAction */
function isValidRoute(route: string | null): route is MenuAction {
  if (!route) return false;
  return MENU_ITEMS.some((item) => item.action === route);
}

// ============================================================================
// 自定义标题栏（Win8 风格 — 透明 + 三按钮）
// ============================================================================

/** 自定义标题栏
 * - 格式：[设备型号] [正在传输] 页面名 Cyrio Ver 版本 开源软件，请勿商用
 * - 反引号包裹的部分（设备型号、正在传输）：仅当对应条件成立时显示，否则完全省略（不留空括号）
 * - 星号包裹的部分（页面名、版本）：必须显示且为真实值
 * - 关闭按钮：若传输进行中，拦截 close 事件弹出危险提示框 */
function TitleBar({
  deviceLabel,
  transmitting,
  pageLabel,
}: {
  deviceLabel: string | null;
  transmitting: boolean;
  pageLabel: string;
}) {
  const [maximized, setMaximized] = useState(false);
  const [version, setVersion] = useState<string>("");
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    // 初始化时检查最大化状态
    appWindow.isMaximized().then(setMaximized).catch(() => {});
    // 监听窗口大小变化，更新最大化按钮图标
    const unlistenPromise = appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized).catch(() => {});
    });
    // 拦截窗口关闭事件：传输中时弹危险确认框
    const unlistenClosePromise = appWindow.onCloseRequested(async (event) => {
      if (transmitting) {
        event.preventDefault();
        setShowCloseConfirm(true);
      }
    });
    // 动态获取版本号（从 Tauri 配置读取）
    import("@tauri-apps/api/app").then(({ getVersion }) => {
      getVersion().then(setVersion).catch(() => setVersion(""));
    });
    return () => {
      unlistenPromise.then(fn => fn());
      unlistenClosePromise.then(fn => fn());
    };
  }, [transmitting]);

  const onMinimize = () => appWindow.minimize();
  const onToggleMaximize = () => appWindow.toggleMaximize();
  // 确认关闭后强制销毁窗口（绕过 onCloseRequested 拦截）
  const onForceClose = async () => {
    setShowCloseConfirm(false);
    try {
      await appWindow.destroy();
    } catch {
      // 降级：直接 close
      appWindow.close().catch(() => {});
    }
  };

  // 拼接标题：[Rio S50] [正在传输] 首页 Cyrio Ver 0.1.0 开源软件，请勿商用
  const parts: string[] = [];
  if (deviceLabel) parts.push(`[${deviceLabel}]`);
  if (transmitting) parts.push("[正在传输]");
  parts.push(pageLabel);
  parts.push("Cyrio");
  if (version) parts.push(`Ver ${version}`);
  parts.push("开源软件，请勿商用");
  const titleText = parts.join(" ");

  return (
    <>
      <div className="titlebar" data-tauri-drag-region>
        <div className="titlebar-title" data-tauri-drag-region>{titleText}</div>
        <div className="titlebar-buttons">
          {/* 最小化 */}
          <button
            className="titlebar-btn minimize"
            onClick={onMinimize}
            title="最小化"
            aria-label="最小化"
          >
            <svg className="caption-glyph" width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M1 5 H9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="square" />
            </svg>
          </button>
          {/* 最大化 / 还原 */}
          <button
            className="titlebar-btn maximize"
            onClick={onToggleMaximize}
            title={maximized ? "向下还原" : "最大化"}
            aria-label={maximized ? "向下还原" : "最大化"}
          >
            {maximized ? (
              <svg className="caption-glyph" width="10" height="10" viewBox="0 0 10 10" fill="none">
                <path d="M2.5 1 H9 V7.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="square" strokeLinejoin="miter" fill="none" />
                <path d="M1 2.5 H7.5 V9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="square" strokeLinejoin="miter" fill="none" />
              </svg>
            ) : (
              <svg className="caption-glyph" width="10" height="10" viewBox="0 0 10 10" fill="none">
                <path d="M1 1 H9 V9 H1 Z" stroke="currentColor" strokeWidth="1.2" strokeLinecap="square" strokeLinejoin="miter" fill="none" />
              </svg>
            )}
          </button>
          {/* 关闭 */}
          <button
            className="titlebar-btn close"
            onClick={() => {
              if (transmitting) setShowCloseConfirm(true);
              else appWindow.close();
            }}
            title="关闭"
            aria-label="关闭"
          >
            <svg className="caption-glyph" width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M1 1L9 9M9 1L1 9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="square" />
            </svg>
          </button>
        </div>
      </div>

      {/* 关闭确认弹窗 — 传输中时显示危险提示 */}
        {showCloseConfirm && (
          <div
            className="modal-backdrop"
          >
            <div
              className="modal danger-modal"
            >
              <div className="modal-header danger-header">
                <h3>危险操作</h3>
                <button className="modal-close" onClick={() => setShowCloseConfirm(false)}>×</button>
              </div>
              <div className="modal-body">
                <p className="danger-text">
                  正在传输文件，此时关闭软件可能导致：
                </p>
                <ul className="danger-list">
                  <li>设备上的歌曲数据损坏</li>
                  <li>传输中断，文件不完整</li>
                  <li>设备需重新插拔才能恢复</li>
                </ul>
                <p className="danger-text">确定要强制关闭吗？</p>
              </div>
              <div className="modal-footer">
                <button
                  className="modal-btn"
                  onClick={() => setShowCloseConfirm(false)}
                >
                  继续传输
                </button>
                <button
                  className="modal-btn danger"
                  onClick={onForceClose}
                >
                  强制关闭
                </button>
              </div>
            </div>
          </div>
        )}
    </>
  );
}

// ============================================================================
// 主组件
// ============================================================================

export default function CyrioLauncher() {
  const [connected, setConnected] = useState(false);
  const [connecting, setConnecting] = useState(false);
  // Phase 5b: hash 路由 — URL hash 双向同步
  const { route, navigate } = useHashRoute();
  // 从 hash route 派生 activeAction（仅当 route 是有效 MenuAction 时）
  const activeAction: MenuAction | null = isValidRoute(route) ? route : null;
  // 移动端检测：Android/iOS 时切换为移动布局（底部选项卡、无标题栏）
  const isMobile = useIsMobile();
  const [deviceModel, setDeviceModel] = useState<DeviceModel>("s-series");
  const [storage, setStorage] = useState<{ internal: StorageInfo | null; sd: StorageInfo | null }>({
    internal: null,
    sd: null,
  });
  const [notice, setNotice] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [uploadFiles, setUploadFiles] = useState<UploadFileItem[]>([]);
  const uploadCurrentIdxRef = useRef(0);
  const [showForceAdd, setShowForceAdd] = useState(false);
  const [paginate, setPaginate] = useState(false);
  const [pendingDrop, setPendingDrop] = useState<string[] | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [playlistsCache, setPlaylistsCache] = useState<PlaylistInfo[]>([]);
  const [currentPlaying, setCurrentPlaying] = useState<SongInfo | null>(null);
  const [detailSong, setDetailSong] = useState<SongInfo | null>(null);
  const [confirm, setConfirm] = useState<ConfirmState>(null);
  const [webdavStatus, setWebdavStatus] = useState<WebDavStatus>({ type: "stopped" });
  const [webdavToggling, setWebdavToggling] = useState(false);
  const [debugOpen, setDebugOpen] = useState(false);
  // 应用设置（localStorage 持久化）
  const [settings, setSettings] = useState<AppSettings>(() => loadSettings());

  const keepAliveRef = useRef<number | null>(null);
  const playlistsCacheRef = useRef<PlaylistInfo[]>([]);

  const triggerRefresh = useCallback(() => setRefreshKey((k) => k + 1), []);

  /** 保存设置到 localStorage 并更新状态。 */
  const saveSettings = useCallback((next: AppSettings) => {
    setSettings(next);
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(next));
    } catch {
      // 忽略写入失败（隐私模式等）
    }
  }, []);

  /** 更新系统托盘 tooltip（连接状态 + 传输进度） */
  const updateTray = useCallback(
    (conn: boolean, transferring?: [number, number]) => {
      invoke("update_tray_tooltip", {
        connected: conn,
        transferring: transferring ?? null,
      }).catch(() => {
        // 托盘更新失败不影响主流程
      });
    },
    [],
  );

  // 连接状态变化时更新托盘
  useEffect(() => {
    updateTray(connected);
  }, [connected, updateTray]);

  // 传输进度变化时更新托盘
  useEffect(() => {
    if (uploading && uploadFiles.length > 0) {
      const done = uploadFiles.filter((f) => f.status === "done" || f.status === "failed").length;
      updateTray(true, [done, uploadFiles.length]);
    }
  }, [uploading, uploadFiles, updateTray]);

  // 监听字节级进度事件：上传进度更新到 uploadFiles 当前文件
  useEffect(() => {
    const unlisteners: Promise<UnlistenFn>[] = [];
    unlisteners.push(
      listen<{ transferred: number; total: number }>("upload-progress", (e) => {
        const idx = uploadCurrentIdxRef.current;
        setUploadFiles((prev) =>
          prev.map((f, i) =>
            i === idx
              ? { ...f, transferred: e.payload.transferred, total: e.payload.total, status: "uploading" as const }
              : f,
          ),
        );
      }),
    );
    unlisteners.push(
      listen<{ transferred: number; total: number }>("download-progress", (e) => {
        const idx = uploadCurrentIdxRef.current;
        setUploadFiles((prev) =>
          prev.map((f, i) =>
            i === idx
              ? { ...f, transferred: e.payload.transferred, total: e.payload.total, status: "uploading" as const }
              : f,
          ),
        );
      }),
    );
    return () => {
      unlisteners.forEach((p) => p.then((fn) => fn()));
    };
  }, []);

  // Phase 6b: Alt+Shift+D 切换调试面板
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.shiftKey && (e.key === 'D' || e.key === 'd')) {
        e.preventDefault();
        setDebugOpen((v) => !v);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // 歌单缓存：连接后立即加载 + 每 60 秒刷新
  // 串行请求，避免并发 USB 传输错误
  const refreshPlaylistsCache = useCallback(async () => {
    try {
      const all: PlaylistInfo[] = [];
      try {
        const internal = await invoke<PlaylistInfo[]>("list_playlists", { memUnit: 0 });
        all.push(...internal.map((p) => ({ ...p, mem_unit: 0 })));
      } catch {}
      try {
        const sd = await invoke<PlaylistInfo[]>("list_playlists", { memUnit: 1 });
        all.push(...sd.map((p) => ({ ...p, mem_unit: 1 })));
      } catch {}
      playlistsCacheRef.current = all;
      setPlaylistsCache(all);
    } catch {}
  }, []);

  useEffect(() => {
    if (!connected) return;
    refreshPlaylistsCache();
    const timer = window.setInterval(refreshPlaylistsCache, 60000);
    return () => clearInterval(timer);
  }, [connected, refreshPlaylistsCache]);

  useEffect(() => {
    if (connected) refreshPlaylistsCache();
  }, [refreshKey, connected, refreshPlaylistsCache]);

  // 播放歌曲（双击或右键"播放试听"）
  async function playSong(song: SongInfo) {
    try {
      setCurrentPlaying(song);
      await invoke("play_song", { fileNo: song.file_no, memUnit: song.mem_unit });
    } catch (e) {
      setNotice(`播放失败：${e}`);
      setCurrentPlaying(null);
    }
  }

  // 启动时检查已连接状态
  useEffect(() => {
    invoke<boolean>("is_connected")
      .then((ok) => {
        if (ok) {
          setConnected(true);
          setDeviceModel("s-series");
        }
      })
      .catch(() => {});
  }, []);

  // 拖拽事件监听
  useEffect(() => {
    if (!connected) return;
    const webview = getCurrentWebview();
    let unlisten: (() => void) | undefined;
    webview
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter") {
          setDragOver(true);
        } else if (event.payload.type === "leave") {
          setDragOver(false);
        } else if (event.payload.type === "drop") {
          setDragOver(false);
          setPendingDrop(event.payload.paths);
        }
      })
      .then((un) => {
        unlisten = un;
      });
    return () => {
      if (unlisten) unlisten();
    };
  }, [connected]);

  // keepAlive：每 30s 读取设备防止休眠
  useEffect(() => {
    return () => {
      if (keepAliveRef.current) clearInterval(keepAliveRef.current);
    };
  }, []);

  function startKeepAlive() {
    if (keepAliveRef.current) clearInterval(keepAliveRef.current);
    keepAliveRef.current = window.setInterval(async () => {
      try {
        await invoke("is_connected");
        await invoke("get_storage", { memUnit: 0 }).catch(() => {});
      } catch {
        setConnected(false);
        stopKeepAlive();
        setNotice("设备已断开");
      }
    }, 30000);
  }

  function stopKeepAlive() {
    if (keepAliveRef.current) {
      clearInterval(keepAliveRef.current);
      keepAliveRef.current = null;
    }
  }

  async function refreshStorage() {
    try {
      const i = await invoke<StorageInfo>("get_storage", { memUnit: 0 });
      const s = await invoke<StorageInfo>("get_storage", { memUnit: 1 }).catch(() => null);
      setStorage({ internal: i, sd: s });
    } catch {
      // 忽略
    }
  }

  async function connectDevice() {
    setConnecting(true);
    setNotice(null);
    try {
      await invoke("open_device");
      setConnected(true);
      setDeviceModel("s-series");
      navigate("songs");
      startKeepAlive();
      await refreshStorage();
    } catch (e) {
      setNotice(`连接失败：${e}`);
    }
    setConnecting(false);
  }

  // 移动端自动连接：跳过设备选择页，直接尝试连接设备
  // 首次延迟 500ms 启动，失败后每 2 秒重试，直到设备连接成功
  const autoConnectAttemptedRef = useRef(false);
  useEffect(() => {
    if (!isMobile || connected || autoConnectAttemptedRef.current) return;
    autoConnectAttemptedRef.current = true;

    let cancelled = false;
    let retryTimer: number | null = null;
    let retryCount = 0;

    const attempt = async () => {
      if (cancelled || connected) return;
      setConnecting(true);
      try {
        await invoke("open_device");
        if (!cancelled) {
          setConnected(true);
          setDeviceModel("s-series");
          navigate("songs");
          startKeepAlive();
          await refreshStorage();
        }
      } catch {
        if (!cancelled) {
          retryCount++;
          // 首几次快速重试（500ms），之后放慢到 2 秒
          const delay = retryCount <= 3 ? 500 : 2000;
          retryTimer = window.setTimeout(attempt, delay);
        }
      }
      if (!cancelled) setConnecting(false);
    };

    // 延迟 500ms 启动，等待 USB Helper 初始化
    retryTimer = window.setTimeout(attempt, 500);

    return () => {
      cancelled = true;
      if (retryTimer) clearTimeout(retryTimer);
    };
  }, [isMobile, connected]);

  async function disconnect() {
    navigate(null);
    setConnected(false);
    stopKeepAlive();
    setStorage({ internal: null, sd: null });
    try {
      await invoke("close_device");
    } catch {}
  }

  // WebDAV 状态轮询
  useEffect(() => {
    if (!connected) {
      setWebdavStatus({ type: "stopped" });
      return;
    }
    let active = true;
    const poll = async () => {
      try {
        const s = await invoke<WebDavStatus>("get_webdav_status");
        if (active) setWebdavStatus(s);
      } catch {}
    };
    poll();
    const timer = setInterval(poll, 3000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [connected]);

  // === 标题栏派生状态 ===
  // 设备型号映射为展示文本（仅连接时显示，未连接返回 null 不留空括号）
  const deviceLabel: string | null = connected
    ? (deviceModel === "s-series" ? "Rio S50"
      : deviceModel === "s30s" ? "Rio S30S"
      : "Rio")
    : null;
  // 传输进行中判定（uploading 且有文件、尚未全部完成）
  const hasFiles = uploadFiles.length > 0;
  const allUploadDone = hasFiles &&
    uploadFiles.every((f) => f.status === "done" || f.status === "failed");
  const transmitting = uploading && hasFiles && !allUploadDone;
  // 当前页面中文名（必须显示且真实反映当前 route）
  const pageLabel: string =
    activeAction ? (MENU_ITEMS.find((it) => it.action === activeAction)?.label ?? "首页")
    : "首页";

  // 顶栏虚拟U盘按钮：启动+挂载 / 停止
  async function toggleWebdav() {
    if (webdavToggling) return;
    setWebdavToggling(true);
    try {
      if (webdavStatus.type === "running") {
        await invoke("stop_webdav");
        setWebdavStatus({ type: "stopped" });
      } else {
        await invoke<string>("start_webdav");
        setWebdavStatus({ type: "running", addr: "http://127.0.0.1:8765" });
        // 自动挂载
        try {
          await invoke("mount_webdav");
        } catch (e) {
          setNotice(`WebDAV 已启动，但自动挂载失败：${e}`);
        }
      }
    } catch (e) {
      setWebdavStatus({ type: "error", message: String(e) });
    }
    setWebdavToggling(false);
  }

  // 拖拽处理
  async function handleDrop(paths: string[], memUnit: number) {
    if (uploading) return;
    setUploading(true);
    setNotice(null);
    // 自动跳转到传输选项卡，让用户看到进度
    navigate("transmission");
    try {
      const expanded = await invoke<string[]>("expand_paths", { paths });
      if (expanded.length === 0) {
        setNotice("没有找到 MP3 文件");
        setUploading(false);
        return;
      }
      // 初始化文件列表
      const items: UploadFileItem[] = expanded.map((p) => {
        const name = p.replace(/\\/g, "/").split("/").pop() || p;
        return { name, transferred: 0, total: 0, status: "pending" as const };
      });
      setUploadFiles(items);
      uploadCurrentIdxRef.current = 0;

      let okCount = 0;
      let failCount = 0;
      for (let i = 0; i < expanded.length; i++) {
        uploadCurrentIdxRef.current = i;
        // 标记当前文件为 uploading
        setUploadFiles((prev) =>
          prev.map((f, idx) => (idx === i ? { ...f, status: "uploading" as const } : f)),
        );
        try {
          const batch = await invoke<BatchUploadResult[]>("upload_song_batch", {
            paths: [expanded[i]],
            memUnit,
            textOpts: settingsToTextOpts(settings),
          });
          if (batch[0]?.success) {
            okCount++;
            setUploadFiles((prev) =>
              prev.map((f, idx) =>
                idx === i ? { ...f, status: "done" as const, transferred: f.total || f.transferred } : f,
              ),
            );
          } else {
            failCount++;
            // 捕获后端返回的具体错误信息（如空间不足、找不到文件等），显示给用户
            const errMsg = batch[0]?.error || "未知错误";
            setUploadFiles((prev) =>
              prev.map((f, idx) => (idx === i ? { ...f, status: "failed" as const, error: errMsg } : f)),
            );
            setNotice(`上传失败：${errMsg}`);
          }
        } catch (e) {
          failCount++;
          // 捕获异常（如设备未连接、USB 错误等），显示给用户
          const errMsg = String(e);
          setUploadFiles((prev) =>
            prev.map((f, idx) => (idx === i ? { ...f, status: "failed" as const, error: errMsg } : f)),
          );
          setNotice(`上传异常：${errMsg}`);
        }
      }
      setNotice(`上传完成：成功 ${okCount} 首${failCount > 0 ? `，失败 ${failCount} 首` : ""}`);
      await refreshStorage();
      triggerRefresh();
    } catch (e) {
      setNotice(`上传失败：${e}`);
    }
    // 延迟清除上传状态，让用户看到完成状态
    setTimeout(() => {
      setUploading(false);
      setUploadFiles([]);
    }, 1500);
  }

  // 未连接：显示连接设备画面
  if (!connected) {
    // 移动端：自动连接，显示简洁加载画面（不显示设备选择页）
    if (isMobile) {
      return (
        <div className="app-root mobile">
          <div className="mobile-auto-connect">
            <div className="mobile-spinner" />
            <div className="mobile-auto-connect-text">
              {connecting ? "正在连接设备…" : "正在检测设备…"}
            </div>
            {notice && <div className="mobile-auto-connect-notice">{notice}</div>}
          </div>
        </div>
      );
    }
    // 桌面端：显示设备选择页
    return (
      <div className={`app-root ${isMobile ? "mobile" : ""}`}>
        {!isMobile && <TitleBar deviceLabel={null} transmitting={false} pageLabel="首页" />}
        <ConnectScene
          connecting={connecting}
          notice={notice}
          onConnect={connectDevice}
          onForceAdd={() => setShowForceAdd(true)}
          onConnected={async () => {
            setConnected(true);
            setDeviceModel("s-series");
            setShowForceAdd(false);
            navigate("songs");
            startKeepAlive();
            await refreshStorage();
          }}
        />
          {showForceAdd && (
            <ForceAddDeviceModal
              onClose={() => setShowForceAdd(false)}
              onConnected={async (model) => {
                setConnected(true);
                setDeviceModel(model);
                setShowForceAdd(false);
                navigate("songs");
                startKeepAlive();
                await refreshStorage();
              }}
            />
          )}
        {debugOpen && (
          <DebugPanel
            route={route}
            navigate={navigate}
            connected={connected}
            playlistsCount={playlistsCache.length}
            paginate={paginate}
          />
        )}
      </div>
    );
  }

  // 已连接：显示主界面
  return (
    <div className={`app-root ${isMobile ? "mobile" : ""}`}>
      {!isMobile && <TitleBar deviceLabel={deviceLabel} transmitting={transmitting} pageLabel={pageLabel} />}
      <div className="launcher">
      {/* 顶部栏：仅桌面端显示返回+虚拟U盘+菜单。移动端不需要 */}
      {!isMobile && (
      <div className="top-bar">
        <button
          className="device-circle-mini"
          onClick={disconnect}
          title="断开并返回"
        >
          <svg className="back-arrow" width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M9 1 L3 7 L9 13" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" fill="none" />
          </svg>
          <span className="back-arrow-text">返回</span>
        </button>

        <button
          className={`webdav-btn ${webdavStatus.type === "running" ? "running" : ""}`}
          onClick={toggleWebdav}
          disabled={webdavToggling || !connected}
          title={
            webdavStatus.type === "running"
              ? "WebDAV 运行中，点击停止"
              : "启动 WebDAV 虚拟U盘并自动挂载"
          }
        >
          {webdavToggling ? "…" : "虚拟U盘"}
        </button>

        {/* 桌面端：顶部菜单栏 */}
        <nav className="menu-bar">
          {MENU_ITEMS.map((item) => {
            // 传输进行中时，禁止切换到其他标签页（强制停留在传输页）
            const locked = uploading && item.action !== "transmission";
            return (
              <button
                key={item.action}
                className={`menu-item ${activeAction === item.action ? "active" : ""} ${locked ? "locked" : ""}`}
                onClick={() => !locked && navigate(item.action)}
                disabled={locked}
                title={locked ? "传输进行中，请等待完成" : undefined}
              >
                <span className="menu-label">{item.label}</span>
              </button>
            );
          })}
        </nav>

        {/* 桌面端：分页切换 */}
        <button
          className={`paginate-toggle ${paginate ? "active" : ""}`}
          onClick={() => setPaginate((v) => !v)}
          title={paginate ? "当前：分页显示，点击切换为全部" : "当前：全部显示，点击切换为分页"}
          aria-label={paginate ? "切换为全部显示" : "切换为分页显示"}
        >
          {paginate ? (
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <rect x="1" y="1" width="12" height="12" stroke="currentColor" strokeWidth="1.2" fill="none" />
              <line x1="7" y1="1" x2="7" y2="13" stroke="currentColor" strokeWidth="1" />
              <line x1="1" y1="7" x2="13" y2="7" stroke="currentColor" strokeWidth="1" />
            </svg>
          ) : (
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <line x1="1" y1="3" x2="13" y2="3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="square" />
              <line x1="1" y1="7" x2="13" y2="7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="square" />
              <line x1="1" y1="11" x2="13" y2="11" stroke="currentColor" strokeWidth="1.4" strokeLinecap="square" />
            </svg>
          )}
        </button>
      </div>
      )}

      <div className="content-area">
          {activeAction && (
            <div
              key={activeAction}
              className="content-inner"
            >
              <ActionContent
                action={activeAction}
                onError={setNotice}
                paginate={paginate}
                refreshKey={refreshKey}
                onRefresh={triggerRefresh}
                onRefreshStorage={refreshStorage}
                playlistsCache={playlistsCache}
                onRefreshPlaylists={refreshPlaylistsCache}
                onPlaySong={playSong}
                onShowDetail={setDetailSong}
                onConfirm={setConfirm}
                onPickFiles={handleDrop}
                isUploading={uploading}
                uploadFiles={uploadFiles}
                uploadNotice={notice}
                settings={settings}
                saveSettings={saveSettings}
                isMobile={isMobile}
              />
            </div>
          )}
          {!activeAction && (
            <div
              key="hint"
              className="content-hint"
            >
              <p>选择上方功能开始操作</p>
              <p className="content-hint-sub">提示：可直接拖拽 MP3 文件或文件夹到任意位置上传</p>
            </div>
          )}
      </div>

      <StorageStatusBar storage={storage} />

        {notice && (
          <div
            className="notice-toast"
          >
            <span>{notice}</span>
            <button className="notice-close" onClick={() => setNotice(null)}>×</button>
          </div>
        )}

        {dragOver && !uploading && (
          <div
            className="drag-overlay"
          >
            <div className="drag-card">
              <div className="drag-text">松开上传到设备</div>
            </div>
          </div>
        )}

        {pendingDrop && (
          <DropTargetModal
            paths={pendingDrop}
            onCancel={() => setPendingDrop(null)}
            onConfirm={async (memUnit) => {
              const paths = pendingDrop;
              setPendingDrop(null);
              await handleDrop(paths, memUnit);
            }}
          />
        )}

      {currentPlaying && (
        <PlayerBar
          song={currentPlaying}
          onClose={() => {
            invoke("stop_audio").catch(() => {});
            setCurrentPlaying(null);
          }}
        />
      )}

        {detailSong && (
          <SongDetailModal song={detailSong} onClose={() => setDetailSong(null)} />
        )}

        {confirm && (
          <ConfirmModal state={confirm} onClose={() => setConfirm(null)} />
        )}

      {debugOpen && (
        <DebugPanel
          route={route}
          navigate={navigate}
          connected={connected}
          playlistsCount={playlistsCache.length}
          paginate={paginate}
        />
      )}
      </div>

      {/* 移动端：底部选项卡栏（M3 Navigation Bar 风格，5项+更多） */}
      {isMobile && (
        <MobileNavBar
          activeAction={activeAction}
          uploading={uploading}
          navigate={navigate}
        />
      )}
    </div>
  );
}

// ============================================================================
// 移动端底部导航栏 — 6 标签（歌曲/歌单/上传传输/设备/设置/关于）
// ============================================================================

/** 移动端底部导航栏
 * - 6 个标签直接平铺：歌曲 / 歌单 / 上传传输 / 设备 / 设置 / 关于
 * - "上传传输"合并了 upload 和 transmission：空闲时进入上传页，传输中自动切换到传输页
 * - 传输进行中时，除"上传传输"外其他标签锁定（灰显+禁用）
 */
function MobileNavBar({
  activeAction,
  uploading,
  navigate,
}: {
  activeAction: MenuAction | null;
  uploading: boolean;
  navigate: (p: string | null) => void;
}) {
  // 导航栏标签定义：action + 显示标签 + 图标
  // "upload-transfer" 是合并标签，点击时根据 uploading 状态决定路由到 upload 还是 transmission
  const tabs: { action: MenuAction | "upload-transfer"; label: string; icon: JSX.Element }[] = [
    { action: "songs", label: "歌曲", icon: MENU_ICONS["songs"] },
    { action: "playlists", label: "歌单", icon: MENU_ICONS["playlists"] },
    { action: "upload-transfer", label: "上传传输", icon: MENU_ICONS["upload"] },
    { action: "device-info", label: "设备", icon: MENU_ICONS["device-info"] },
    { action: "settings", label: "设置", icon: MENU_ICONS["settings"] },
    { action: "about", label: "关于", icon: MENU_ICONS["about"] },
  ];

  // 判断合并标签是否处于激活状态
  const isUploadTransferActive = activeAction === "upload" || activeAction === "transmission";

  // 传输进行中时，除"上传传输"外其他标签锁定
  const isLocked = (tabAction: string): boolean =>
    uploading && tabAction !== "upload-transfer";

  // 点击导航标签
  const handleClick = (tabAction: string) => {
    if (isLocked(tabAction)) return;
    if (tabAction === "upload-transfer") {
      // 合并标签：传输中进入传输页，空闲时进入上传页
      navigate(uploading ? "transmission" : "upload");
    } else {
      navigate(tabAction);
    }
  };

  return (
    <nav className="mobile-tab-bar">
      {tabs.map((tab) => {
        const isActive =
          tab.action === "upload-transfer"
            ? isUploadTransferActive
            : activeAction === tab.action;
        const locked = isLocked(tab.action);
        return (
          <button
            key={tab.action}
            className={`mobile-tab ${isActive ? "active" : ""} ${locked ? "locked" : ""}`}
            onClick={() => handleClick(tab.action)}
            disabled={locked}
          >
            <span className="mobile-tab-icon">{tab.icon}</span>
            <span className="mobile-tab-label">{tab.label}</span>
          </button>
        );
      })}
    </nav>
  );
}

// ============================================================================
// 调试面板（Phase 6b）— Alt+Shift+D 切换，路径输入框与路由绑定
// ============================================================================

function DebugPanel({
  route,
  navigate,
  connected,
  playlistsCount,
  paginate,
}: {
  route: string | null;
  navigate: (p: string | null) => void;
  connected: boolean;
  playlistsCount: number;
  paginate: boolean;
}) {
  const [pathInput, setPathInput] = useState(route || '');
  useEffect(() => { setPathInput(route || ''); }, [route]);

  return (
    <div className="debug-panel">
      <div className="debug-panel-title">调试 (Alt+Shift+D)</div>
      <div className="debug-row">
        <label>page_path:</label>
        <input
          value={pathInput}
          onChange={(e) => setPathInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              navigate(pathInput.trim() || null);
            }
          }}
          placeholder="songs / playlists / upload / sync / device-info / about"
        />
      </div>
      <div className="debug-row">
        <span className="debug-label">connected:</span>
        <span className="debug-value">{connected ? 'true' : 'false'}</span>
      </div>
      <div className="debug-row">
        <span className="debug-label">playlists:</span>
        <span className="debug-value">{playlistsCount}</span>
      </div>
      <div className="debug-row">
        <span className="debug-label">paginate:</span>
        <span className="debug-value">{paginate ? 'on' : 'off'}</span>
      </div>
      <div className="debug-actions">
        <button onClick={() => navigate('songs')}>songs</button>
        <button onClick={() => navigate(null)}>clear</button>
      </div>
    </div>
  );
}

// ============================================================================
// 连接设备画面（未连接时）—— 自动扫描 + 大圆球显示
// ============================================================================

function ConnectScene({
  connecting,
  notice,
  onConnect,
  onForceAdd,
  onConnected,
}: {
  connecting: boolean;
  notice: string | null;
  onConnect: () => void;
  onForceAdd: () => void;
  onConnected: () => void;
}) {
  const [rioDevices, setRioDevices] = useState<UsbDevice[]>([]);
  const [scanning, setScanning] = useState(false);
  const [localConnecting, setLocalConnecting] = useState(false);
  const [localNotice, setLocalNotice] = useState<string | null>(null);
  const scanTimerRef = useRef<number | null>(null);
  const connectingRef = useRef(false);
  const mountedRef = useRef(true);

  const scanRioDevices = useCallback(async () => {
    // 连接中不扫描，避免 nusb list_devices 与 claim_interface 竞态
    if (connectingRef.current) return;
    setScanning(true);
    try {
      const list = await invoke<UsbDevice[]>("list_usb_devices");
      if (!mountedRef.current) return;
      const diamond = list.filter((d) => d.is_diamond);
      setRioDevices(diamond);
    } catch {
      if (!mountedRef.current) return;
      setRioDevices([]);
    }
    if (mountedRef.current) setScanning(false);
  }, []);

  // 初始扫描 + 定时扫描
  useEffect(() => {
    mountedRef.current = true;
    scanRioDevices();
    scanTimerRef.current = window.setInterval(scanRioDevices, AUTO_SCAN_INTERVAL_MS);
    return () => {
      mountedRef.current = false;
      if (scanTimerRef.current) clearInterval(scanTimerRef.current);
    };
  }, [scanRioDevices]);

  function stopScan() {
    if (scanTimerRef.current) {
      clearInterval(scanTimerRef.current);
      scanTimerRef.current = null;
    }
  }

  function restartScan() {
    stopScan();
    scanRioDevices();
    scanTimerRef.current = window.setInterval(scanRioDevices, AUTO_SCAN_INTERVAL_MS);
  }

  // 点击 Rio 大圆球连接
  async function connectRioWrapper(d: UsbDevice) {
    // 立即设置 gate，阻止后续扫描 invoke 与 claim_interface 竞态
    connectingRef.current = true;
    stopScan();
    setLocalConnecting(true);
    setLocalNotice(null);
    try {
      await invoke("open_device_force", { vid: d.vid_num, pid: d.pid_num });
      onConnected();
    } catch (e) {
      setLocalNotice(`连接失败：${e}`);
      connectingRef.current = false;
      // 延迟恢复扫描，给 USB 设备一点恢复时间
      setTimeout(() => {
        if (mountedRef.current) restartScan();
      }, 1000);
    }
    setLocalConnecting(false);
  }

  // 有 Rio 设备：显示大圆球
  if (rioDevices.length > 0) {
    return (
      <div className="connect-scene with-devices">
        <div className="connect-text">检测到 Rio 设备，点击连接</div>
        <div className="rio-orbs">
            {rioDevices.map((d, i) => (
              <button
                key={`${d.vid_num}:${d.pid_num}:${i}`}
                className="rio-orb"
                onClick={() => connectRioWrapper(d)}
                disabled={localConnecting}
                title={`${d.name || "Rio"} · VID ${d.vid} PID ${d.pid}`}
              >
                <div className="rio-orb-icon">
                  <DeviceSvg model="s-series" size={56} />
                </div>
                <div className="rio-orb-label">{d.name || "Rio"}</div>
                <div className="rio-orb-vidpid">{d.vid}:{d.pid}</div>
              </button>
            ))}
        </div>
        {localConnecting && <div className="connect-text">正在连接…</div>}
        {localNotice && <div className="connect-notice">{localNotice}</div>}
        <button className="connect-force-btn" onClick={() => { stopScan(); onForceAdd(); }}>
          + 强制添加任意 USB 设备
        </button>
      </div>
    );
  }

  // 没有 Rio 设备：显示断开连接画面
  return (
    <div className="connect-scene">
      <div className="connect-illustration" onClick={connecting ? undefined : () => { stopScan(); onConnect(); }}>
        <div className="connect-computer">
          <div className="connect-screen">
            <div className="connect-screen-glow" />
          </div>
          <div className="connect-stand" />
          <div className="connect-base" />
        </div>
        <div className="connect-cable">
          <svg width="180" height="40" viewBox="0 0 180 40">
            <path
              d="M 0 20 Q 30 20 50 20 L 70 20"
              stroke="var(--text-dim)"
              strokeWidth="2.5"
              fill="none"
              strokeLinecap="round"
            />
            <rect x="70" y="14" width="14" height="12" rx="2" fill="var(--text-dim)" />
            <g className="spark">
              <circle cx="90" cy="20" r="2" fill="var(--warning)" />
              <path d="M 86 16 L 88 18 M 94 16 L 92 18 M 86 24 L 88 22 M 94 24 L 92 22"
                stroke="var(--warning)" strokeWidth="1" />
            </g>
            <rect x="96" y="14" width="14" height="12" rx="2" fill="var(--text-dim)" />
            <path
              d="M 110 20 L 130 20 Q 150 20 180 20"
              stroke="var(--text-dim)"
              strokeWidth="2.5"
              fill="none"
              strokeLinecap="round"
            />
          </svg>
        </div>
        <div className="connect-device">
          <DeviceSvg model="rio-500" size={72} />
        </div>
      </div>
      <div className="connect-text">
        {scanning ? "正在扫描 USB 设备…" : connecting ? "正在连接…" : "未检测到 Rio 设备，请连接后自动识别"}
      </div>
      {notice && <div className="connect-notice">{notice}</div>}
      <button className="connect-force-btn" onClick={() => { stopScan(); onForceAdd(); }}>
        + 强制添加任意 USB 设备
      </button>
    </div>
  );
}

// ============================================================================
// 拖拽存储区选择模态框
// ============================================================================

function DropTargetModal({
  paths,
  onCancel,
  onConfirm,
}: {
  paths: string[];
  onCancel: () => void;
  onConfirm: (memUnit: number) => void;
}) {
  const [expanded, setExpanded] = useState<string[] | null>(null);
  const [expanding, setExpanding] = useState(true);

  useEffect(() => {
    invoke<string[]>("expand_paths", { paths })
      .then((list) => {
        setExpanded(list);
        setExpanding(false);
      })
      .catch(() => {
        setExpanded([]);
        setExpanding(false);
      });
  }, [paths]);

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div
        className="modal-content drop-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>选择上传目标</h3>
          <button className="modal-close" onClick={onCancel}>×</button>
        </div>
        <div className="modal-body">
          <p className="modal-desc">
            {expanding
              ? "正在扫描 MP3 文件…"
              : expanded && expanded.length > 0
              ? `将上传 ${expanded.length} 个 MP3 文件到：`
              : "未找到 MP3 文件"}
          </p>
          {expanded && expanded.length > 0 && (
            <div className="drop-target-grid">
              <button
                className="drop-target-card"
                onClick={() => onConfirm(0)}
              >
                <div className="drop-target-icon">内置</div>
                <div className="drop-target-hint">内部存储</div>
              </button>
              <button
                className="drop-target-card"
                onClick={() => onConfirm(1)}
              >
                <div className="drop-target-icon">SD</div>
                <div className="drop-target-hint">SD 卡</div>
              </button>
            </div>
          )}
        </div>
        <div className="modal-footer">
          <button className="modal-btn" onClick={onCancel}>取消</button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 强制添加设备模态框
// ============================================================================

function ForceAddDeviceModal({
  onClose,
  onConnected,
}: {
  onClose: () => void;
  onConnected: (model: DeviceModel, label: string) => void;
}) {
  const [devices, setDevices] = useState<UsbDevice[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<number | null>(null);

  const loadDevices = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<UsbDevice[]>("list_usb_devices");
      list.sort((a, b) => {
        if (a.is_diamond && !b.is_diamond) return -1;
        if (!a.is_diamond && b.is_diamond) return 1;
        return 0;
      });
      setDevices(list);
    } catch (e) {
      setError(`加载失败: ${e}`);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    loadDevices();
  }, [loadDevices]);

  async function connectOne(d: UsbDevice) {
    setConnecting(d.vid_num);
    setError(null);
    try {
      const info = await invoke<{ connected: boolean; model: string }>("open_device_force", {
        vid: d.vid_num,
        pid: d.pid_num,
      });
      const model: DeviceModel = d.is_diamond ? "s-series" : "s-series";
      onConnected(model, info.model);
    } catch (e) {
      setError(`连接失败：${e}`);
    }
    setConnecting(null);
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>强制添加 USB 设备</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="modal-body">
          <p className="modal-desc">
            选择任意 USB 设备，强制以 Rio 协议打开。Diamond 设备已置顶。
          </p>
          {loading && <div className="loading-text">加载中…</div>}
          {error && <div className="error-banner">{error}</div>}
          {!loading && !error && devices.length === 0 && (
            <div className="empty-text">未检测到任何 USB 设备</div>
          )}
          {!loading && devices.length > 0 && (
            <div className="usb-device-list">
              {devices.map((d, i) => (
                <button
                  key={`${d.vid_num}:${d.pid_num}:${i}`}
                  className={`usb-device-item ${d.is_diamond ? "diamond" : ""}`}
                  onClick={() => connectOne(d)}
                  disabled={connecting !== null}
                >
                  <div className="usb-device-info">
                    <div className="usb-device-name">
                      {d.name || "(未命名设备)"}
                      {d.is_diamond && <span className="usb-badge">Diamond</span>}
                    </div>
                    <div className="usb-device-vidpid">
                      VID {d.vid} · PID {d.pid}
                      {d.manufacturer && ` · ${d.manufacturer}`}
                    </div>
                  </div>
                  {connecting === d.vid_num ? (
                    <span className="usb-connecting">连接中…</span>
                  ) : (
                    <span className="usb-arrow">连接</span>
                  )}
                </button>
              ))}
            </div>
          )}
        </div>
        <div className="modal-footer">
          <button className="modal-btn refresh" onClick={loadDevices} disabled={loading}>
            刷新
          </button>
          <button className="modal-btn" onClick={onClose}>取消</button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 存储状态条（顶栏上方）
// ============================================================================

function StorageStatusBar({
  storage,
}: {
  storage: { internal: StorageInfo | null; sd: StorageInfo | null };
}) {
  const { internal, sd } = storage;
  if (!internal && !sd) {
    return (
      <div className="storage-status-bar">
        <span className="storage-status-text">读取存储信息中…</span>
      </div>
    );
  }
  return (
    <div className="storage-status-bar">
      {internal && (
        <div className="storage-status-item">
          <span className="storage-status-label">内置</span>
          <span className="storage-status-value">
            {formatSize(internal.free)} / {formatSize(internal.size)}
          </span>
          <div className="storage-status-mini-bar">
            <div
              className="storage-status-mini-fill"
              style={{ width: `${internal.size > 0 ? (internal.used / internal.size) * 100 : 0}%` }}
            />
          </div>
        </div>
      )}
      {sd && sd.present && (
        <div className="storage-status-item">
          <span className="storage-status-label">SD</span>
          <span className="storage-status-value">
            {formatSize(sd.free)} / {formatSize(sd.size)}
          </span>
          <div className="storage-status-mini-bar">
            <div
              className="storage-status-mini-fill sd"
              style={{ width: `${sd.size > 0 ? (sd.used / sd.size) * 100 : 0}%` }}
            />
          </div>
        </div>
      )}
      {!sd && (
        <div className="storage-status-item dim">
          <span className="storage-status-label">SD</span>
          <span className="storage-status-value">未插入</span>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// 功能页面分发
// ============================================================================

function ActionContent({
  action,
  onError,
  paginate,
  refreshKey,
  onRefresh,
  onRefreshStorage,
  playlistsCache,
  onRefreshPlaylists,
  onPlaySong,
  onShowDetail,
  onConfirm,
  onPickFiles,
  isUploading,
  uploadFiles,
  uploadNotice,
  settings,
  saveSettings,
  isMobile,
}: {
  action: MenuAction;
  onError: (msg: string | null) => void;
  paginate: boolean;
  refreshKey: number;
  onRefresh: () => void;
  onRefreshStorage: () => void;
  playlistsCache: PlaylistInfo[];
  onRefreshPlaylists: () => void;
  onPlaySong: (s: SongInfo) => void;
  onShowDetail: (s: SongInfo) => void;
  onConfirm: (s: ConfirmState) => void;
  onPickFiles: (paths: string[], memUnit: number) => Promise<void>;
  isUploading: boolean;
  uploadFiles: UploadFileItem[];
  uploadNotice: string | null;
  settings: AppSettings;
  saveSettings: (s: AppSettings) => void;
  isMobile: boolean;
}) {
  if (action === "songs")
    return (
      <SongsPane
        onError={onError}
        paginate={paginate}
        refreshKey={refreshKey}
        playlistsCache={playlistsCache}
        onRefreshPlaylists={onRefreshPlaylists}
        onPlaySong={onPlaySong}
        onShowDetail={onShowDetail}
        onConfirm={onConfirm}
        isMobile={isMobile}
      />
    );
  if (action === "playlists")
    return (
      <PlaylistsPane
        onError={onError}
        paginate={paginate}
        refreshKey={refreshKey}
        onPlaySong={onPlaySong}
        onShowDetail={onShowDetail}
        onConfirm={onConfirm}
      />
    );
  if (action === "device-info")
    return (
      <DeviceInfoPane
        refreshKey={refreshKey}
        onRefreshStorage={onRefreshStorage}
      />
    );
  if (action === "upload")
    return (
      <UploadPane
        onError={onError}
        onUploaded={() => {
          onRefresh();
          onRefreshStorage();
        }}
        onPickFiles={onPickFiles}
        isUploading={isUploading}
      />
    );
  if (action === "transmission")
    return (
      <TransmissionPane
        files={uploadFiles}
        uploading={isUploading}
        notice={uploadNotice}
      />
    );
  if (action === "sync")
    return (
      <SyncPane
        onError={onError}
        onRefreshStorage={onRefreshStorage}
        playlistsCache={playlistsCache}
        onRefreshPlaylists={onRefreshPlaylists}
        onConfirm={onConfirm}
      />
    );
  if (action === "settings")
    return (
      <SettingsPane
        settings={settings}
        onSave={saveSettings}
        onError={(msg) => onError(msg)}
      />
    );
  if (action === "about") return <AboutPane />;
  return null;
}

// ============================================================================
// 虚拟滚动 hook — 只渲染可见行，大幅减少 DOM 节点数量
// ============================================================================

/** 虚拟滚动配置 */
const VIRTUAL_ROW_HEIGHT = 28; // 每行高度（px），与 CSS padding 一致
const VIRTUAL_BUFFER = 5; // 上下额外渲染的缓冲行数

function useVirtualScroll(itemCount: number) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(600);

  // 监听滚动和容器大小变化
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const onScroll = () => setScrollTop(el.scrollTop);
    const onResize = () => setViewportHeight(el.clientHeight);

    el.addEventListener('scroll', onScroll, { passive: true });
    onResize();

    // ResizeObserver 监听容器大小变化
    const ro = new ResizeObserver(onResize);
    ro.observe(el);

    return () => {
      el.removeEventListener('scroll', onScroll);
      ro.disconnect();
    };
  }, []);

  // 计算可见范围
  const startIndex = Math.max(0, Math.floor(scrollTop / VIRTUAL_ROW_HEIGHT) - VIRTUAL_BUFFER);
  const endIndex = Math.min(
    itemCount,
    Math.ceil((scrollTop + viewportHeight) / VIRTUAL_ROW_HEIGHT) + VIRTUAL_BUFFER
  );

  // 总高度（用于撑开滚动条）
  const totalHeight = itemCount * VIRTUAL_ROW_HEIGHT;
  // 可见区域的偏移量
  const offsetY = startIndex * VIRTUAL_ROW_HEIGHT;

  return {
    containerRef,
    startIndex,
    endIndex,
    totalHeight,
    offsetY,
    // 滚动到指定索引
    scrollToIndex: useCallback((idx: number) => {
      const el = containerRef.current;
      if (!el) return;
      const targetTop = idx * VIRTUAL_ROW_HEIGHT;
      const currentStart = el.scrollTop;
      const currentEnd = el.scrollTop + el.clientHeight;
      if (targetTop < currentStart || targetTop + VIRTUAL_ROW_HEIGHT > currentEnd) {
        el.scrollTo({ top: Math.max(0, targetTop - el.clientHeight / 2), behavior: 'smooth' });
      }
    }, []),
  };
}

// ============================================================================
// 列表导航 hook
// ============================================================================

function useListNavigation<T>(items: T[], scrollToIndex?: (idx: number) => void) {
  const [selectedIdx, setSelectedIdx] = useState(-1);

  useEffect(() => {
    setSelectedIdx(-1);
  }, [items]);

  useEffect(() => {
    if (items.length === 0) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => Math.min(i + 1, items.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => Math.max(i - 1, 0));
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [items]);

  useEffect(() => {
    if (selectedIdx < 0) return;
    if (scrollToIndex) {
      scrollToIndex(selectedIdx);
    }
  }, [selectedIdx, scrollToIndex]);

  return { selectedIdx, setSelectedIdx };
}

// ============================================================================
// 筛选 hook（支持分页开关）
// ============================================================================

type SortKey = "name" | "size" | "time";
function useFilteredPaged<T>(
  items: T[],
  opts: {
    search: string;
    sortBy: SortKey;
    getSearchText: (t: T) => string;
    getSortValue: (t: T, key: SortKey) => string | number;
  },
  paginate: boolean,
  pageSize: number
) {
  const [page, setPage] = useState(0);

  useEffect(() => {
    setPage(0);
  }, [items, opts.search, opts.sortBy, paginate]);

  const filtered = useMemo(() => {
    let r = items;
    if (opts.search.trim()) {
      const q = opts.search.trim().toLowerCase();
      r = r.filter((t) => opts.getSearchText(t).toLowerCase().includes(q));
    }
    const sorted = [...r].sort((a, b) => {
      const va = opts.getSortValue(a, opts.sortBy);
      const vb = opts.getSortValue(b, opts.sortBy);
      if (typeof va === "number" && typeof vb === "number") return vb - va;
      return String(va).localeCompare(String(vb), "zh-Hans");
    });
    return sorted;
  }, [items, opts.search, opts.sortBy]);

  if (!paginate) {
    return { filtered, pageItems: filtered, page: 0, totalPages: 1, setPage: () => {} };
  }

  const totalPages = Math.max(1, Math.ceil(filtered.length / pageSize));
  const safePage = Math.min(page, totalPages - 1);
  const pageItems = filtered.slice(safePage * pageSize, (safePage + 1) * pageSize);

  return { filtered, pageItems, page: safePage, totalPages, setPage };
}

// ============================================================================
// 筛选栏
// ============================================================================

function FilterBar({
  search,
  setSearch,
  sortBy,
  setSortBy,
  total,
  shown,
}: {
  search: string;
  setSearch: (v: string) => void;
  sortBy: SortKey;
  setSortBy: (v: SortKey) => void;
  total: number;
  shown: number;
}) {
  return (
    <div className="filter-bar">
      <div className="filter-group">
        <span className="filter-label">排序</span>
        <div className="seg-control">
          {([
            { v: "name", l: "名称" },
            { v: "size", l: "大小" },
            { v: "time", l: "时间" },
          ] as { v: SortKey; l: string }[]).map((o) => (
            <button
              key={o.v}
              className={sortBy === o.v ? "active" : ""}
              onClick={() => setSortBy(o.v)}
            >
              {o.l}
            </button>
          ))}
        </div>
      </div>
      <input
        className="filter-search"
        type="text"
        placeholder="搜索…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
      />
      <div className="filter-count">{shown} / {total}</div>
    </div>
  );
}

// ============================================================================
// 翻页栏
// ============================================================================

function Pagination({
  page,
  totalPages,
  setPage,
}: {
  page: number;
  totalPages: number;
  setPage: (p: number) => void;
}) {
  if (totalPages <= 1) return null;
  return (
    <div className="pagination">
      <button className="page-btn" onClick={() => setPage(Math.max(0, page - 1))} disabled={page === 0}>
        上一页
      </button>
      <span className="page-info">{page + 1} / {totalPages}</span>
      <button
        className="page-btn"
        onClick={() => setPage(Math.min(totalPages - 1, page + 1))}
        disabled={page === totalPages - 1}
      >
        下一页
      </button>
    </div>
  );
}

// ============================================================================
// 批量操作工具栏
// ============================================================================

function BatchToolbar({
  selectedCount,
  onSelectAll,
  onClearSelection,
  onBatchDelete,
  onBatchAddToPlaylist,
  onRefresh,
  loading,
  showAddToPlaylist,
  batchMenuOpen,
  setBatchMenuOpen,
  onBatchSlugSelected,
  onBatchStripSelected,
  onBatchSlugAll,
  onBatchStripAll,
  onRepairAllEncoding,
  onRepairSelectedEncoding,
}: {
  selectedCount: number;
  onSelectAll: () => void;
  onClearSelection: () => void;
  onBatchDelete: () => void;
  onBatchAddToPlaylist: () => void;
  onRefresh: () => void;
  loading: boolean;
  showAddToPlaylist: boolean;
  batchMenuOpen?: boolean;
  setBatchMenuOpen?: (v: boolean) => void;
  onBatchSlugSelected?: () => void;
  onBatchStripSelected?: () => void;
  onBatchSlugAll?: () => void;
  onBatchStripAll?: () => void;
  onRepairAllEncoding?: () => void;
  onRepairSelectedEncoding?: () => void;
}) {
  const hasMoreMenu = !!(onBatchSlugSelected || onBatchStripSelected || onBatchSlugAll || onBatchStripAll || onRepairAllEncoding || onRepairSelectedEncoding);

  // 更多菜单：点击外部关闭
  useEffect(() => {
    if (!batchMenuOpen) return;
    function onClickAway() {
      setBatchMenuOpen?.(false);
    }
    function onEsc(e: KeyboardEvent) {
      if (e.key === "Escape") setBatchMenuOpen?.(false);
    }
    const t = setTimeout(() => {
      window.addEventListener("click", onClickAway);
      window.addEventListener("keydown", onEsc);
    }, 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("click", onClickAway);
      window.removeEventListener("keydown", onEsc);
    };
  }, [batchMenuOpen, setBatchMenuOpen]);

  return (
    <div className="batch-toolbar">
      <button className="batch-btn" onClick={onSelectAll}>全选</button>
      <button className="batch-btn" onClick={onClearSelection} disabled={selectedCount === 0}>
        清空
      </button>
      <button
        className="batch-btn danger"
        onClick={onBatchDelete}
        disabled={selectedCount === 0}
      >
        删除{selectedCount > 0 ? ` (${selectedCount})` : ""}
      </button>
      {showAddToPlaylist && (
        <button
          className="batch-btn"
          onClick={onBatchAddToPlaylist}
          disabled={selectedCount === 0}
        >
          加入歌单{selectedCount > 0 ? ` (${selectedCount})` : ""}
        </button>
      )}
      {/* 更多下拉菜单：批量转拼音 / 去词 / 修复编码（仅歌曲页显示） */}
      {hasMoreMenu && batchMenuOpen !== undefined && setBatchMenuOpen && (
        <div className="batch-more-wrap">
          <button
            className="batch-btn"
            onClick={(e) => {
              e.stopPropagation();
              setBatchMenuOpen(!batchMenuOpen);
            }}
          >
            更多
          </button>
          {batchMenuOpen && (
            <div className="batch-more-menu" onClick={(e) => e.stopPropagation()}>
              <div className="batch-more-group-title">仅选中（{selectedCount}）</div>
              <button
                className="batch-more-item"
                onClick={() => onBatchSlugSelected?.()}
                disabled={selectedCount === 0}
              >
                转拼音
              </button>
              <button
                className="batch-more-item"
                onClick={() => onBatchStripSelected?.()}
                disabled={selectedCount === 0}
              >
                去词
              </button>
              <button
                className="batch-more-item"
                onClick={() => onRepairSelectedEncoding?.()}
                disabled={selectedCount === 0}
              >
                修复编码
              </button>
              <div className="batch-more-sep" />
              <div className="batch-more-group-title">全部歌曲</div>
              <button className="batch-more-item" onClick={() => onBatchSlugAll?.()}>
                全部转拼音
              </button>
              <button className="batch-more-item" onClick={() => onBatchStripAll?.()}>
                全部去词
              </button>
              <div className="batch-more-sep" />
              <button className="batch-more-item" onClick={() => onRepairAllEncoding?.()}>
                修复所有编码
              </button>
            </div>
          )}
        </div>
      )}
      <button className="batch-btn refresh" onClick={onRefresh} disabled={loading} title="刷新">
        刷新
      </button>
    </div>
  );
}

// ============================================================================
// 歌曲页面（自行加载数据，不缓存）
// ============================================================================

function songKey(s: SongInfo): string {
  return `${s.mem_unit}:${s.file_no}`;
}

function SongsPane({
  onError,
  paginate,
  refreshKey,
  playlistsCache,
  onRefreshPlaylists,
  onPlaySong,
  onShowDetail,
  onConfirm,
  isMobile,
}: {
  onError: (msg: string | null) => void;
  paginate: boolean;
  refreshKey: number;
  playlistsCache: PlaylistInfo[];
  onRefreshPlaylists: () => void;
  onPlaySong: (s: SongInfo) => void;
  onShowDetail: (s: SongInfo) => void;
  onConfirm: (s: ConfirmState) => void;
  isMobile: boolean;
}) {
  const [songs, setSongs] = useState<SongInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [sortBy, setSortBy] = useState<SortKey>("name");
  const [showPlaylistPicker, setShowPlaylistPicker] = useState(false);
  const [pickerTargetSongs, setPickerTargetSongs] = useState<SongInfo[]>([]);
  const [deleting, setDeleting] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState>(null);
  const [renameTarget, setRenameTarget] = useState<SongInfo | null>(null);
  const [batchMenuOpen, setBatchMenuOpen] = useState(false);
  const [batchPreview, setBatchPreview] = useState<BatchPreviewState>(null);
  const lastClickedIdxRef = useRef<number | null>(null);
  /** 标记组件是否已卸载——卸载后忽略所有异步回调，防止 loading 卡死 */
  const unmountedRef = useRef(false);

  // 加载歌曲数据：挂载时 + refreshKey 变化时
  // 串行请求内置 + SD 卡，避免并发锁竞争导致 USB 传输错误
  const loadSongs = useCallback(async () => {
    setLoading(true);
    try {
      const all: SongInfo[] = [];
      // 先请求内置存储
      try {
        const internal = await invoke<SongInfo[]>("list_songs", { memUnit: 0 });
        if (!unmountedRef.current) {
          all.push(...internal.map((s) => ({ ...s, mem_unit: 0 })));
        }
      } catch {
        // 内置存储读取失败（不应发生，忽略）
      }
      // 再请求 SD 卡（串行，避免与内置存储的 USB 传输并发）
      try {
        const sd = await invoke<SongInfo[]>("list_songs", { memUnit: 1 });
        if (!unmountedRef.current) {
          all.push(...sd.map((s) => ({ ...s, mem_unit: 1 })));
        }
      } catch {
        // SD 卡未插入或读取失败，忽略（不显示 SD 卡歌曲）
      }
      if (unmountedRef.current) return;
      setSongs(all);
    } catch (e) {
      if (unmountedRef.current) return;
      onError(`加载歌曲失败: ${e}`);
    }
    if (!unmountedRef.current) setLoading(false);
  }, [onError]);

  // 组件卸载时设置 unmounted 标志，防止异步回调更新已卸载的组件
  // 标记组件是否已卸载——卸载后忽略所有异步回调，防止 loading 卡死
  useEffect(() => {
    unmountedRef.current = false;
    return () => {
      unmountedRef.current = true;
    };
  }, []);

  useEffect(() => {
    loadSongs();
  }, [loadSongs, refreshKey]);

  // 监听 rename-progress 事件（批量操作进度反馈）
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<RenameProgress>("rename-progress", (e) => {
      if (unmountedRef.current) return;
      const p = e.payload;
      setBatchPreview((prev) => {
        if (!prev || prev.phase !== "running") return prev;
        return {
          ...prev,
          progress: {
            current: p.current,
            total: p.total,
            currentTitle: p.current_title,
          },
        };
      });
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const { filtered, pageItems, page, totalPages, setPage } = useFilteredPaged(
    songs,
    {
      search,
      sortBy,
      getSearchText: (s) => `${s.title} ${s.name} ${s.artist} ${s.album}`,
      getSortValue: (s, key) => {
        if (key === "name") return s.title || s.name;
        if (key === "size") return s.size;
        return s.time;
      },
    },
    paginate,
    10
  );

  // 虚拟滚动：只渲染可见行，大幅减少 DOM 节点
  const virtual = useVirtualScroll(pageItems.length);
  const { selectedIdx, setSelectedIdx } = useListNavigation(pageItems, virtual.scrollToIndex);

  function selectAll() {
    setSelected(new Set(filtered.map(songKey)));
  }

  function clearSelection() {
    setSelected(new Set());
  }

  async function batchDelete() {
    if (selected.size === 0) return;
    const count = selected.size;
    onConfirm({
      title: "批量删除歌曲",
      message: `确认删除选中的 ${count} 首歌曲？此操作不可撤销。`,
      danger: true,
      onConfirm: async () => {
        setDeleting(true);
        let okCount = 0;
        let failCount = 0;
        for (const s of songs) {
          if (selected.has(songKey(s))) {
            try {
              await invoke("delete_song", { fileNo: s.file_no, memUnit: s.mem_unit });
              okCount++;
            } catch {
              failCount++;
            }
          }
        }
        setSelected(new Set());
        onError(`删除完成：成功 ${okCount} 项${failCount > 0 ? `，失败 ${failCount} 项` : ""}`);
        setDeleting(false);
        await loadSongs();
      },
    });
  }

  // 用缓存立即弹出歌单选择器
  function batchAddToPlaylist() {
    if (selected.size === 0) return;
    const targets = songs.filter((s) => selected.has(songKey(s)));
    setPickerTargetSongs(targets);
    setShowPlaylistPicker(true);
  }

  // 右键单首加入歌单
  function addToPlaylistSingle(song: SongInfo) {
    setPickerTargetSongs([song]);
    setShowPlaylistPicker(true);
  }

  async function addToSpecificPlaylist(p: PlaylistInfo) {
    setShowPlaylistPicker(false);
    let okCount = 0;
    let failCount = 0;
    for (const s of pickerTargetSongs) {
      try {
        await invoke("add_song_to_playlist", {
          songFileNo: s.file_no,
          songMemUnit: s.mem_unit,
          playlistFileNo: p.file_no,
          playlistMemUnit: p.mem_unit,
        });
        okCount++;
      } catch {
        failCount++;
      }
    }
    onError(`加入歌单完成：成功 ${okCount} 项${failCount > 0 ? `，失败 ${failCount} 项` : ""}`);
    setSelected(new Set());
  }

  async function downloadSong(song: SongInfo) {
    try {
      const title = song.title || song.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "song";
      // 简单下载：用浏览器 API 不可，需要 Tauri dialog。这里用 invoke download_song 但需要 save_path
      // 暂用 notice 提示
      onError(`下载功能：请用 ${title}.mp3`);
    } catch (e) {
      onError(`下载失败: ${e}`);
    }
  }

  async function deleteSingle(song: SongInfo) {
    const title = song.title || song.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "(无标题)";
    onConfirm({
      title: "删除歌曲",
      message: `确认删除 "${title}"？此操作不可撤销。`,
      danger: true,
      onConfirm: async () => {
        try {
          await invoke("delete_song", { fileNo: song.file_no, memUnit: song.mem_unit });
          onError("删除成功");
          await loadSongs();
        } catch (e) {
          onError(`删除失败: ${e}`);
        }
      },
    });
  }

  // 修复单首歌曲编码（download → overwrite，清 bit 0）
  async function repairSingleEncoding(song: SongInfo) {
    const title = song.title || song.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "(无标题)";
    onConfirm({
      title: "修复编码",
      message: `将修复 "${title}" 的中文编码。继续？`,
      onConfirm: async () => {
        try {
          await invoke("repair_song_encoding", {
            fileNo: song.file_no,
            memUnit: song.mem_unit,
          });
          onError("编码已修复");
          await loadSongs();
        } catch (e) {
          onError(`修复失败：${e}`);
        }
      },
    });
  }

  /** 把当前选中的歌曲转成 RenameItemInput 数组 */
  function selectedItems(): { fileNo: number; memUnit: number; title: string }[] {
    return songs
      .filter((s) => selected.has(songKey(s)))
      .map((s) => ({
        fileNo: s.file_no,
        memUnit: s.mem_unit,
        title: s.title || s.name || "",
      }));
  }

  /** 从自定义停用词文本解析为数组 */
  function customWords(): string[] {
    const raw = localStorage.getItem("cyrio.settings");
    if (!raw) return [];
    try {
      const parsed = JSON.parse(raw) as { custom_stop_words?: string };
      return (parsed.custom_stop_words || "")
        .split("\n")
        .map((w) => w.trim())
        .filter((w) => w.length > 0);
    } catch {
      return [];
    }
  }

  // 批量转拼音（仅对选中项）— 先预览，再执行
  async function batchSlugSelected() {
    setBatchMenuOpen(false);
    if (selected.size === 0) return;
    const items = selectedItems();
    try {
      const previews = await invoke<PreviewResult[]>("preview_slug", { items });
      if (unmountedRef.current) return;
      setBatchPreview({
        phase: "preview",
        title: `转拼音（${items.length} 首歌曲）`,
        previews,
        progress: null,
        results: null,
        error: null,
      });
    } catch (e) {
      onError(`转拼音预览失败：${e}`);
    }
  }

  // 批量去词（仅对选中项）— 先预览，再执行
  async function batchStripSelected() {
    setBatchMenuOpen(false);
    if (selected.size === 0) return;
    const items = selectedItems();
    const words = customWords();
    try {
      const previews = await invoke<PreviewResult[]>("preview_strip", { items, customWords: words });
      if (unmountedRef.current) return;
      setBatchPreview({
        phase: "preview",
        title: `去词（${items.length} 首歌曲）`,
        previews,
        progress: null,
        results: null,
        error: null,
      });
    } catch (e) {
      onError(`去词预览失败：${e}`);
    }
  }

  /** 执行批量转拼音（从预览对话框确认后调用） */
  async function executeBatchSlug(items: { fileNo: number; memUnit: number; title: string }[]) {
    setBatchPreview({
      phase: "running",
      title: "正在转拼音…",
      previews: [],
      progress: { current: 0, total: items.length, currentTitle: "" },
      results: null,
      error: null,
    });
    try {
      const results = await invoke<RenameResult[]>("batch_slug_songs", { items });
      if (unmountedRef.current) return;
      const successCount = results.filter((r) => r.success).length;
      const failedCount = results.filter((r) => !r.success).length;
      const errors = results
        .filter((r) => !r.success && r.error)
        .map((r) => `${r.original || "(无标题)"}: ${r.error}`)
        .slice(0, 5)
        .join("\n");
      setSelected(new Set());
      await loadSongs();
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        results: { success: successCount, failed: failedCount, skipped: 0 },
        error: failedCount > 0 ? errors : null,
      } : null);
      if (failedCount > 0) onError(`转拼音有 ${failedCount} 项失败：\n${errors}`);
    } catch (e) {
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        error: String(e),
        results: { success: 0, failed: items.length, skipped: 0 },
      } : null);
      onError(`转拼音失败：${e}`);
    }
  }

  /** 执行批量去词（从预览对话框确认后调用） */
  async function executeBatchStrip(items: { fileNo: number; memUnit: number; title: string }[]) {
    setBatchPreview({
      phase: "running",
      title: "正在去词…",
      previews: [],
      progress: { current: 0, total: items.length, currentTitle: "" },
      results: null,
      error: null,
    });
    try {
      const results = await invoke<RenameResult[]>("batch_strip_songs", { items, customWords: customWords() });
      if (unmountedRef.current) return;
      const successCount = results.filter((r) => r.success).length;
      const failedCount = results.filter((r) => !r.success).length;
      const errors = results
        .filter((r) => !r.success && r.error)
        .map((r) => `${r.original || "(无标题)"}: ${r.error}`)
        .slice(0, 5)
        .join("\n");
      setSelected(new Set());
      await loadSongs();
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        results: { success: successCount, failed: failedCount, skipped: 0 },
        error: failedCount > 0 ? errors : null,
      } : null);
      if (failedCount > 0) onError(`去词有 ${failedCount} 项失败：\n${errors}`);
    } catch (e) {
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        error: String(e),
        results: { success: 0, failed: items.length, skipped: 0 },
      } : null);
      onError(`去词失败：${e}`);
    }
  }

  // 全部转拼音 — 先预览，再执行
  async function batchSlugAll() {
    setBatchMenuOpen(false);
    if (songs.length === 0) return;
    const items = songs.map((s) => ({
      fileNo: s.file_no,
      memUnit: s.mem_unit,
      title: s.title || s.name || "",
    }));
    try {
      const previews = await invoke<PreviewResult[]>("preview_slug", { items });
      if (unmountedRef.current) return;
      setBatchPreview({
        phase: "preview",
        title: `全部转拼音（${songs.length} 首歌曲）`,
        previews,
        progress: null,
        results: null,
        error: null,
      });
    } catch (e) {
      onError(`转拼音预览失败：${e}`);
    }
  }

  // 全部去词 — 先预览，再执行
  async function batchStripAll() {
    setBatchMenuOpen(false);
    if (songs.length === 0) return;
    const items = songs.map((s) => ({
      fileNo: s.file_no,
      memUnit: s.mem_unit,
      title: s.title || s.name || "",
    }));
    const words = customWords();
    try {
      const previews = await invoke<PreviewResult[]>("preview_strip", { items, customWords: words });
      if (unmountedRef.current) return;
      setBatchPreview({
        phase: "preview",
        title: `全部去词（${songs.length} 首歌曲）`,
        previews,
        progress: null,
        results: null,
        error: null,
      });
    } catch (e) {
      onError(`去词预览失败：${e}`);
    }
  }

  /** 执行全部转拼音 */
  async function executeBatchSlugAll() {
    setBatchPreview({
      phase: "running",
      title: "正在转拼音（全部）…",
      previews: [],
      progress: { current: 0, total: songs.length, currentTitle: "" },
      results: null,
      error: null,
    });
    try {
      const results = await invoke<RenameResult[]>("batch_slug_all_songs");
      if (unmountedRef.current) return;
      const successCount = results.filter((r) => r.success).length;
      const failedCount = results.filter((r) => !r.success).length;
      const errors = results
        .filter((r) => !r.success && r.error)
        .map((r) => `${r.original || "(无标题)"}: ${r.error}`)
        .slice(0, 5)
        .join("\n");
      await loadSongs();
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        results: { success: successCount, failed: failedCount, skipped: 0 },
        error: failedCount > 0 ? errors : null,
      } : null);
      if (failedCount > 0) onError(`转拼音有 ${failedCount} 项失败：\n${errors}`);
    } catch (e) {
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        error: String(e),
        results: { success: 0, failed: songs.length, skipped: 0 },
      } : null);
      onError(`转拼音失败：${e}`);
    }
  }

  /** 执行全部去词 */
  async function executeBatchStripAll() {
    setBatchPreview({
      phase: "running",
      title: "正在去词（全部）…",
      previews: [],
      progress: { current: 0, total: songs.length, currentTitle: "" },
      results: null,
      error: null,
    });
    try {
      const results = await invoke<RenameResult[]>("batch_strip_all_songs", { customWords: customWords() });
      if (unmountedRef.current) return;
      const successCount = results.filter((r) => r.success).length;
      const failedCount = results.filter((r) => !r.success).length;
      const errors = results
        .filter((r) => !r.success && r.error)
        .map((r) => `${r.original || "(无标题)"}: ${r.error}`)
        .slice(0, 5)
        .join("\n");
      await loadSongs();
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        results: { success: successCount, failed: failedCount, skipped: 0 },
        error: failedCount > 0 ? errors : null,
      } : null);
      if (failedCount > 0) onError(`去词有 ${failedCount} 项失败：\n${errors}`);
    } catch (e) {
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        error: String(e),
        results: { success: 0, failed: songs.length, skipped: 0 },
      } : null);
      onError(`去词失败：${e}`);
    }
  }

  // 修复所有歌曲编码 — 先预览（筛选 bit 0=1 的歌曲），再执行
  // 点击后立即弹出弹窗显示"正在扫描设备存储…"，避免扫描期间无 UI 反馈
  async function repairAllEncoding() {
    setBatchMenuOpen(false);
    // 立即弹出弹窗（running 阶段，total=0 表示扫描中），让用户看到响应
    setBatchPreview({
      phase: "running",
      title: "正在扫描编码错误…",
      previews: [],
      progress: { current: 0, total: 0, currentTitle: "" },
      results: null,
      error: null,
    });
    try {
      const previews = await invoke<PreviewResult[]>("preview_repair_encoding");
      if (unmountedRef.current) return;
      if (previews.length === 0) {
        setBatchPreview(null);
        onError("编码检测完成：所有歌曲编码正常，无需修复");
        return;
      }
      // 扫描完成：切换到预览阶段，显示需修复的歌曲列表
      setBatchPreview({
        phase: "preview",
        title: `修复编码（${previews.length} 首需修复）`,
        previews,
        progress: null,
        results: null,
        error: null,
      });
    } catch (e) {
      setBatchPreview(null);
      onError(`编码检测失败：${e}`);
    }
  }

  /** 执行全部编码修复（预览确认后调用） */
  async function executeRepairAllEncoding() {
    setBatchPreview({
      phase: "running",
      title: "正在修复编码…",
      previews: [],
      progress: { current: 0, total: 0, currentTitle: "" },
      results: null,
      error: null,
    });
    try {
      const results = await invoke<RenameResult[]>("repair_all_songs_encoding");
      if (unmountedRef.current) return;
      const successCount = results.filter((r) => r.success).length;
      const failedCount = results.filter((r) => !r.success).length;
      const errors = results
        .filter((r) => !r.success && r.error)
        .map((r) => `${r.original || "(无标题)"}: ${r.error}`)
        .slice(0, 5)
        .join("\n");
      await loadSongs();
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        results: { success: successCount, failed: failedCount, skipped: 0 },
        error: failedCount > 0 ? errors : null,
      } : null);
      if (failedCount > 0) {
        onError(`编码修复完成：成功 ${successCount} 项，失败 ${failedCount} 项\n${errors}`);
      }
    } catch (e) {
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        error: String(e),
        results: { success: 0, failed: 0, skipped: 0 },
      } : null);
      onError(`修复失败：${e}`);
    }
  }

  // 批量修复选中歌曲的编码 — 弹出确认对话框，确认后直接执行（无需预览，title 不变）
  async function batchRepairSelectedEncoding() {
    setBatchMenuOpen(false);
    if (selected.size === 0) return;
    const items = selectedItems();
    onConfirm({
      title: "修复编码（选中）",
      message: `将修复 ${items.length} 首选中歌曲的中文编码（清 bit 0，恢复正确显示）。继续？`,
      onConfirm: async () => {
        await executeBatchRepairSelected(items);
      },
    });
  }

  /** 执行批量修复选中歌曲编码（确认对话框后调用） */
  async function executeBatchRepairSelected(items: { fileNo: number; memUnit: number; title: string }[]) {
    setBatchPreview({
      phase: "running",
      title: "正在修复编码（选中）…",
      previews: [],
      progress: { current: 0, total: items.length, currentTitle: "" },
      results: null,
      error: null,
    });
    try {
      const results = await invoke<RenameResult[]>("repair_selected_encoding", { items });
      if (unmountedRef.current) return;
      const successCount = results.filter((r) => r.success).length;
      const failedCount = results.filter((r) => !r.success).length;
      const errors = results
        .filter((r) => !r.success && r.error)
        .map((r) => `${r.original || "(无标题)"}: ${r.error}`)
        .slice(0, 5)
        .join("\n");
      setSelected(new Set());
      await loadSongs();
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        results: { success: successCount, failed: failedCount, skipped: 0 },
        error: failedCount > 0 ? errors : null,
      } : null);
      if (failedCount > 0) {
        onError(`编码修复完成：成功 ${successCount} 项，失败 ${failedCount} 项\n${errors}`);
      }
    } catch (e) {
      if (unmountedRef.current) return;
      setBatchPreview((prev) => prev ? {
        ...prev,
        phase: "done",
        error: String(e),
        results: { success: 0, failed: items.length, skipped: 0 },
      } : null);
      onError(`修复失败：${e}`);
    }
  }

  return (
    <div className="pane">
      <BatchToolbar
        selectedCount={selected.size}
        onSelectAll={selectAll}
        onClearSelection={clearSelection}
        onBatchDelete={batchDelete}
        onBatchAddToPlaylist={batchAddToPlaylist}
        onRefresh={loadSongs}
        loading={deleting || loading}
        showAddToPlaylist
        batchMenuOpen={batchMenuOpen}
        setBatchMenuOpen={setBatchMenuOpen}
        onBatchSlugSelected={batchSlugSelected}
        onBatchStripSelected={batchStripSelected}
        onBatchSlugAll={batchSlugAll}
        onBatchStripAll={batchStripAll}
        onRepairAllEncoding={repairAllEncoding}
        onRepairSelectedEncoding={batchRepairSelectedEncoding}
      />
      <FilterBar
        search={search}
        setSearch={setSearch}
        sortBy={sortBy}
        setSortBy={setSortBy}
        total={songs.length}
        shown={filtered.length}
      />
      {loading && <div className="loading-text">加载中…</div>}
      {!loading && pageItems.length === 0 && (
        <div className="empty-text">{songs.length === 0 ? "暂无歌曲，拖拽 MP3 文件上传" : "无匹配项"}</div>
      )}
      {!loading && pageItems.length > 0 && (
        <>
          {/* 移动端：卡片列表（替代表格） */}
          {isMobile ? (
            <div className="mobile-song-list">
              {pageItems.map((s, i) => {
                const k = songKey(s);
                const isSelected = selected.has(k);
                const isActive = i === selectedIdx;
                const title = s.title || s.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "(无标题)";
                return (
                  <div
                    key={k}
                    className={`mobile-song-card ${isActive ? "active" : ""} ${isSelected ? "checked" : ""}`}
                    onClick={(e) => {
                      setSelectedIdx(i);
                      if (e.shiftKey && lastClickedIdxRef.current !== null) {
                        const start = Math.min(lastClickedIdxRef.current, i);
                        const end = Math.max(lastClickedIdxRef.current, i);
                        setSelected(new Set(pageItems.slice(start, end + 1).map(songKey)));
                      } else {
                        setSelected((prev) => {
                          const next = new Set(prev);
                          if (next.has(k)) next.delete(k);
                          else next.add(k);
                          return next;
                        });
                        lastClickedIdxRef.current = i;
                      }
                    }}
                    onDoubleClick={() => onPlaySong(s)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setContextMenu({ x: e.clientX, y: e.clientY, song: s });
                    }}
                  >
                    <div className="mobile-song-info">
                      <span className="mobile-song-title">{title}</span>
                      <span className="mobile-song-meta">
                        {s.artist || "—"}
                      </span>
                    </div>
                    <span className={`mobile-song-badge mem-${s.mem_unit}`}>
                      {s.mem_unit === 0 ? "内置" : "SD"}
                    </span>
                    <span className="mobile-song-time">
                      {s.time > 0 ? formatTime(s.time) : "—"}
                    </span>
                  </div>
                );
              })}
            </div>
          ) : (
          <div className="song-table-wrap" ref={virtual.containerRef}>
            <table className="song-table">
              <thead>
                <tr>
                  <th className="col-title">标题</th>
                  <th className="col-artist">艺术家</th>
                  <th className="col-album">专辑</th>
                  <th className="col-time">时长</th>
                  <th className="col-size">大小</th>
                  <th className="col-bitrate">比特率</th>
                  <th className="col-mem">存储</th>
                </tr>
              </thead>
              <tbody>
                {/* 虚拟滚动：顶部占位行 */}
                {virtual.offsetY > 0 && (
                  <tr style={{ height: virtual.offsetY }} aria-hidden="true"><td colSpan={7} /></tr>
                )}
                {/* 只渲染可见行 */}
                {pageItems.slice(virtual.startIndex, virtual.endIndex).map((s, vi) => {
                  const i = virtual.startIndex + vi;
                  const k = songKey(s);
                  const isSelected = selected.has(k);
                  const isActive = i === selectedIdx;
                  const title = s.title || s.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "(无标题)";
                  return (
                    <tr
                      key={k}
                      className={`${isActive ? "active" : ""} ${isSelected ? "checked" : ""}`}
                      onClick={(e) => {
                        setSelectedIdx(i);
                        if (e.shiftKey && lastClickedIdxRef.current !== null) {
                          // Shift+点击：从上次点击位置到当前行范围选择
                          const start = Math.min(lastClickedIdxRef.current, i);
                          const end = Math.max(lastClickedIdxRef.current, i);
                          setSelected(new Set(pageItems.slice(start, end + 1).map(songKey)));
                        } else {
                          // 单击即切换该行选中状态（无需修饰键）
                          // macOS Ctrl+Click 会被系统转为右键，由 onContextMenu 处理
                          setSelected((prev) => {
                            const next = new Set(prev);
                            if (next.has(k)) next.delete(k);
                            else next.add(k);
                            return next;
                          });
                          lastClickedIdxRef.current = i;
                        }
                      }}
                      onDoubleClick={() => onPlaySong(s)}
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setContextMenu({ x: e.clientX, y: e.clientY, song: s });
                      }}
                    >
                      <td className="col-title">
                        <span className={`row-check ${isSelected ? "checked" : ""}`} />
                        {title}
                      </td>
                      <td className="col-artist">{s.artist || "—"}</td>
                      <td className="col-album">{s.album || "—"}</td>
                      <td className="col-time">{s.time > 0 ? formatTime(s.time) : "—"}</td>
                      <td className="col-size">{formatSize(s.size)}</td>
                      <td className="col-bitrate">{s.bit_rate > 0 ? `${s.bit_rate >> 7}kbps` : "—"}</td>
                      <td className="col-mem">
                        <span className={`mem-badge mem-${s.mem_unit}`}>
                          {s.mem_unit === 0 ? "内置" : "SD 卡"}
                        </span>
                      </td>
                    </tr>
                  );
                })}
                {/* 虚拟滚动：底部占位行 */}
                {virtual.totalHeight - virtual.offsetY - (virtual.endIndex - virtual.startIndex) * 28 > 0 && (
                  <tr style={{ height: virtual.totalHeight - virtual.offsetY - (virtual.endIndex - virtual.startIndex) * 28 }} aria-hidden="true"><td colSpan={7} /></tr>
                )}
              </tbody>
            </table>
          </div>
          )}
          {paginate && <Pagination page={page} totalPages={totalPages} setPage={setPage} />}
        </>
      )}
      {showPlaylistPicker && (
        <PlaylistPicker
          playlists={playlistsCache}
          onPick={addToSpecificPlaylist}
          onClose={() => setShowPlaylistPicker(false)}
          onRefresh={onRefreshPlaylists}
        />
      )}
      <ContextMenu
        state={contextMenu}
        onClose={() => setContextMenu(null)}
        onPlay={(s) => onPlaySong(s)}
        onAddToPlaylist={(s) => addToPlaylistSingle(s)}
        onShowDetail={(s) => onShowDetail(s)}
        onDownload={(s) => downloadSong(s)}
        onDelete={(s) => deleteSingle(s)}
        onRename={(s) => setRenameTarget(s)}
        onRepairEncoding={(s) => repairSingleEncoding(s)}
      />
        {renameTarget && (
          <RenameModal
            song={renameTarget}
            onClose={() => setRenameTarget(null)}
            onRenamed={() => loadSongs()}
            onError={(msg) => onError(msg)}
          />
        )}
        {batchPreview && (
          <BatchPreviewModal
            state={batchPreview}
            onClose={() => setBatchPreview(null)}
            onConfirmSlug={(items) => executeBatchSlug(items)}
            onConfirmStrip={(items) => executeBatchStrip(items)}
            onConfirmSlugAll={() => executeBatchSlugAll()}
            onConfirmStripAll={() => executeBatchStripAll()}
            onConfirmRepairAll={() => executeRepairAllEncoding()}
          />
        )}
    </div>
  );
}

// ============================================================================
// 歌单选择器
// ============================================================================

function PlaylistPicker({
  playlists,
  onPick,
  onClose,
  onRefresh,
}: {
  playlists: PlaylistInfo[];
  onPick: (p: PlaylistInfo) => void;
  onClose: () => void;
  onRefresh?: () => void;
}) {
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>选择目标歌单</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="modal-body">
          {playlists.length === 0 ? (
            <div className="empty-text">暂无歌单，请先创建</div>
          ) : (
            <div className="song-table-wrap">
              <table className="song-table">
                <thead>
                  <tr>
                    <th className="col-title">歌单名</th>
                    <th className="col-size">大小</th>
                    <th className="col-mem">存储</th>
                  </tr>
                </thead>
                <tbody>
                  {playlists.map((p) => {
                    const title = p.title || p.name?.replace(/^D:\\/, "").replace(/\.pls$/i, "") || "(未命名)";
                    return (
                      <tr
                        key={`${p.mem_unit}:${p.file_no}`}
                        onClick={() => onPick(p)}
                      >
                        <td className="col-title">{title}</td>
                        <td className="col-size">{formatSize(p.size)}</td>
                        <td className="col-mem">
                          <span className={`mem-badge mem-${p.mem_unit}`}>
                            {p.mem_unit === 0 ? "内置" : "SD 卡"}
                          </span>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
        {onRefresh && (
          <div className="modal-footer">
            <button className="modal-btn refresh" onClick={onRefresh}>刷新歌单</button>
            <button className="modal-btn" onClick={onClose}>取消</button>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// 歌单页面（自行加载数据，不缓存）
// ============================================================================

function PlaylistsPane({
  onError,
  paginate,
  refreshKey,
  onPlaySong,
  onShowDetail,
  onConfirm,
}: {
  onError: (msg: string | null) => void;
  paginate: boolean;
  refreshKey: number;
  onPlaySong: (s: SongInfo) => void;
  onShowDetail: (s: SongInfo) => void;
  onConfirm: (s: ConfirmState) => void;
}) {
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [sortBy, setSortBy] = useState<SortKey>("name");
  const [activePlaylist, setActivePlaylist] = useState<PlaylistInfo | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const lastClickedIdxRef = useRef<number | null>(null);
  /** 标记组件是否已卸载——卸载后忽略所有异步回调，防止 loading 卡死 */
  const unmountedRef = useRef(false);

  const loadPlaylists = useCallback(async () => {
    setLoading(true);
    try {
      const all: PlaylistInfo[] = [];
      try {
        const internal = await invoke<PlaylistInfo[]>("list_playlists", { memUnit: 0 });
        if (!unmountedRef.current) all.push(...internal.map((p) => ({ ...p, mem_unit: 0 })));
      } catch {}
      try {
        const sd = await invoke<PlaylistInfo[]>("list_playlists", { memUnit: 1 });
        if (!unmountedRef.current) all.push(...sd.map((p) => ({ ...p, mem_unit: 1 })));
      } catch {}
      if (unmountedRef.current) return;
      setPlaylists(all);
    } catch (e) {
      if (unmountedRef.current) return;
      onError(`加载歌单失败: ${e}`);
    }
    if (!unmountedRef.current) setLoading(false);
  }, [onError]);

  // 组件卸载时设置 unmounted 标志，防止异步回调更新已卸载的组件
  useEffect(() => {
    unmountedRef.current = false;
    return () => {
      unmountedRef.current = true;
    };
  }, []);

  useEffect(() => {
    loadPlaylists();
  }, [loadPlaylists, refreshKey]);

  const { filtered, pageItems, page, totalPages, setPage } = useFilteredPaged(
    playlists,
    {
      search,
      sortBy,
      getSearchText: (p) => `${p.title} ${p.name}`,
      getSortValue: (p, key) => {
        if (key === "name") return p.title || p.name;
        if (key === "size") return p.size;
        return 0;
      },
    },
    paginate,
    10
  );

  const { selectedIdx, setSelectedIdx } = useListNavigation(pageItems);

  function playlistKey(p: PlaylistInfo): string {
    return `${p.mem_unit}:${p.file_no}`;
  }

  function selectAll() {
    setSelected(new Set(filtered.map(playlistKey)));
  }

  function clearSelection() {
    setSelected(new Set());
  }

  async function batchDelete() {
    if (selected.size === 0) return;
    const count = selected.size;
    onConfirm({
      title: "批量删除歌单",
      message: `确认删除选中的 ${count} 个歌单？此操作不可撤销。`,
      danger: true,
      onConfirm: async () => {
        setDeleting(true);
        let okCount = 0;
        let failCount = 0;
        for (const p of playlists) {
          const k = playlistKey(p);
          if (selected.has(k)) {
            try {
              await invoke("delete_song", { fileNo: p.file_no, memUnit: p.mem_unit });
              okCount++;
            } catch {
              failCount++;
            }
          }
        }
        setSelected(new Set());
        onError(`删除完成：成功 ${okCount} 项${failCount > 0 ? `，失败 ${failCount} 项` : ""}`);
        setDeleting(false);
        await loadPlaylists();
      },
    });
  }

  if (activePlaylist) {
    return (
      <PlaylistDetail
        playlist={activePlaylist}
        onBack={() => setActivePlaylist(null)}
        onError={onError}
        onPlaySong={onPlaySong}
        onShowDetail={onShowDetail}
      />
    );
  }

  return (
    <div className="pane">
      <BatchToolbar
        selectedCount={selected.size}
        onSelectAll={selectAll}
        onClearSelection={clearSelection}
        onBatchDelete={batchDelete}
        onBatchAddToPlaylist={async () => {}}
        onRefresh={loadPlaylists}
        loading={deleting || loading}
        showAddToPlaylist={false}
      />
      <FilterBar
        search={search}
        setSearch={setSearch}
        sortBy={sortBy}
        setSortBy={setSortBy}
        total={playlists.length}
        shown={filtered.length}
      />
      {loading && <div className="loading-text">加载中…</div>}
      {!loading && pageItems.length === 0 && (
        <div className="empty-text">{playlists.length === 0 ? "暂无歌单" : "无匹配项"}</div>
      )}
      {!loading && pageItems.length > 0 && (
        <>
          <div className="song-table-wrap">
            <table className="song-table">
              <thead>
                <tr>
                  <th className="col-title">歌单名</th>
                  <th className="col-size">大小</th>
                  <th className="col-mem">存储</th>
                </tr>
              </thead>
              <tbody>
                {pageItems.map((p, i) => {
                  const k = playlistKey(p);
                  const isSelected = selected.has(k);
                  const isActive = i === selectedIdx;
                  const title = p.title || p.name?.replace(/^D:\\/, "").replace(/\.pls$/i, "") || "(未命名)";
                  return (
                    <tr
                      key={k}
                      className={`${isActive ? "active" : ""} ${isSelected ? "checked" : ""}`}
                      onClick={(e) => {
                        setSelectedIdx(i);
                        if (e.shiftKey && lastClickedIdxRef.current !== null) {
                          const start = Math.min(lastClickedIdxRef.current, i);
                          const end = Math.max(lastClickedIdxRef.current, i);
                          setSelected(new Set(pageItems.slice(start, end + 1).map(playlistKey)));
                        } else if (e.ctrlKey || e.metaKey) {
                          setSelected((prev) => {
                            const next = new Set(prev);
                            if (next.has(k)) next.delete(k);
                            else next.add(k);
                            return next;
                          });
                          lastClickedIdxRef.current = i;
                        } else {
                          setSelected((prev) => {
                            if (prev.size === 1 && prev.has(k)) return new Set();
                            return new Set([k]);
                          });
                          lastClickedIdxRef.current = i;
                        }
                      }}
                      onDoubleClick={() => setActivePlaylist(p)}
                    >
                      <td className="col-title">
                        <span className={`row-check ${isSelected ? "checked" : ""}`} />
                        {title}
                      </td>
                      <td className="col-size">{formatSize(p.size)}</td>
                      <td className="col-mem">
                        <span className={`mem-badge mem-${p.mem_unit}`}>
                          {p.mem_unit === 0 ? "内置" : "SD 卡"}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          {paginate && <Pagination page={page} totalPages={totalPages} setPage={setPage} />}
        </>
      )}
      <button className="batch-btn" onClick={() => setShowCreate(true)} style={{ alignSelf: "flex-start" }}>
        + 新建歌单
      </button>
      {showCreate && (
        <CreatePlaylistModal
          onClose={() => setShowCreate(false)}
          onCreated={async () => {
            setShowCreate(false);
            await loadPlaylists();
          }}
          onError={onError}
        />
      )}
    </div>
  );
}

// ============================================================================
// 新建歌单模态框
// ============================================================================

function CreatePlaylistModal({
  onClose,
  onCreated,
  onError,
}: {
  onClose: () => void;
  onCreated: () => void;
  onError: (msg: string | null) => void;
}) {
  const [name, setName] = useState("");
  const [memUnit, setMemUnit] = useState(0);
  const [creating, setCreating] = useState(false);

  async function create() {
    if (!name.trim()) return;
    setCreating(true);
    try {
      await invoke("create_playlist", { name: name.trim(), memUnit });
      onError(`歌单 "${name.trim()}" 已创建`);
      onCreated();
    } catch (e) {
      onError(`创建失败：${e}`);
    }
    setCreating(false);
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>新建歌单</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="modal-body">
          <input
            className="filter-search"
            style={{ width: "100%", maxWidth: "none", padding: "6px 10px" }}
            type="text"
            placeholder="歌单名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
          />
          <div className="mem-switch" style={{ marginLeft: 0, marginTop: 4 }}>
            <button className={memUnit === 0 ? "active" : ""} onClick={() => setMemUnit(0)}>内置</button>
            <button className={memUnit === 1 ? "active" : ""} onClick={() => setMemUnit(1)}>SD 卡</button>
          </div>
        </div>
        <div className="modal-footer">
          <button className="modal-btn" onClick={onClose}>取消</button>
          <button className="modal-btn" onClick={create} disabled={creating || !name.trim()}>
            {creating ? "创建中…" : "创建"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 歌单详情页
// ============================================================================

function PlaylistDetail({
  playlist,
  onBack,
  onError,
  onPlaySong,
  onShowDetail,
}: {
  playlist: PlaylistInfo;
  onBack: () => void;
  onError: (msg: string | null) => void;
  onPlaySong: (s: SongInfo) => void;
  onShowDetail: (s: SongInfo) => void;
}) {
  const [songs, setSongs] = useState<SongInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState>(null);
  const [renameTarget, setRenameTarget] = useState<SongInfo | null>(null);
  /** 标记组件是否已卸载——卸载后忽略所有异步回调，防止 loading 卡死 */
  const unmountedRef = useRef(false);

  const loadSongs = useCallback(async () => {
    setLoading(true);
    onError(null);
    try {
      const result = await invoke<SongInfo[]>("list_playlist_songs", {
        playlistFileNo: playlist.file_no,
        memUnit: playlist.mem_unit,
      });
      if (unmountedRef.current) return;
      setSongs(result);
    } catch (e) {
      if (unmountedRef.current) return;
      onError(`加载歌单内容失败: ${e}`);
    }
    if (!unmountedRef.current) setLoading(false);
  }, [playlist, onError]);

  // 组件卸载时设置 unmounted 标志，防止异步回调更新已卸载的组件
  useEffect(() => {
    unmountedRef.current = false;
    return () => {
      unmountedRef.current = true;
    };
  }, []);

  useEffect(() => {
    loadSongs();
  }, [loadSongs]);

  const title = playlist.title || playlist.name?.replace(/^D:\\/, "").replace(/\.pls$/i, "") || "(未命名)";

  return (
    <div className="pane">
      <div className="detail-header">
        <button className="back-btn" onClick={onBack} title="返回">
          <svg width="12" height="12" viewBox="0 0 14 14" fill="none" style={{ flexShrink: 0 }}>
            <path d="M9 1 L3 7 L9 13" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" fill="none" />
          </svg>
          <span>返回</span>
        </button>
        <h2>{title}</h2>
        <span className={`mem-badge mem-${playlist.mem_unit}`}>
          {playlist.mem_unit === 0 ? "内置" : "SD 卡"}
        </span>
        <span className="count-badge">{songs.length} 首</span>
        <button className="refresh-btn" onClick={loadSongs} disabled={loading}>刷新</button>
      </div>
      {loading && <div className="loading-text">加载中…</div>}
      {!loading && songs.length === 0 && <div className="empty-text">歌单为空</div>}
      {!loading && songs.length > 0 && (
        <div className="song-table-wrap">
          <table className="song-table">
            <thead>
              <tr>
                <th className="col-title">标题</th>
                <th className="col-artist">艺术家</th>
                <th className="col-time">时长</th>
                <th className="col-size">大小</th>
                <th className="col-mem">存储</th>
              </tr>
            </thead>
            <tbody>
              {songs.map((s) => {
                const t = s.title || s.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "(无标题)";
                return (
                  <tr
                    key={`${s.mem_unit}:${s.file_no}`}
                    onDoubleClick={() => onPlaySong(s)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setContextMenu({ x: e.clientX, y: e.clientY, song: s });
                    }}
                  >
                    <td className="col-title">{t}</td>
                    <td className="col-artist">{s.artist || "—"}</td>
                    <td className="col-time">{s.time > 0 ? formatTime(s.time) : "—"}</td>
                    <td className="col-size">{formatSize(s.size)}</td>
                    <td className="col-mem">
                      <span className={`mem-badge mem-${s.mem_unit}`}>
                        {s.mem_unit === 0 ? "内置" : "SD 卡"}
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
      <ContextMenu
        state={contextMenu}
        onClose={() => setContextMenu(null)}
        onPlay={(s) => onPlaySong(s)}
        onAddToPlaylist={() => {
          onError("歌单内歌曲无需再添加到歌单");
        }}
        onShowDetail={(s) => onShowDetail(s)}
        onDownload={() => {
          onError("请在歌曲页面下载");
        }}
        onDelete={() => {
          onError("请在歌曲页面删除");
        }}
        onRename={(s) => setRenameTarget(s)}
        onRepairEncoding={async (s) => {
          try {
            await invoke("repair_song_encoding", {
              fileNo: s.file_no,
              memUnit: s.mem_unit,
            });
            onError("编码已修复");
            await loadSongs();
          } catch (e) {
            onError(`修复失败：${e}`);
          }
        }}
      />
        {renameTarget && (
          <RenameModal
            song={renameTarget}
            onClose={() => setRenameTarget(null)}
            onRenamed={() => loadSongs()}
            onError={(msg) => onError(msg)}
          />
        )}
    </div>
  );
}

// ============================================================================
// 设备信息页面（自行加载数据）
// ============================================================================

function DeviceInfoPane({
  refreshKey,
  onRefreshStorage,
}: {
  refreshKey: number;
  onRefreshStorage: () => void;
}) {
  const [internal, setInternal] = useState<StorageInfo | null>(null);
  const [sd, setSd] = useState<StorageInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadStorage = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const i = await invoke<StorageInfo>("get_storage", { memUnit: 0 });
      setInternal(i);
      const s = await invoke<StorageInfo>("get_storage", { memUnit: 1 }).catch(() => null);
      setSd(s);
    } catch (e) {
      setError(`加载失败: ${e}`);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    loadStorage();
  }, [loadStorage, refreshKey]);

  return (
    <div className="pane">
      <div className="pane-header">
        <h2>设备信息</h2>
        <button className="refresh-btn" onClick={() => { loadStorage(); onRefreshStorage(); }} disabled={loading}>刷新</button>
      </div>
      {error && <div className="error-banner">{error}</div>}
      {loading && <div className="loading-text">加载中…</div>}
      {!loading && internal && (
        <div className="storage-grid">
          <StorageCard info={internal} title="内置存储" />
          {sd && <StorageCard info={sd} title="SD 卡" />}
          {!sd && (
            <div className="storage-card empty">
              <div className="storage-title">SD 卡</div>
              <div className="storage-empty">未插入</div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function StorageCard({ info, title }: { info: StorageInfo; title: string }) {
  const usedPct = info.size > 0 ? (info.used / info.size) * 100 : 0;
  return (
    <div className="storage-card">
      <div className="storage-title">{title}</div>
      <div className="storage-size">{info.size_formatted}</div>
      <div className="storage-bar">
        <div
          className="storage-bar-fill"
          style={{ width: `${usedPct}%` }}
        />
      </div>
      <div className="storage-detail">
        已用 {formatSize(info.used)} / 可用 {formatSize(info.free)}
      </div>
    </div>
  );
}

// ============================================================================
// 上传页面
// ============================================================================

function UploadPane({
  onError,
  onUploaded,
  onPickFiles,
  isUploading,
}: {
  onError: (msg: string | null) => void;
  onUploaded: () => void;
  onPickFiles: (paths: string[], memUnit: number) => Promise<void>;
  isUploading: boolean;
}) {
  const [memUnit, setMemUnit] = useState(0);

  async function pickAndUpload() {
    onError(null);
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: true,
        filters: [{ name: "MP3", extensions: ["mp3"] }],
      });
      if (!selected) return;
      const paths = Array.isArray(selected)
        ? selected.map((s) => (typeof s === "string" ? s : (s as { path: string }).path))
        : [typeof selected === "string" ? selected : (selected as { path: string }).path];
      // 复用父组件的 handleDrop，触发统一的传输对话框
      await onPickFiles(paths, memUnit);
      onUploaded();
    } catch (e) {
      onError(`上传失败: ${e}`);
    }
  }

  return (
    <div className="pane">
      <div className="pane-header">
        <h2>上传</h2>
        <div className="mem-switch">
          <button className={memUnit === 0 ? "active" : ""} onClick={() => setMemUnit(0)}>内置</button>
          <button className={memUnit === 1 ? "active" : ""} onClick={() => setMemUnit(1)}>SD 卡</button>
        </div>
      </div>
      <div className="upload-zone">
        <p className="upload-hint">选择 MP3 文件，或直接拖拽到任意位置</p>
        <button
          className="upload-btn"
          onClick={pickAndUpload}
          disabled={isUploading}
        >
          {isUploading ? "上传中…" : "选择文件"}
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// 关于页面
// ============================================================================

function AboutPane() {
  const [version, setVersion] = useState<string>("");
  useEffect(() => {
    // 动态获取应用版本（从 tauri.conf.json 读取）
    import("@tauri-apps/api/app").then(({ getVersion }) => {
      getVersion().then(setVersion).catch(() => setVersion("未知"));
    });
  }, []);
  return (
    <div className="pane">
      <div className="pane-header">
        <h2>关于</h2>
      </div>
      <div className="about-content">
        <div className="about-title-line">
          <span className="about-name">cyrio</span>
          <span className="about-version">v{version || "…"}</span>
        </div>
        <div className="about-info">
          <div className="about-row">
            <span className="about-label">作者</span>
            <span className="about-value">cyanteam</span>
          </div>
          <div className="about-row">
            <span className="about-label">GitHub</span>
            <span className="about-value">github.com/cyanteam</span>
          </div>
          <div className="about-row">
            <span className="about-label">邮箱</span>
            <span className="about-value">qtof@qq.com</span>
          </div>
          <div className="about-row">
            <span className="about-label">协议</span>
            <span className="about-value">Rio Receiver USB 逆向实现</span>
          </div>
          <div className="about-row">
            <span className="about-label">支持</span>
            <span className="about-value">Rio S50 / S30S · 跨存储歌单 · UTF-8 编码</span>
          </div>
          <div className="about-row">
            <span className="about-label">技术</span>
            <span className="about-value">Rust + Tauri 2.0 + egui</span>
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 设置页面
// ============================================================================

/** 设置项开关行：左标签 + 右开关 */
function SettingsToggle({
  label,
  description,
  value,
  onChange,
  disabled,
  disabledHint,
}: {
  label: string;
  description?: string;
  value: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  disabledHint?: string;
}) {
  return (
    <div className={`settings-row ${disabled ? "disabled" : ""}`}>
      <div className="settings-row-text">
        <div className="settings-row-label">{label}</div>
        {description && <div className="settings-row-desc">{description}</div>}
        {disabled && disabledHint && <div className="settings-row-hint">{disabledHint}</div>}
      </div>
      <button
        type="button"
        className={`settings-switch ${value ? "on" : ""}`}
        onClick={() => !disabled && onChange(!value)}
        disabled={disabled}
        aria-pressed={value}
      >
        <span className="settings-switch-knob" />
      </button>
    </div>
  );
}

function SettingsPane({
  settings,
  onSave,
  onError,
}: {
  settings: AppSettings;
  onSave: (s: AppSettings) => void;
  onError: (msg: string) => void;
}) {
  // 本地草稿：编辑期间不立即写盘，点击保存才提交
  const [draft, setDraft] = useState<AppSettings>(settings);
  const [saved, setSaved] = useState(false);

  // 当父级 settings 变化时同步草稿（外部重置）
  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  function update(patch: Partial<AppSettings>) {
    setDraft((prev) => ({ ...prev, ...patch }));
    setSaved(false);
  }

  function save() {
    onSave(draft);
    setSaved(true);
    onError("设置已保存");
    setTimeout(() => setSaved(false), 1800);
  }

  function reset() {
    setDraft({ ...DEFAULT_SETTINGS });
    setSaved(false);
  }

  return (
    <div className="pane">
      <div className="pane-header">
        <h2>设置</h2>
        <div className="settings-actions">
          <button className="modal-btn" onClick={reset}>重置为默认</button>
          <button className="modal-btn primary" onClick={save}>
            {saved ? "已保存" : "保存设置"}
          </button>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section-title">上传文本处理</div>
        <SettingsToggle
          label="上传时应用 Slug（中文转拼音）"
          description="把歌曲名中的中文转为拼音格式，如「赛马」→「Sai-Ma」"
          value={draft.upload_apply_slug}
          onChange={(v) => update({ upload_apply_slug: v })}
        />
        <SettingsToggle
          label="上传时应用去词"
          description="移除歌曲名中常见的无关词汇（Hi-Res、4K、原创、引号歌词片段等）"
          value={draft.upload_apply_strip}
          onChange={(v) => update({ upload_apply_strip: v })}
        />
        <div className={`settings-subsection ${draft.upload_apply_strip ? "" : "off"}`}>
          <div className="settings-subsection-title">去词规则</div>
          <SettingsToggle
            label="去除括号内容"
            description="移除 ()、【】、[] 包裹的内容"
            value={draft.strip_parentheses}
            onChange={(v) => update({ strip_parentheses: v })}
            disabled={!draft.upload_apply_strip}
          />
          <SettingsToggle
            label="去除引号歌词片段"
            description='移除结尾中文/英文引号包裹的歌词片段，如「"在百万级播音室大声听"」'
            value={draft.strip_quotes}
            onChange={(v) => update({ strip_quotes: v })}
            disabled={!draft.upload_apply_strip}
          />
          <SettingsToggle
            label="去除音质/分辨率标签"
            description="移除 Hi-Res、无损、4K、高清、原创 等 B 站下载常见标签"
            value={draft.strip_quality_tags}
            onChange={(v) => update({ strip_quality_tags: v })}
            disabled={!draft.upload_apply_strip}
          />
          <div className="settings-custom-words">
            <div className="settings-row-label">自定义停用词</div>
            <div className="settings-row-desc">每行一个词，匹配到的内容将被移除</div>
            <textarea
              className="settings-textarea"
              value={draft.custom_stop_words}
              onChange={(e) => update({ custom_stop_words: e.target.value })}
              placeholder={"例如：\n原唱\n完整版\n现场版"}
              rows={5}
              disabled={!draft.upload_apply_strip}
            />
          </div>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section-title">关于设置</div>
        <div className="settings-row-desc">
          设置存储在浏览器 localStorage 中，卸载或清理浏览器数据将丢失。
          上传文本处理仅作用于新上传的歌曲；对已存在的歌曲，请在歌曲页右键「重命名」或在批量工具栏「更多」中处理。
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 工具函数
// ============================================================================

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)}MB`;
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

// ============================================================================
// 右键菜单组件
// ============================================================================

function ContextMenu({
  state,
  onClose,
  onPlay,
  onAddToPlaylist,
  onShowDetail,
  onDownload,
  onDelete,
  onRename,
  onRepairEncoding,
}: {
  state: ContextMenuState;
  onClose: () => void;
  onPlay: (s: SongInfo) => void;
  onAddToPlaylist: (s: SongInfo) => void;
  onShowDetail: (s: SongInfo) => void;
  onDownload: (s: SongInfo) => void;
  onDelete: (s: SongInfo) => void;
  onRename: (s: SongInfo) => void;
  onRepairEncoding: (s: SongInfo) => void;
}) {
  useEffect(() => {
    if (!state) return;
    function onClickAway() {
      onClose();
    }
    function onEsc(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    const timer = setTimeout(() => {
      window.addEventListener("click", onClickAway);
      window.addEventListener("contextmenu", onClickAway);
      window.addEventListener("keydown", onEsc);
    }, 0);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("click", onClickAway);
      window.removeEventListener("contextmenu", onClickAway);
      window.removeEventListener("keydown", onEsc);
    };
  }, [state, onClose]);

  if (!state) return null;

  // 防止菜单超出窗口
  const x = Math.min(state.x, window.innerWidth - 200);
  const y = Math.min(state.y, window.innerHeight - 280);

  return (
    <div className="context-menu" style={{ left: x, top: y }}>
      <button onClick={() => { onPlay(state.song); onClose(); }}>播放试听</button>
      <button onClick={() => { onAddToPlaylist(state.song); onClose(); }}>加入歌单</button>
      <button onClick={() => { onShowDetail(state.song); onClose(); }}>详细信息</button>
      <hr />
      <button onClick={() => { onRename(state.song); onClose(); }}>重命名</button>
      <button onClick={() => { onDownload(state.song); onClose(); }}>下载到本地</button>
      <button onClick={() => { onRepairEncoding(state.song); onClose(); }}>修复编码</button>
      <hr />
      <button className="danger" onClick={() => { onDelete(state.song); onClose(); }}>删除</button>
    </div>
  );
}

// ============================================================================
// 重命名对话框
// ============================================================================

/** 重命名单首歌曲：调用 rename_song 命令，更新 title 字段 */
function RenameModal({
  song,
  onClose,
  onRenamed,
  onError,
}: {
  song: SongInfo;
  onClose: () => void;
  onRenamed: () => void;
  onError: (msg: string) => void;
}) {
  const original = song.title || song.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "";
  const [value, setValue] = useState(original);
  const [submitting, setSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // 挂载后聚焦并选中所有文字
  useEffect(() => {
    const t = setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 50);
    return () => clearTimeout(t);
  }, []);

  async function submit() {
    const trimmed = value.trim();
    if (!trimmed) {
      onError("标题不能为空");
      return;
    }
    if (trimmed === original) {
      onClose();
      return;
    }
    setSubmitting(true);
    try {
      await invoke("rename_song", {
        fileNo: song.file_no,
        memUnit: song.mem_unit,
        newTitle: trimmed,
      });
      onError("已重命名");
      onRenamed();
      onClose();
    } catch (e) {
      onError(`重命名失败：${e}`);
    }
    setSubmitting(false);
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content rename-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>重命名</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="modal-body">
          <div className="rename-original">
            <span className="rename-label">原始标题：</span>
            <span className="rename-original-value">{original || "(无标题)"}</span>
          </div>
          <input
            ref={inputRef}
            type="text"
            className="rename-input"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !submitting) submit();
              if (e.key === "Escape") onClose();
            }}
            placeholder="输入新的歌曲标题"
            disabled={submitting}
          />
        </div>
        <div className="modal-footer">
          <button className="modal-btn" onClick={onClose} disabled={submitting}>取消</button>
          <button className="modal-btn primary" onClick={submit} disabled={submitting || !value.trim()}>
            确认
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 批量操作预览对话框（转拼音 / 去词）
// 显示操作前→后名称对比，执行时显示下载/上传进度
// ============================================================================

function BatchPreviewModal({
  state,
  onClose,
  onConfirmSlug,
  onConfirmStrip,
  onConfirmSlugAll,
  onConfirmStripAll,
  onConfirmRepairAll,
}: {
  state: NonNullable<BatchPreviewState>;
  onClose: () => void;
  onConfirmSlug: (items: { fileNo: number; memUnit: number; title: string }[]) => void;
  onConfirmStrip: (items: { fileNo: number; memUnit: number; title: string }[]) => void;
  onConfirmSlugAll: () => void;
  onConfirmStripAll: () => void;
  onConfirmRepairAll: () => void;
}) {
  // 从标题推断操作类型和范围
  const isSlug = state.title.includes("转拼音");
  const isRepair = state.title.includes("修复编码");
  const isAll = state.title.includes("全部");
  const changedItems = state.previews.filter((p) => p.changed);
  const skippedItems = state.previews.filter((p) => !p.changed);
  const isRunning = state.phase === "running";
  const isDone = state.phase === "done";

  // 执行中不允许关闭
  useEffect(() => {
    if (!isRunning) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") e.preventDefault();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isRunning]);

  function handleConfirm() {
    // 编码修复：直接执行全部修复
    if (isRepair) {
      onConfirmRepairAll();
      return;
    }
    // 从预览结果构建 items（只处理会变化的项）
    const items = changedItems.map((p) => ({
      fileNo: p.file_no,
      memUnit: p.mem_unit,
      title: p.original,
    }));
    if (isAll) {
      if (isSlug) onConfirmSlugAll();
      else onConfirmStripAll();
    } else {
      if (isSlug) onConfirmSlug(items);
      else onConfirmStrip(items);
    }
  }

  const progressPercent =
    state.progress && state.progress.total > 0
      ? (state.progress.current / state.progress.total) * 100
      : 0;

  return (
    <div
      className="confirm-overlay"
    >
      <div
        className="modal-content batch-preview-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>{state.title}</h3>
          {!isRunning && (
            <button className="modal-close" onClick={onClose}>×</button>
          )}
        </div>

        {/* 预览阶段：显示操作前→后名称对比，或编码修复的歌曲列表 */}
        {state.phase === "preview" && (
          <>
            <div className="modal-body">
              <div className="batch-preview-summary">
                <span className="batch-changed-count">
                  {isRepair
                    ? `检测到 ${changedItems.length} 首编码错误`
                    : `将更改 ${changedItems.length} 项`}
                </span>
                {!isRepair && skippedItems.length > 0 && (
                  <span className="batch-skipped-count">
                    （跳过 {skippedItems.length} 项无需处理）
                  </span>
                )}
              </div>
              <div className="batch-preview-list">
                {isRepair ? (
                  // 编码修复：original == new_title（软件端已正确），只显示标题 + 存储位置
                  changedItems.slice(0, 50).map((p, i) => (
                    <div key={i} className="batch-preview-row">
                      <div className="batch-preview-original">
                        [{p.mem_unit === 0 ? "内置" : "SD"}] {p.original || "(无标题)"}
                      </div>
                    </div>
                  ))
                ) : (
                  changedItems.slice(0, 50).map((p, i) => (
                    <div key={i} className="batch-preview-row">
                      <div className="batch-preview-original">{p.original || "(无标题)"}</div>
                      <div className="batch-preview-arrow">改名后</div>
                      <div className="batch-preview-new">{p.new_title || "(无标题)"}</div>
                    </div>
                  ))
                )}
                {changedItems.length > 50 && (
                  <div className="batch-preview-more">
                    …还有 {changedItems.length - 50} 项
                  </div>
                )}
              </div>
              <div className="batch-preview-note">
                {isRepair
                  ? "这些歌曲 bit 0=1 导致设备屏幕中文乱码。修复需下载、清 bit 0、重传整个文件，大文件较慢，请耐心等待进度反馈。"
                  : "改名需下载、修改、重传整个文件（设备协议限制，无法只改头部），大文件较慢，请耐心等待进度反馈。"}
              </div>
            </div>
            <div className="modal-footer">
              <button className="modal-btn" onClick={onClose}>取消</button>
              <button
                className="modal-btn primary"
                onClick={handleConfirm}
                disabled={changedItems.length === 0}
              >
                确认执行（{changedItems.length} 项）
              </button>
            </div>
          </>
        )}

        {/* 执行阶段：显示进度 */}
        {isRunning && (
          <div className="modal-body">
            <div className="batch-progress-info">
              <div className="batch-progress-text">
                {state.progress && state.progress.total > 0
                  ? `${state.progress.current} / ${state.progress.total}`
                  : state.title.includes("扫描") ? "正在扫描设备存储…" : "准备中…"}
              </div>
              <div className="batch-progress-current">
                {state.progress?.currentTitle || ""}
              </div>
            </div>
            {/* 扫描期间 total=0 不显示进度条，避免 0% 长时间停留被误认为卡住 */}
            {state.progress && state.progress.total > 0 && (
              <div className="batch-progress-bar-wrap">
                <div
                  className="batch-progress-bar-fill"
                  style={{ width: `${progressPercent}%` }}
                />
                {/* 百分比文字：定位在绿色填充区域的右端（进度条内部） */}
                <div
                  className={
                    progressPercent >= 12
                      ? "batch-progress-bar-percent inside"
                      : "batch-progress-bar-percent outside"
                  }
                  style={{ left: `${progressPercent}%` }}
                >
                  {progressPercent.toFixed(0)}%
                </div>
              </div>
            )}
            {/* 三步骤提示只对改名/去词显示；编码修复无"改名"步骤，不显示 */}
            {!isRepair && state.progress && state.progress.total > 0 && (
              <div className="batch-progress-phases">
                <span className="batch-phase">下载</span>
                <span className="batch-phase-arrow">·</span>
                <span className="batch-phase">改名</span>
                <span className="batch-phase-arrow">·</span>
                <span className="batch-phase">上传</span>
              </div>
            )}
          </div>
        )}

        {/* 完成阶段：显示结果 */}
        {isDone && (
          <div className="modal-body">
            {state.results && state.results.failed === 0 ? (
              <div className="batch-result-success">
                操作完成
                <div className="batch-result-detail">
                  成功 {state.results.success} 项
                </div>
              </div>
            ) : state.error ? (
              <div className="batch-result-error">
                {state.results && state.results.success > 0
                  ? `部分失败：成功 ${state.results.success} 项，失败 ${state.results?.failed || 0} 项`
                  : `操作失败：失败 ${state.results?.failed || 0} 项`}
                <pre className="batch-result-error-detail">{state.error}</pre>
              </div>
            ) : (
              <div className="batch-result-success">
                操作完成
                {state.results && (
                  <div className="batch-result-detail">
                    成功 {state.results.success} 项
                    {state.results.failed > 0 && `，失败 ${state.results.failed} 项`}
                  </div>
                )}
              </div>
            )}
            <div className="modal-footer">
              <button className="modal-btn primary" onClick={onClose}>关闭</button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// 底部播放器
// ============================================================================

function PlayerBar({
  song,
  onClose,
}: {
  song: SongInfo;
  onClose: () => void;
}) {
  const [isPlaying, setIsPlaying] = useState(false);
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    const timer = setInterval(async () => {
      try {
        const state = await invoke<PlaybackState>("get_playback_state");
        setIsPlaying(state.is_playing);
        setPosition(state.position);
        setDuration(state.duration);
        setIsLoading(state.is_loading);
      } catch {}
    }, 500);
    return () => clearInterval(timer);
  }, []);

  const title = song.title || song.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "(无标题)";
  const progress = duration > 0 ? (position / duration) * 100 : 0;

  async function togglePlay() {
    try {
      if (isPlaying) {
        await invoke("pause_audio");
      } else {
        await invoke("resume_audio");
      }
    } catch {}
  }

  return (
    <div className="player-bar">
      <div className="player-info">
        <div className="player-title">{title}</div>
        <div className="player-subtitle">
          {isLoading ? "加载中..." : `${song.artist || "—"} · ${song.mem_unit === 0 ? "内置" : "SD 卡"}`}
        </div>
      </div>
      <div className="player-controls">
        <button
          className={`player-btn play ${isPlaying ? "" : ""}`}
          onClick={togglePlay}
          title={isPlaying ? "暂停" : "继续"}
          aria-label={isPlaying ? "暂停" : "播放"}
          disabled={isLoading}
        >
          {isPlaying ? (
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <rect x="2" y="1.5" width="2.5" height="9" fill="currentColor" />
              <rect x="7.5" y="1.5" width="2.5" height="9" fill="currentColor" />
            </svg>
          ) : (
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <path d="M2.5 1.5 L10.5 6 L2.5 10.5 Z" fill="currentColor" />
            </svg>
          )}
        </button>
        <button
          className="player-btn"
          onClick={() => invoke("stop_audio").catch(() => {})}
          title="停止"
          aria-label="停止"
          disabled={isLoading}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
            <rect x="2" y="2" width="8" height="8" fill="currentColor" />
          </svg>
        </button>
      </div>
      <div className="player-progress-wrap">
        <span className="player-time">{formatTime(position)}</span>
        <div className="player-progress">
          <div className="player-progress-fill" style={{ width: `${progress}%` }} />
        </div>
        <span className="player-time">{formatTime(duration)}</span>
      </div>
      <button className="player-close" onClick={onClose} title="关闭">×</button>
    </div>
  );
}

// ============================================================================
// 歌曲详细信息模态框
// ============================================================================

function SongDetailModal({
  song,
  onClose,
}: {
  song: SongInfo;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<SongDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [coverUrl, setCoverUrl] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setCoverUrl(null);
    invoke<SongDetail>("get_song_detail", { fileNo: song.file_no, memUnit: song.mem_unit })
      .then((d) => {
        setDetail(d);
        if (d.cover_art && d.cover_art.length > 0) {
          const bytes = new Uint8Array(d.cover_art);
          const blob = new Blob([bytes], { type: "image/jpeg" });
          setCoverUrl(URL.createObjectURL(blob));
        }
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [song]);

  useEffect(() => {
    return () => {
      if (coverUrl) URL.revokeObjectURL(coverUrl);
    };
  }, [coverUrl]);

  const title = song.title || song.name?.replace(/^D:\\/, "").replace(/\.mp3$/i, "") || "(无标题)";

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content detail-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>详细信息</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="detail-body">
          {loading && <div className="loading-text">加载中…</div>}
          {!loading && detail && (
            <>
              <div className="detail-cover">
                {coverUrl ? (
                  <img src={coverUrl} alt="封面" />
                ) : (
                  <div className="detail-cover-placeholder">无封面</div>
                )}
              </div>

              <div className="detail-section">
                <h4>ID3 标签</h4>
                <div className="detail-grid">
                  <span className="detail-label">标题</span>
                  <span className="detail-value">{detail.id3.title || title}</span>
                  <span className="detail-label">艺术家</span>
                  <span className="detail-value">{detail.id3.artist || "—"}</span>
                  <span className="detail-label">专辑</span>
                  <span className="detail-value">{detail.id3.album || "—"}</span>
                  <span className="detail-label">年份</span>
                  <span className="detail-value">{detail.id3.year || "—"}</span>
                  <span className="detail-label">流派</span>
                  <span className="detail-value">{detail.id3.genre || "—"}</span>
                  <span className="detail-label">音轨</span>
                  <span className="detail-value">{detail.id3.track || "—"}</span>
                  <span className="detail-label">作曲</span>
                  <span className="detail-value">{detail.id3.composer || "—"}</span>
                </div>
              </div>

              {detail.technical && (
                <div className="detail-section">
                  <h4>技术参数</h4>
                  <div className="detail-grid">
                    <span className="detail-label">时长</span>
                    <span className="detail-value">{formatTime(detail.technical.duration)}</span>
                    <span className="detail-label">采样率</span>
                    <span className="detail-value">{detail.technical.sample_rate} Hz</span>
                    <span className="detail-label">比特率</span>
                    <span className="detail-value">{detail.technical.bit_rate} kbps</span>
                    <span className="detail-label">声道</span>
                    <span className="detail-value">{detail.technical.channels === 1 ? "单声道" : "立体声"}</span>
                    <span className="detail-label">MPEG 层</span>
                    <span className="detail-value">Layer {detail.technical.layer}</span>
                  </div>
                </div>
              )}

              <div className="detail-section">
                <h4>文件信息</h4>
                <div className="detail-grid">
                  <span className="detail-label">文件号</span>
                  <span className="detail-value">{detail.basic.file_no}</span>
                  <span className="detail-label">大小</span>
                  <span className="detail-value">{formatSize(detail.basic.size)}</span>
                  <span className="detail-label">存储</span>
                  <span className="detail-value">{detail.basic.mem_unit === 0 ? "内置" : "SD 卡"}</span>
                  <span className="detail-label">修改时间</span>
                  <span className="detail-value">
                    {detail.mod_date > 0
                      ? new Date(detail.mod_date * 1000).toLocaleString("zh-CN")
                      : "—"}
                  </span>
                </div>
              </div>
            </>
          )}
        </div>
        <div className="modal-footer">
          <button className="modal-btn" onClick={onClose}>关闭</button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 同步页面
// ============================================================================

function SyncPane({
  onError,
  onRefreshStorage,
  playlistsCache,
  onRefreshPlaylists,
  onConfirm,
}: {
  onError: (msg: string | null) => void;
  onRefreshStorage: () => void;
  playlistsCache: PlaylistInfo[];
  onRefreshPlaylists: () => void;
  onConfirm: (s: ConfirmState) => void;
}) {
  const [rules, setRules] = useState<SyncRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);

  const loadRules = useCallback(async () => {
    setLoading(true);
    try {
      setRules(await invoke<SyncRule[]>("list_sync_rules"));
    } catch (e) {
      onError(`加载同步规则失败: ${e}`);
    }
    setLoading(false);
  }, [onError]);

  useEffect(() => {
    loadRules();
  }, [loadRules]);

  async function runSync(rule: SyncRule) {
    setSyncing(rule.id);
    try {
      const result = await invoke<SyncResult>("run_sync", { ruleId: rule.id });
      onError(`同步完成：新增 ${result.added.length}，删除 ${result.deleted.length}，跳过 ${result.skipped.length}${result.errors.length > 0 ? `，失败 ${result.errors.length}` : ""}`);
      await loadRules();
      await onRefreshStorage();
    } catch (e) {
      onError(`同步失败: ${e}`);
    }
    setSyncing(null);
  }

  async function deleteRule(id: string) {
    onConfirm({
      title: "删除同步规则",
      message: "确认删除此同步规则？此操作不可撤销。",
      danger: true,
      onConfirm: async () => {
        try {
          await invoke("delete_sync_rule", { id });
          await loadRules();
        } catch (e) {
          onError(`删除规则失败: ${e}`);
        }
      },
    });
  }

  return (
    <div className="pane sync-pane">
      <div className="pane-header">
        <h2>歌曲同步</h2>
        <span className="count-badge">{rules.length} 条规则</span>
        <button className="batch-btn refresh" onClick={loadRules} disabled={loading}>刷新</button>
        <button className="batch-btn" onClick={() => setShowAdd(true)} style={{ marginLeft: "auto" }}>
          + 添加规则
        </button>
      </div>
      {loading && <div className="loading-text">加载中…</div>}
      {!loading && rules.length === 0 && (
        <div className="sync-empty">
          暂无同步规则，点击"添加规则"创建
          <br />
          <span style={{ fontSize: 11, opacity: 0.7 }}>
            镜像同步：本地文件夹为主，设备完全镜像本地内容
          </span>
        </div>
      )}
      {!loading && rules.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 6, overflow: "auto", flex: 1 }}>
          {rules.map((rule) => (
            <div key={rule.id} className="sync-rule-card">
              <div className="sync-rule-info">
                <div className="sync-rule-path">{rule.local_path}</div>
                <div className="sync-rule-meta">
                  <span className={`mem-badge mem-${rule.mem_unit}`}>
                    {rule.mem_unit === 0 ? "内置" : "SD 卡"}
                  </span>
                  {rule.playlist_file_no !== null && (
                    <span>歌单 #{rule.playlist_file_no}</span>
                  )}
                  {rule.last_sync_at !== null && (
                    <span>· 上次同步 {new Date(rule.last_sync_at * 1000).toLocaleString("zh-CN")}</span>
                  )}
                </div>
              </div>
              <div className="sync-rule-actions">
                <button
                  className="sync-run-btn"
                  onClick={() => runSync(rule)}
                  disabled={syncing !== null}
                >
                  {syncing === rule.id ? "同步中…" : "同步"}
                </button>
                <button
                  className="sync-delete-btn"
                  onClick={() => deleteRule(rule.id)}
                  disabled={syncing !== null}
                >
                  删除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      {showAdd && (
        <AddSyncRuleModal
          playlistsCache={playlistsCache}
          onClose={() => setShowAdd(false)}
          onAdded={async () => {
            setShowAdd(false);
            await loadRules();
            onRefreshPlaylists();
          }}
          onError={onError}
        />
      )}
    </div>
  );
}

// ============================================================================
// 添加同步规则模态框
// ============================================================================

function AddSyncRuleModal({
  playlistsCache,
  onClose,
  onAdded,
  onError,
}: {
  playlistsCache: PlaylistInfo[];
  onClose: () => void;
  onAdded: () => void;
  onError: (msg: string | null) => void;
}) {
  const [localPath, setLocalPath] = useState("");
  const [memUnit, setMemUnit] = useState(0);
  const [playlistFileNo, setPlaylistFileNo] = useState<number | null>(null);
  const [creating, setCreating] = useState(false);

  async function pickFolder() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        setLocalPath(selected);
      }
    } catch (e) {
      onError(`选择文件夹失败: ${e}`);
    }
  }

  async function create() {
    if (!localPath.trim()) return;
    setCreating(true);
    try {
      await invoke("add_sync_rule", {
        localPath: localPath.trim(),
        memUnit,
        playlistFileNo,
      });
      onError("同步规则已添加");
      onAdded();
    } catch (e) {
      onError(`添加失败: ${e}`);
    }
    setCreating(false);
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>添加同步规则</h3>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>
        <div className="modal-body">
          <p className="modal-desc">镜像同步：本地文件夹为主，设备完全镜像本地内容。</p>
          <div style={{ marginBottom: 10 }}>
            <label style={{ fontSize: 11, color: "var(--text-dim)", display: "block", marginBottom: 4 }}>
              本地文件夹
            </label>
            <div style={{ display: "flex", gap: 6 }}>
              <input
                className="filter-search"
                style={{ flex: 1, maxWidth: "none", padding: "6px 10px" }}
                type="text"
                placeholder="选择或输入文件夹路径"
                value={localPath}
                onChange={(e) => setLocalPath(e.target.value)}
              />
              <button className="batch-btn" onClick={pickFolder}>浏览</button>
            </div>
          </div>
          <div style={{ marginBottom: 10 }}>
            <label style={{ fontSize: 11, color: "var(--text-dim)", display: "block", marginBottom: 4 }}>
              目标存储
            </label>
            <div className="mem-switch" style={{ marginLeft: 0 }}>
              <button className={memUnit === 0 ? "active" : ""} onClick={() => setMemUnit(0)}>内置</button>
              <button className={memUnit === 1 ? "active" : ""} onClick={() => setMemUnit(1)}>SD 卡</button>
            </div>
          </div>
          <div>
            <label style={{ fontSize: 11, color: "var(--text-dim)", display: "block", marginBottom: 4 }}>
              目标歌单（可选，留空则不同步到歌单）
            </label>
            <select
              className="filter-search"
              style={{ width: "100%", maxWidth: "none", padding: "6px 10px" }}
              value={playlistFileNo === null ? "" : String(playlistFileNo)}
              onChange={(e) => setPlaylistFileNo(e.target.value === "" ? null : Number(e.target.value))}
            >
              <option value="">不同步到歌单</option>
              {playlistsCache.map((p) => {
                const title = p.title || p.name?.replace(/^D:\\/, "").replace(/\.pls$/i, "") || "(未命名)";
                return (
                  <option key={`${p.mem_unit}:${p.file_no}`} value={String(p.file_no)}>
                    {title} ({p.mem_unit === 0 ? "内置" : "SD 卡"})
                  </option>
                );
              })}
            </select>
          </div>
        </div>
        <div className="modal-footer">
          <button className="modal-btn" onClick={onClose}>取消</button>
          <button className="modal-btn" onClick={create} disabled={creating || !localPath.trim()}>
            {creating ? "添加中…" : "添加"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 二次确认弹窗
// ============================================================================

function ConfirmModal({
  state,
  onClose,
}: {
  state: NonNullable<ConfirmState>;
  onClose: () => void;
}) {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "Enter") {
        e.preventDefault();
        state.onConfirm();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [state, onClose]);

  return (
    <div
      className="confirm-overlay"
    >
      <div
        className="confirm-card"
      >
        <div className="confirm-title">{state.title}</div>
        <div className="confirm-message">{state.message}</div>
        <div className="confirm-buttons">
          <button
            className="confirm-btn cancel"
            onClick={onClose}
          >
            {state.cancelText || "取消"}
          </button>
          <button
            className={`confirm-btn ${state.danger ? "danger" : "primary"}`}
            onClick={() => {
              state.onConfirm();
              onClose();
            }}
          >
            {state.confirmText || "确认"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// 上传传输侧栏（Windows 98-XP 风格，左侧固定栏，不遮罩）
// ============================================================================

/** 传输页面 — 全宽选项卡，展示上传/下载传输进度
 * - 顶部：总进度（计数/百分比/字节数）
 * - 中部：当前文件进度
 * - 底部：文件列表（每曲目进度条、名称、状态、错误） */
function TransmissionPane({
  files,
  uploading,
  notice,
}: {
  files: UploadFileItem[];
  uploading: boolean;
  notice: string | null;
}) {
  const doneCount = files.filter((f) => f.status === "done").length;
  const failedCount = files.filter((f) => f.status === "failed").length;
  const totalCount = files.length;
  const currentFile = files.find((f) => f.status === "uploading");

  const totalBytes = files.reduce((sum, f) => sum + (f.total || 0), 0);
  const transferredBytes = files.reduce((sum, f) => sum + f.transferred, 0);
  // 总进度按"已传输字节数 / 总字节数"计算，更精确
  const totalPercent =
    totalBytes > 0 ? Math.min(100, (transferredBytes / totalBytes) * 100) : 0;

  const currentPercent =
    currentFile && currentFile.total > 0
      ? (currentFile.transferred / currentFile.total) * 100
      : 0;

  const allDone = totalCount > 0 && doneCount + failedCount === totalCount;
  const isTransmitting = uploading && !allDone;

  // 空状态：尚未开始任何传输
  if (files.length === 0) {
    return (
      <div className="pane transmission-pane">
        <div className="pane-header">
          <h2>传输</h2>
        </div>
        <div className="transmission-empty">
          <div className="transmission-empty-title">暂无传输任务</div>
          <div className="transmission-empty-hint">
            切换到「上传」标签页，拖入 MP3 文件或选择文件后，传输进度会自动显示在此页面。
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="pane transmission-pane">
      <div className="pane-header">
        <h2>{isTransmitting ? "正在传输" : allDone ? "传输完成" : "传输"}</h2>
        <span className="count-badge">
          {doneCount} / {totalCount}
          {failedCount > 0 && <span className="upload-failed"> · 失败 {failedCount}</span>}
        </span>
      </div>

      {/* 总进度区 */}
      <div className="transmission-summary">
        <div className="transmission-summary-row">
          <span className="transmission-summary-label">总进度</span>
          <span className="transmission-summary-value">
            {doneCount} / {totalCount}
            {totalBytes > 0 && (
              <span className="transmission-summary-bytes">
                {" "}· {formatBytes(transferredBytes)} / {formatBytes(totalBytes)}
              </span>
            )}
          </span>
          <span className="transmission-summary-percent">{totalPercent.toFixed(1)}%</span>
        </div>
        <div className="transmission-total-bar">
          <div className="transmission-total-fill" style={{ width: `${totalPercent}%` }} />
        </div>
        {notice && <div className="transmission-notice">{notice}</div>}
      </div>

      {/* 当前文件进度区 */}
      {currentFile && (
        <div className="transmission-current">
          <div className="transmission-current-row">
            <span className="transmission-current-name" title={currentFile.name}>
              {currentFile.name}
            </span>
            <span className="transmission-current-percent">{Math.round(currentPercent)}%</span>
          </div>
          <div className="transmission-current-bar">
            <div
              className="transmission-current-fill"
              style={{ width: `${currentPercent}%` }}
            />
          </div>
          {currentFile.total > 0 && (
            <div className="transmission-current-bytes">
              {formatBytes(currentFile.transferred)} / {formatBytes(currentFile.total)}
            </div>
          )}
        </div>
      )}

      {/* 文件列表 — 每个曲目含进度条、名称、状态 */}
      <div className="transmission-list">
        <div className="transmission-list-header">
          <span>文件名</span>
          <span>状态</span>
        </div>
        {files.map((f, i) => {
          const pct = f.total > 0 ? Math.min(100, (f.transferred / f.total) * 100) : 0;
          const statusLabel =
            f.status === "uploading" ? "传输中"
            : f.status === "done" ? "已完成"
            : f.status === "failed" ? "失败"
            : "等待中";
          return (
            <div key={i} className={`transmission-row ${f.status}`}>
              <div className="transmission-row-info">
                <span className="transmission-row-name" title={f.name}>{f.name}</span>
                <span className={`transmission-row-status ${f.status}`}>{statusLabel}</span>
              </div>
              <div className="transmission-row-bar">
                <div className="transmission-row-fill" style={{ width: `${pct}%` }} />
              </div>
              <div className="transmission-row-meta">
                {f.status === "uploading" && f.total > 0 && (
                  <span>{formatBytes(f.transferred)} / {formatBytes(f.total)}</span>
                )}
                {f.status === "done" && f.total > 0 && (
                  <span>{formatBytes(f.total)}</span>
                )}
                {f.status === "failed" && f.error && (
                  <span className="transmission-row-error" title={f.error}>{f.error}</span>
                )}
                {f.status === "pending" && <span>等待中</span>}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
