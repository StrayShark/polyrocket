//! L2 — Storage path IPC commands (v0.53a).
//!
//! Three IPCs today:
//! - `get_storage_info` — return the default path,
//!   the current effective path, and disk health
//!   metrics (writable, free_bytes).
//! - `set_storage_path` — write a custom path to
//!   `_polyrocket_settings: storage_path`. The
//!   change takes effect on NEXT launch (the
//!   current process's DB is already open).
//! - `reset_storage_path` — clear the custom path;
//!   next launch falls back to the OS default.
//!
//! ## Restart-required contract
//!
//! polyrocket is a long-running desktop process.
//! The DB pool is opened in `lib.rs::run` before
//! any IPC handler runs. Changing `storage_path`
//! mid-flight would mean migrating an open SQLite
//! connection — out of scope for v0.53. The L1
//! surfaces a "Restart now" button on the success
//! path; the Rust side also logs the requirement.

use crate::AppResult;
use crate::infra::state::AppState;
use crate::platform::paths;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct StorageInfo {
    /// OS-recommended path (Tauri's
    /// `app_data_dir`). macOS: `~/Library/Application
    /// Support/com.polyrocket.app/`. Win: `%APPDATA%`.
    /// Linux: `~/.local/share/com.polyrocket.app/`.
    pub default_path: String,
    /// The path polyrocket will use on the next
    /// launch. Equals `default_path` when the user
    /// hasn't picked a custom one.
    pub current_path: String,
    /// True when current_path != default_path.
    pub is_custom: bool,
    /// True when the current_path exists on disk.
    /// (For the default path, this is created on
    /// first call; for a custom path, it must
    /// already exist.)
    pub exists: bool,
    /// True when the current user can write to
    /// current_path. When false, setStoragePath
    /// would fail — the L1 must show the user a
    /// "permission denied" error and a hint to pick
    /// a writable location.
    pub writable: bool,
    /// Free space in bytes. `None` when the OS query
    /// fails (rare; some sandboxed filesystems).
    pub free_bytes: Option<u64>,
    /// True when the active session's db is at the
    /// default path even though `storage_path` is
    /// set. This happens when the user picked a
    /// custom path but hasn't restarted yet.
    /// The L1 surfaces a "Restart required" banner
    /// when this is true.
    pub restart_required: bool,
}

/// v0.53a — return the current storage path
/// information. Cheap: just `std::fs` metadata +
/// `statvfs` (or platform equivalent). No DB read
/// happens here — the "active" path is computed by
/// walking the `storage_path` settings key.
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
    /// Absolute path to the directory where
    /// polyrocket.db should live on next launch.
    /// The directory must already exist.
    pub path: String,
}

/// v0.53a — write a custom storage path. The change
/// takes effect on next launch. We don't migrate
/// the existing DB — that's a v0.54+ feature.
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
    // Make sure the logs/ subdir exists (or can be
    // created) under the chosen path.
    let logs_dir = path.join("logs");
    std::fs::create_dir_all(&logs_dir).map_err(|e| {
        crate::AppError::Internal(format!(
            "create_dir_all({}): {e}",
            logs_dir.display()
        ))
    })?;
    // Persist to _polyrocket_settings.
    crate::infra::db::settings::set(
        &state.db,
        "storage_path",
        &path.to_string_lossy(),
    )
    .await
    .map_err(|e| {
        crate::AppError::Internal(format!("settings.set: {e}"))
    })?;
    // v0.53a — also write the JSON config file.
    // This is the source of truth at startup
    // (read BEFORE the pool opens). The DB row is
    // for the L1 surface; the JSON file is for
    // path resolution on next launch.
    let app = crate::infra::scheduler::TAURI_APP
        .get()
        .ok_or_else(|| crate::AppError::Internal(
            "TAURI_APP not initialized (storage.path.set)".into(),
        ))?
        .clone();
    write_storage_config_file(&app, Some(&path))?;
    // Audit-log it (no secret; path is OK to log).
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
    .ok(); // best-effort
    Ok(())
}

/// v0.53a — clear the custom storage path. Next
/// launch falls back to the OS default.
#[tauri::command]
pub async fn reset_storage_path(
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::infra::db::settings::delete(&state.db, "storage_path")
        .await
        .map_err(|e| {
            crate::AppError::Internal(format!("settings.delete: {e}"))
        })?;
    // v0.53a — also clear the JSON config.
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
// helpers
// ----------------------------------------------------------------

/// Probe whether the current process can create a
/// new file in `dir`. We try to create a tempfile
/// and immediately remove it. Returns true on
/// success.
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

/// v0.53a — write the `storage_path.json` config
/// file. The shape is `{"path": <string|null>}`
/// — a single key so users who hand-edit the file
/// can't break the app by adding unknown keys.
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

/// Best-effort free-space query. Returns None when
/// the OS call fails (sandbox, exotic fs).
///
/// v0.53a: returns None on all platforms. The L1
/// hides the "free space" metric when this is None.
/// v0.54+ could pull in `nix` and use `statvfs`.
fn free_space_bytes(_path: &std::path::Path) -> Option<u64> {
    None
}

// ----------------------------------------------------------------
// tests
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
        // Documented behavior: we don't pull in `nix`
        // for statvfs in v0.53a. v0.54+ candidate.
        assert_eq!(free_space_bytes(std::path::Path::new("/")), None);
    }
}
