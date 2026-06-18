//! L5 — Application filesystem paths.
//!
//! All file IO under the app's data directory goes through here so
//! the layout is auditable from one place.
//!
//! Layout (per `tauri::path::PathResolver::app_data_dir`)
//! --------------------------------------------------------------
//! macOS:   ~/Library/Application Support/com.polyrocket.app/
//! Linux:   ~/.local/share/com.polyrocket.app/
//! Windows: %APPDATA%\com.polyrocket.app\
//!
//! Files we own:
//!   polyrocket.db          (SQLite database — L4 infra/db/pool.rs owns)
//!   polyrocket.db-wal      (SQLite WAL — SQLite internal)
//!   polyrocket.db-shm      (SQLite shared memory — SQLite internal)
//!   logs/polyrocket.log    (Tauri app log — Tauri plugin owns)
//!   storage_path.json      (v0.53a — custom storage path config;
//!                            read at startup BEFORE the pool is
//!                            open, since the pool's location
//!                            depends on this file)

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// App data dir (created if missing on first call).
/// All other path helpers in this module are derived from this.
pub fn app_data_dir(app: &AppHandle) -> crate::AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::AppError::Internal(format!("app_data_dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// v0.56 — best-effort OS-default app data dir
/// WITHOUT an AppHandle. Used by the early
/// startup hook (before the Tauri setup runs) to
/// load `network_proxy.json`.
///
/// The platform convention matches Tauri's
/// `app.path().app_data_dir()`:
///   - macOS: `~/Library/Application Support/<bundle_id>`
///   - Linux: `${XDG_DATA_HOME:-~/.local/share}/<bundle_id>`
///   - Windows: `%APPDATA%\<bundle_id>`
///
/// When the dir doesn't exist yet (first
/// launch), we return `None` so callers can
/// fall through to the "no proxy" path.
pub fn default_app_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        let p = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.polyrocket.app");
        Some(p)
    }
    #[cfg(target_os = "linux")]
    {
        let xdg = std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local").join("share"))
            })?;
        Some(xdg.join("com.polyrocket.app"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").ok()?;
        Some(PathBuf::from(appdata).join("com.polyrocket.app"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Path to `polyrocket.db` (does NOT touch the file — pure path computation).
/// 解析默认 DB 路径：`<app_data_dir>/polyrocket.db`。
///
/// **不**考虑 `storage_path.json`（用户自定义路径走 `resolve_db_path`）。
/// 这个 fn 是「最后兜底」+ 测试用。
pub fn db_path(app: &AppHandle) -> crate::AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("polyrocket.db"))
}

/// Path to the app log directory (created if missing).
/// 解析默认日志目录：`<app_data_dir>/logs/`。**自动创建**（如果不存在）。
///
/// **不**考虑 `storage_path.json` —— 用 `resolve_log_dir` 走自定义路径。
pub fn log_dir(app: &AppHandle) -> crate::AppResult<PathBuf> {
    let dir = app_data_dir(app)?.join("logs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ============================================================
// v0.53a — custom storage path resolution
// ============================================================
//
// `storage_path.json` is a 1-line JSON file at
// `<app_data_dir>/storage_path.json` containing either
// `null` (use the default path) or a string (use
// the given absolute path). It is the source of
// truth for path resolution at startup — we read
// it BEFORE opening the pool, since the pool's
// location depends on it.
//
// The IPC `setStoragePath` writes both this file
// AND the `_polyrocket_settings.storage_path`
// row (the latter is for the L1 to display via
// `getStorageInfo`). The two are kept in sync
// because the IPC writes both atomically. If
// they ever diverge (corruption, manual edit),
// the JSON file wins on the next launch — that's
// the side that controls where the pool opens.
//
// The `storage_path.json` shape is intentionally
// minimal so users who hand-edit it can't break
// things:
//   {"path": null}                    → default
//   {"path": "/Users/me/polyrocket"}   → custom

const STORAGE_CONFIG_FILE: &str = "storage_path.json";

/// Read the `storage_path.json` and return the
/// custom path (absolute directory), or None
/// when the file is missing or `path` is null.
///
/// Pure std — no pool, no AppHandle. Used by
/// `lib.rs::run` before the pool is open.
pub fn read_custom_storage_path(app: &AppHandle) -> Option<PathBuf> {
    let path = app_data_dir(app).ok()?.join(STORAGE_CONFIG_FILE);
    let body = std::fs::read_to_string(&path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let inner = v.get("path")?;
    if inner.is_null() {
        return None;
    }
    let s = inner.as_str()?;
    let p = PathBuf::from(s);
    if p.is_absolute() && p.exists() && p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// Resolve the db path polyrocket will use THIS
/// launch. If `storage_path.json` says custom +
/// that path exists, use it. Otherwise use the
/// default `<app_data_dir>/polyrocket.db`.
pub fn resolve_db_path(app: &AppHandle) -> crate::AppResult<PathBuf> {
    if let Some(custom) = read_custom_storage_path(app) {
        Ok(custom.join("polyrocket.db"))
    } else {
        db_path(app)
    }
}

/// Resolve the log directory THIS launch. Same
/// resolution rules as `resolve_db_path` (custom
/// wins, otherwise default).
pub fn resolve_log_dir(app: &AppHandle) -> crate::AppResult<PathBuf> {
    if let Some(custom) = read_custom_storage_path(app) {
        let logs = custom.join("logs");
        std::fs::create_dir_all(&logs)?;
        Ok(logs)
    } else {
        log_dir(app)
    }
}

/// Build the SQLx connection URL for a given db path.
/// `mode=rwc` opens for read+write and creates if missing.
///
/// Build the SQLx connection URL for a given db path.
/// `mode=rwc` opens for read+write and creates if missing.
///
/// The path is run through [`url_path_encode`] to handle spaces, unicode,
/// and other characters that the SQLx URL parser would otherwise choke on.
///
/// **为什么不直接用 `path` 字符串**：SQLx 要求 `sqlite://` 协议头 + `mode=rwc` 标记
///（read+write+create）。少了 `mode` 会让首次启动时 DB 不存在而 connection 失败。
pub fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", url_path_encode(&path.to_string_lossy()))
}

/// Percent-encode a path string for use in a `sqlite://` URL.
/// Encodes everything outside the unreserved set + a few path-safe
/// characters (`/`, `.`, `-`, `_`, `~`).
fn url_path_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let ok = b.is_ascii_alphanumeric()
            || matches!(b, b'/' | b'.' | b'-' | b'_' | b'~' | b':');
        if ok {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_url_simple() {
        let p = std::path::Path::new("/tmp/polyrocket.db");
        assert_eq!(sqlite_url(p), "sqlite:///tmp/polyrocket.db?mode=rwc");
    }

    #[test]
    fn sqlite_url_escapes_spaces_and_unicode() {
        let p = std::path::PathBuf::from(
            "/Users/test/Library/Application Support/com.polyrocket.app/polyrocket.db",
        );
        let url = sqlite_url(&p);
        assert!(url.starts_with("sqlite:///Users/test/Library/Application%20Support/"));
        assert!(url.ends_with("?mode=rwc"));
        // No raw spaces should remain
        assert!(!url.contains(' '));
    }

    #[test]
    fn url_path_encode_unreserved_passes_through() {
        assert_eq!(url_path_encode("/tmp/db"), "/tmp/db");
        assert_eq!(url_path_encode("a-b_c.d~e:1"), "a-b_c.d~e:1");
    }

    #[test]
    fn url_path_encode_special_chars() {
        assert_eq!(url_path_encode("a b"), "a%20b");
        assert_eq!(url_path_encode("中文"), "%E4%B8%AD%E6%96%87");
    }
}
