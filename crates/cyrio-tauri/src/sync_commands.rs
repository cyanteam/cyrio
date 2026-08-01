//! 歌曲同步命令：镜像同步本地文件夹到设备/歌单
//!
//! 同步策略：本地文件夹为主，设备完全镜像本地内容。
//! 文件匹配仅用 basename（不含目录），不比较内容 hash（性能优先）。
//! 配置文件：`app_config_dir()/cyrio/sync_rules.json`。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::DeviceState;

/// 同步规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRule {
    /// 规则 ID（UUID v4）
    pub id: String,
    /// 本地文件夹路径
    pub local_path: String,
    /// 目标存储单元（0=内置, 1=SD）
    pub mem_unit: u8,
    /// 可选：同步到指定歌单的 file_no（None 表示仅同步到设备存储）
    pub playlist_file_no: Option<u32>,
    /// 上次同步时间（Unix 秒）
    pub last_sync_at: Option<u64>,
}

/// 同步结果
#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    /// 新增的歌曲名列表
    pub added: Vec<String>,
    /// 删除的歌曲名列表
    pub deleted: Vec<String>,
    /// 跳过的歌曲名列表（已存在）
    pub skipped: Vec<String>,
    /// 错误信息列表
    pub errors: Vec<String>,
}

const CONFIG_FILE: &str = "sync_rules.json";
const CONFIG_DIR: &str = "cyrio";

/// 获取配置文件路径
fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("获取配置目录失败: {}", e))?;
    Ok(dir.join(CONFIG_DIR).join(CONFIG_FILE))
}

/// 加载所有同步规则（配置文件不存在或解析失败时返回空 Vec）
fn load_rules(app: &AppHandle) -> Vec<SyncRule> {
    match config_path(app) {
        Ok(p) => {
            if let Ok(data) = std::fs::read(&p) {
                if let Ok(rules) = serde_json::from_slice::<Vec<SyncRule>>(&data) {
                    return rules;
                }
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

/// 保存同步规则到配置文件
fn save_rules(app: &AppHandle, rules: &[SyncRule]) -> Result<(), String> {
    let p = config_path(app)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    let data = serde_json::to_vec_pretty(rules).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&p, data).map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

/// 列出所有同步规则
#[tauri::command]
pub async fn list_sync_rules(app: AppHandle) -> Result<Vec<SyncRule>, String> {
    Ok(load_rules(&app))
}

/// 添加同步规则
#[tauri::command]
pub async fn add_sync_rule(
    app: AppHandle,
    local_path: String,
    mem_unit: u8,
    playlist_file_no: Option<u32>,
) -> Result<(), String> {
    let mut rules = load_rules(&app);
    let id = uuid::Uuid::new_v4().to_string();
    rules.push(SyncRule {
        id,
        local_path,
        mem_unit,
        playlist_file_no,
        last_sync_at: None,
    });
    save_rules(&app, &rules)
}

/// 删除同步规则
#[tauri::command]
pub async fn delete_sync_rule(app: AppHandle, id: String) -> Result<(), String> {
    let mut rules = load_rules(&app);
    rules.retain(|r| r.id != id);
    save_rules(&app, &rules)
}

/// 执行同步（镜像同步：本地为主，设备完全镜像本地）
///
/// 1. walkdir 收集本地所有 .mp3 文件（basename → path）
/// 2. list_files 列出设备现有歌曲（basename → file_no）
/// 3. 计算差异：to_add（本地有设备无）、to_delete（设备有本地无）
/// 4. 先删后增（腾出空间），上传时若指定歌单则加入歌单
/// 5. 更新 last_sync_at
#[tauri::command]
pub async fn run_sync(
    state: tauri::State<'_, DeviceState>,
    app: AppHandle,
    rule_id: String,
) -> Result<SyncResult, String> {
    let rules = load_rules(&app);
    let rule = rules
        .iter()
        .find(|r| r.id == rule_id)
        .ok_or("同步规则不存在")?
        .clone();

    let mut result = SyncResult {
        added: Vec::new(),
        deleted: Vec::new(),
        skipped: Vec::new(),
        errors: Vec::new(),
    };

    // 1. 收集本地 mp3 文件（basename → path），用 smol::unblock 避免阻塞
    let local_files: std::collections::HashMap<String, PathBuf> = {
        let path = rule.local_path.clone();
        smol::unblock(move || {
            let mut map = std::collections::HashMap::new();
            for entry in walkdir::WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file()
                    && entry.path().extension().and_then(|e| e.to_str()) == Some("mp3")
                {
                    if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                        map.insert(name.to_string(), entry.path().to_path_buf());
                    }
                }
            }
            map
        })
        .await
    };

    // 2. 列出设备现有歌曲（basename → file_no）
    let device_songs: std::collections::HashMap<String, u32> = {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        let files = device
            .list_files(rule.mem_unit, |_| {})
            .await
            .map_err(|e| e.to_string())?;
        let mut map = std::collections::HashMap::new();
        for f in files
            .iter()
            .filter(|f| f.file_type == cyrio_core::protocol::constants::TYPE_MP3)
        {
            // f.name 格式如 "D:\xxx.mp3"，取 basename
            let basename = f
                .name
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&f.name)
                .to_string();
            map.insert(basename, f.file_no);
        }
        map
    };

    // 3. 计算差异
    let to_add: Vec<(String, PathBuf)> = local_files
        .iter()
        .filter(|(name, _)| !device_songs.contains_key(*name))
        .map(|(n, p)| (n.clone(), p.clone()))
        .collect();
    let to_delete: Vec<(String, u32)> = device_songs
        .iter()
        .filter(|(name, _)| !local_files.contains_key(*name))
        .map(|(n, fno)| (n.clone(), *fno))
        .collect();

    // 4. 执行删除（先删后增，腾出空间）
    for (name, file_no) in &to_delete {
        let guard = state.device.lock().await;
        let device = guard.as_ref().ok_or("设备未连接")?;
        match device.delete_file(rule.mem_unit, *file_no).await {
            Ok(()) => result.deleted.push(name.clone()),
            Err(e) => result
                .errors
                .push(format!("删除 {} 失败: {}", name, e)),
        }
    }

    // 5. 执行上传（并加入歌单）
    // 同步功能默认不应用 slug/strip（保持文件名原样上传）
    let sync_text_opts = cyrio_core::api::upload::UploadTextOptions::default();
    for (name, path) in &to_add {
        let upload_result = {
            let guard = state.device.lock().await;
            match guard.as_ref() {
                Some(device) => cyrio_core::api::upload::upload_mp3(
                    device,
                    rule.mem_unit,
                    path,
                    &sync_text_opts,
                    |_| {},
                )
                .await
                .map_err(|e| e.to_string()),
                None => Err("设备未连接".to_string()),
            }
        };
        // 失效 songs 缓存（上传后）
        state.invalidate_songs_cache(rule.mem_unit).await;
        match upload_result {
            Ok(file_no) => {
                result.added.push(name.clone());
                // 如果指定了歌单，加入歌单
                if let Some(pl_no) = rule.playlist_file_no {
                    let guard = state.device.lock().await;
                    let device = guard.as_ref().ok_or("设备未连接")?;
                    if let Err(e) = cyrio_core::api::playlist::add_to_playlist(
                        device,
                        file_no,
                        rule.mem_unit,
                        pl_no,
                        rule.mem_unit,
                    )
                    .await
                    {
                        result
                            .errors
                            .push(format!("加入歌单 {} 失败: {}", name, e));
                    }
                }
            }
            Err(e) => result
                .errors
                .push(format!("上传 {} 失败: {}", name, e)),
        }
    }

    // 6. 跳过的（已存在）
    for name in local_files.keys() {
        if device_songs.contains_key(name) {
            result.skipped.push(name.clone());
        }
    }

    // 7. 失效 songs 缓存（同步改变了设备歌曲列表）
    state.invalidate_songs_cache(rule.mem_unit).await;

    // 8. 更新 last_sync_at
    let mut rules = load_rules(&app);
    if let Some(r) = rules.iter_mut().find(|r| r.id == rule_id) {
        r.last_sync_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        );
    }
    let _ = save_rules(&app, &rules);

    Ok(result)
}
