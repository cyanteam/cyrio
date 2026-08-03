//! # 高层 API
//!
//! 把 protocol 层的裸 USB 操作封装为一等公民 API：
//! - [`device`]：RioDevice（设备连接 + init 序列 + 读写操作）
//! - [`types`]：用户面向类型（Song/Playlist）+ 通用辅助函数
//! - [`playlist`]：播放列表操作（list_playlist_songs / create_playlist / add_to_playlist）
//! - [`upload`]：上传相关（ID3v2 解析 + MP3 上传 + 批量上传 + 路径展开）
//! - [`rename`]：重命名/批量文本处理（slug + strip 集成）
//!
//! 移植自 nodejs `src/api/`。

pub mod device;
pub mod playlist;
pub mod rename;
pub mod types;
pub mod upload;

pub use types::{format_bytes, is_mp3_file, is_playlist_file, precheck_free_space, rio_file_to_playlist, rio_file_to_song, Playlist, Song};
pub use upload::{expand_paths, get_id3v2_size, read_id3_tags, build_upload_header, upload_mp3, upload_mp3_batch, process_title, Id3Tags, UploadResult, UploadTextOptions};
pub use rename::{batch_strip_noise, batch_to_slug, rename_song_title, repair_song_encoding, RenameResult};
