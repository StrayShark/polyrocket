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

/// Path to `polyrocket.db` (does NOT touch the file — pure path computation).
pub fn db_path(app: &AppHandle) -> crate::AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("polyrocket.db"))
}

/// Path to the app log directory (created if missing).
pub fn log_dir(app: &AppHandle) -> crate::AppResult<PathBuf> {
    let dir = app_data_dir(app)?.join("logs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Build the SQLx connection URL for a given db path.
/// `mode=rwc` opens for read+write and creates if missing.
///
/// The path is run through [`url_path_encode`] to handle spaces, unicode,
/// and other characters that the SQLx URL parser would otherwise choke on.
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
