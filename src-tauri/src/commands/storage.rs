//! L2 —— 存储路径 IPC 命令（v0.53a）。
//!
//! 当前提供 3 个 IPC：
//! - `get_storage_info` —— 返回默认路径、当前生效路径，
//!   以及磁盘健康指标（是否可写、可用字节数）。
//! - `set_storage_path` —— 把自定义路径写入
//!   `_polyrocket_settings: storage_path`。修改
//!   在下次启动后生效（当前进程的 DB 已经打开）。
//! - `reset_storage_path` —— 清除自定义路径，
//!   下次启动回退到系统默认。
//!
//! ## 需重启的契约
//!
//! polyrocket 是一个长期运行的桌面进程。
//! DB 连接池在 `lib.rs::run` 中、任何 IPC handler
//! 运行之前就已打开。在运行时修改 `storage_path`
//! 等价于迁移一个已打开的 SQLite 连接 —— 这超出
//! v0.53 的范围。L1 在成功路径上展示「立即重启」按钮；
//! Rust 端也会记录这一要求。

use crate::AppResult;
use crate::infra::state::AppState;
use crate::platform::paths;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct StorageInfo {
    /// 系统推荐的路径（Tauri 的 `app_data_dir`）。
    /// macOS：`~/Library/Application Support/com.polyrocket.app/`。
    /// Win：`%APPDATA%`。
    /// Linux：`~/.local/share/com.polyrocket.app/`。
    pub default_path: String,
    /// polyrocket 下次启动将使用的路径。
    /// 当用户没有指定自定义路径时，等于 `default_path`。
    pub current_path: String,
    /// 当 current_path != default_path 时为 true。
    pub is_custom: bool,
    /// 当 current_path 在磁盘上存在时为 true。
    /// （默认路径会在首次访问时创建；自定义路径
    /// 则必须已经存在。）
    pub exists: bool,
    /// 当前用户能写入 current_path 时为 true。
    /// 若为 false，setStoragePath 会失败 —— L1 必须
    /// 向用户显示「权限不足」错误，并提示换一个
    /// 可写的位置。
    pub writable: bool,
    /// 可用空间（字节）。操作系统查询失败时为 None
    /// （极少发生；通常是某些沙箱文件系统）。
    pub free_bytes: Option<u64>,
    /// 即便已经设置了 `storage_path`，本会话的活动 db
    /// 仍位于默认路径下时为 true。这发生在用户选择了
    /// 自定义路径但尚未重启时。
    /// L1 在此为 true 时显示「需要重启」横幅。
    pub restart_required: bool,
}

/// v0.53a —— 返回当前存储路径信息。
/// 开销很低：仅 `std::fs` 元数据 + `statvfs`
/// （或平台等价调用）。此处不读 DB —— 「活动」路径
/// 是通过遍历 `storage_path` 设置项推导出的。
#[tauri::command]
pub async fn get_storage_info(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<StorageInfo> {
    let default_path = paths::app_data_dir(&app)?
        .join("polyrocket.db")
        .to_string_lossy()
        .to_string();
    let custom_path =
        crate::infra::db::settings::get(&state.db, "storage_path").await?;
    let (current_path, is_custom) = match custom_path {
        Some(p) => (p.clone(), true),
        None => (default_path.clone(), false),
    };
    let path = std::path::Path::new(&current_path);
    let exists = path.exists();
    let writable = if exists {
        probe_writable(path)
    } else if let Some(parent) = path.parent() {
        probe_writable(parent)
    } else {
        false
    };
    let free_bytes = free_space_bytes(path);
    let restart_required = is_custom
        && std::path::Path::new(&default_path).exists();
    Ok(StorageInfo {
        default_path,
        current_path,
        is_custom,
        exists,
        writable,
        free_bytes,
        restart_required,
    })
}

#[derive(Debug, Deserialize)]
pub struct SetStoragePathArgs {
    /// 下次启动时 `polyrocket.db` 应所在目录的绝对路径。
    /// 目录必须已存在。
    pub path: String,
}

/// v0.53a —— 写入自定义存储路径。该改动在下一次启动时生效。
/// 我们不会迁移已有 DB —— 那是 v0.54+ 的特性。
#[tauri::command]
pub async fn set_storage_path(
    state: State<'_, AppState>,
    args: SetStoragePathArgs,
) -> AppResult<()> {
    let path = std::path::PathBuf::from(args.path.trim());
    if !path.is_absolute() {
        return Err(crate::AppError::Invalid(
            "storage path must be absolute".into(),
        ));
    }
    if !path.exists() {
        return Err(crate::AppError::Invalid(format!(
            "storage path does not exist: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(crate::AppError::Invalid(format!(
            "storage path is not a directory: {}",
            path.display()
        )));
    }
    if !probe_writable(&path) {
        return Err(crate::AppError::Invalid(format!(
            "storage path is not writable: {}",
            path.display()
        )));
    }
    // 确保所选路径下的 logs/ 子目录存在（或可创建）。
    let logs_dir = path.join("logs");
    std::fs::create_dir_all(&logs_dir).map_err(|e| {
        crate::AppError::Internal(format!(
            "create_dir_all({}): {e}",
            logs_dir.display()
        ))
    })?;
    // 持久化到 _polyrocket_settings。
    crate::infra::db::settings::set(
        &state.db,
        "storage_path",
        &path.to_string_lossy(),
    )
    .await
    .map_err(|e| {
        crate::AppError::Internal(format!("settings.set: {e}"))
    })?;
    // v0.53a —— 同样写入 JSON 配置文件。
    // 这是启动时的真值来源（在连接池打开之前读取）。
    // DB 中的行供 L1 表面展示；JSON 文件供下次启动
    // 做路径解析。
    let app = crate::infra::scheduler::TAURI_APP
        .get()
        .ok_or_else(|| crate::AppError::Internal(
            "TAURI_APP 未初始化（storage.path.set）".into(),
        ))?
        .clone();
    write_storage_config_file(&app, Some(&path))?;
    // 写一条审计日志（无敏感内容，路径可以记录）。
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result)
         VALUES ('user', 'storage.path.set', ?, ?, 'ok')",
    )
    .bind(path.to_string_lossy().to_string())
    .bind(format!(
        "{{\"path\":\"{}\"}}",
        path.to_string_lossy()
    ))
    .execute(&state.db)
    .await
    .ok(); // 尽力而为
    Ok(())
}

/// v0.53a —— 清除自定义存储路径。下次启动回退到系统默认。
#[tauri::command]
pub async fn reset_storage_path(
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::infra::db::settings::delete(&state.db, "storage_path")
        .await
        .map_err(|e| {
            crate::AppError::Internal(format!("settings.delete: {e}"))
        })?;
    // v0.53a —— 同时清空 JSON 配置。
    let app = crate::infra::scheduler::TAURI_APP
        .get()
        .ok_or_else(|| crate::AppError::Internal(
            "TAURI_APP not initialized (storage.path.reset)".into(),
        ))?
        .clone();
    write_storage_config_file(&app, None)?;
    sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result)
         VALUES ('user', 'storage.path.reset', 'storage_path', 'null', 'ok')",
    )
    .execute(&state.db)
    .await
    .ok();
    Ok(())
}

// ----------------------------------------------------------------
// 辅助函数
// ----------------------------------------------------------------

/// 探测当前进程能否在 `dir` 下创建新文件。
/// 尝试创建一个临时文件并立即删除。成功返回 true。
fn probe_writable(dir: &std::path::Path) -> bool {
    let probe = dir.join(".polyrocket-write-probe");
    let result = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe);
    let ok = result.is_ok();
    if ok {
        let _ = std::fs::remove_file(&probe);
    }
    ok
}

/// v0.53a —— 写入 `storage_path.json` 配置文件。
/// 形态为 `{"path": <string|null>}`，仅有一个键，
/// 这样即便用户手动编辑该文件，添加未知键也不会让
/// 应用崩溃。
fn write_storage_config_file(
    app: &AppHandle,
    path: Option<&std::path::Path>,
) -> AppResult<()> {
    let dir = crate::platform::paths::app_data_dir(app)?;
    let file = dir.join("storage_path.json");
    let body = match path {
        Some(p) => serde_json::json!({"path": p.to_string_lossy()}),
        None => serde_json::json!({"path": null}),
    };
    let s = serde_json::to_string_pretty(&body).map_err(|e| {
        crate::AppError::Internal(format!("serialize storage_path.json: {e}"))
    })?;
    std::fs::write(&file, s).map_err(|e| {
        crate::AppError::Internal(format!("write storage_path.json: {e}"))
    })?;
    Ok(())
}

/// 尽力查询可用空间。当操作系统调用失败（沙箱、
/// 特殊文件系统）时返回 None。
///
/// v0.53a：在所有平台上都返回 None。L1 在值为 None
/// 时隐藏「可用空间」指标。v0.54+ 可以引入 `nix` crate
/// 并调用 `statvfs`。
fn free_space_bytes(_path: &std::path::Path) -> Option<u64> {
    None
}

// ----------------------------------------------------------------
// 测试
// ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_writable_on_tempdir() {
        let dir = std::env::temp_dir();
        assert!(probe_writable(&dir));
    }

    #[test]
    fn probe_writable_on_nonexistent_dir_returns_false() {
        let p = std::path::PathBuf::from("/this/does/not/exist/anywhere");
        assert!(!probe_writable(&p));
    }

    #[test]
    fn free_space_bytes_returns_none_in_v53a() {
        // 记录在案的行为：v0.53a 阶段不引入 `nix`
        // 调用 statvfs。v0.54+ 候选。
        assert_eq!(free_space_bytes(std::path::Path::new("/")), None);
    }
}
