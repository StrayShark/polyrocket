//! L5 —— 应用文件系统路径。
//!
//! 应用数据目录下所有文件 IO 都通过这里进行,
//! 以便从一处审计目录布局。
//!
//! 布局(依据 `tauri::path::PathResolver::app_data_dir`)
//! --------------------------------------------------------------
//! macOS:   ~/Library/Application Support/com.polyrocket.app/
//! Linux:   ~/.local/share/com.polyrocket.app/
//! Windows: %APPDATA%\com.polyrocket.app\
//!
//! 我们拥有的文件:
//!   polyrocket.db          (SQLite 数据库 —— 由 L4 infra/db/pool.rs 拥有)
//!   polyrocket.db-wal      (SQLite WAL —— SQLite 内部)
//!   polyrocket.db-shm      (SQLite 共享内存 —— SQLite 内部)
//!   logs/polyrocket.log    (Tauri 应用日志 —— 由 Tauri 插件拥有)
//!   storage_path.json      (v0.53a —— 自定义存储路径配置;
//!                            在启动时、连接池打开**之前**读取,
//!                            因为连接池的位置取决于此文件)

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// 应用数据目录(首次调用时若缺失则创建)。
/// 本模块的其他路径助手均从此派生。
pub fn app_data_dir(app: &AppHandle) -> crate::AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::AppError::Internal(format!("app_data_dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// v0.56 —— 尽力获取 OS 默认的应用数据目录,**不**依赖 AppHandle。
/// 供早期启动钩子(Tauri setup 之前)用于
/// 加载 `network_proxy.json`。
///
/// 各平台的目录约定与 Tauri 的
/// `app.path().app_data_dir()` 一致:
///   - macOS: `~/Library/Application Support/<bundle_id>`
///   - Linux: `${XDG_DATA_HOME:-~/.local/share}/<bundle_id>`
///   - Windows: `%APPDATA%\<bundle_id>`
///
/// 当目录尚未存在(首次启动)时,
/// 返回 `None`,以便调用方回退到 "无代理" 路径。
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

/// `polyrocket.db` 的路径(不触碰文件 —— 纯路径计算)。
/// 解析默认 DB 路径：`<app_data_dir>/polyrocket.db`。
///
/// **不**考虑 `storage_path.json`(用户自定义路径走 `resolve_db_path`)。
/// 这个 fn 是「最后兜底」+ 测试用。
pub fn db_path(app: &AppHandle) -> crate::AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("polyrocket.db"))
}

/// 应用日志目录的路径（如果不存在则创建）。
/// 解析默认日志目录：`<app_data_dir>/logs/`。**自动创建**（如果不存在）。
///
/// **不**考虑 `storage_path.json` —— 用 `resolve_log_dir` 走自定义路径。
pub fn log_dir(app: &AppHandle) -> crate::AppResult<PathBuf> {
    let dir = app_data_dir(app)?.join("logs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ============================================================
// v0.53a —— 自定义存储路径解析
// ============================================================
//
// `storage_path.json` 是位于 `<app_data_dir>/storage_path.json`
// 的单行 JSON 文件,内容为 `null`(使用默认路径)
// 或一个字符串(使用指定的绝对路径)。
// 它是启动时路径解析的可信源 —— 我们在打开连接池**之前**
// 读取它,因为连接池的位置取决于它。
//
// IPC `setStoragePath` 会同时写入此文件以及
// `_polyrocket_settings.storage_path` 行
// (后者供 L1 通过 `getStorageInfo` 显示)。
// 两者保持同步,因为 IPC 原子地写入二者。
// 如果它们出现分歧(损坏、手动编辑),
// 下次启动时以 JSON 文件为准 ——
// 因为 JSON 文件控制连接池的打开位置。
//
// `storage_path.json` 的结构刻意保持最小,
// 这样手动编辑的用户无法破坏配置:
//   {"path": null}                    → 默认
//   {"path": "/Users/me/polyrocket"}   → 自定义

const STORAGE_CONFIG_FILE: &str = "storage_path.json";

/// 读取 `storage_path.json` 并返回自定义路径
/// (绝对目录);当文件缺失或 `path` 为 null 时返回 None。
///
/// 纯标准库 —— 不依赖连接池、不依赖 AppHandle。
/// 在打开连接池之前,由 `lib.rs::run` 使用。
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

/// 解析 polyrocket 本次启动使用的数据库路径。
/// 若 `storage_path.json` 指定自定义路径且该路径存在,
/// 则使用它;否则使用默认的 `<app_data_dir>/polyrocket.db`。
pub fn resolve_db_path(app: &AppHandle) -> crate::AppResult<PathBuf> {
    if let Some(custom) = read_custom_storage_path(app) {
        Ok(custom.join("polyrocket.db"))
    } else {
        db_path(app)
    }
}

/// 解析本次启动使用的日志目录。解析规则与
/// `resolve_db_path` 相同(自定义路径优先,否则默认)。
pub fn resolve_log_dir(app: &AppHandle) -> crate::AppResult<PathBuf> {
    if let Some(custom) = read_custom_storage_path(app) {
        let logs = custom.join("logs");
        std::fs::create_dir_all(&logs)?;
        Ok(logs)
    } else {
        log_dir(app)
    }
}

/// 为指定数据库路径构造 SQLx 连接 URL。
/// `mode=rwc` 表示以读写模式打开,文件不存在时创建。
///
/// 为指定数据库路径构造 SQLx 连接 URL。
/// `mode=rwc` 表示以读写模式打开,文件不存在时创建。
///
/// 路径会经过 [`url_path_encode`] 处理,以兼容空格、Unicode
/// 以及 SQLx URL 解析器无法处理的其它字符。
///
/// **为什么不直接用 `path` 字符串**：SQLx 要求 `sqlite://` 协议头 + `mode=rwc` 标记
///(read+write+create)。少了 `mode` 会让首次启动时 DB 不存在而 connection 失败。
pub fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", url_path_encode(&path.to_string_lossy()))
}

/// 对用于 `sqlite://` URL 的路径字符串进行百分号编码。
/// 对未保留集合之外的字符,以及若干路径安全字符
/// (`/`、`.`、`-`、`_`、`~`) 进行编码。
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
// 测试
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
        // 不应残留原始空格
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
