// v0.56 —— 网络代理配置。
//
// 用户可以将 polyrocket 的出站 HTTP
//（LLM 客户端、Polymarket CLOB、sidecar HTTP 等）
// 通过代理路由。支持的两种 scheme：
//
//   - "http"   —— http://host:port  （HTTP CONNECT）
//   - "socks5" —— socks5://host:port （例如 Tor
//                SOCKS5 在 127.0.0.1:9050）
//
// 代理配置存储在 OS keyring 中,别名为
// "network.proxy"。URL 本身不是秘密（host:port 仅是信息）,
// 但密码（用于需要认证的代理）是秘密 —— 目前仅保留 URL;
// 如果用户需要认证,可以预先在环境变量中配置代理
//（HTTP_PROXY 等）,我们将作为回退读取。
//
// ## 重启语义
//
// 共享的 reqwest::Client 在启动时仅构建一次。
// 启动后修改代理需要重建 client。v0.56 采取务实方案：
// set_proxy 更新 keyring + JSON 文件,用户看到
// 「需要重启」横幅（与 v0.53 storage path 流程一致）。
// 当前会话的 HTTP 流量在重启前继续使用旧代理。
//
// v0.56+ 候选方案：热替换 client。我们已经在
// commands/llm.rs 中使用 `OnceLock<HttpClient>`;
// 可增加一个 `set` setter,原子地替换。
// v0.56 暂不实现以保持改动最小。

use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

const PROXY_SETTING_KEY: &str = "network.proxy.url";

/// v0.56 —— 用户视角下的代理配置。`enabled` 是一个独立的布尔值,
/// 用户可以在不清空 URL 的情况下停用代理（更快的开关切换）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    /// 当前代理启用时为 true。当为 false 时,
    /// URL 仍会存储但被忽略。
    pub enabled: bool,
    /// 代理 URL,例如 "socks5://127.0.0.1:9050"
    /// 或 "http://proxy.example.com:8080"。未配置
    /// 代理时为 `None`。
    pub url: Option<String>,
    /// "http" 或 "socks5"。由 URL scheme 推导;
    /// 暴露给 L1,使用户看到正在使用的类型。
    pub scheme: Option<String>,
    /// 当当前会话的 HTTP 客户端是用此配置构建（即用户
    /// 刚刚重启）时为 true。当用户修改了配置但
    /// 尚未重启时为 false。与 `storage.restart_required`
    /// 模式一致。
    pub restart_required: bool,
}

/// v0.56 —— 返回当前代理配置。L1 用它渲染
/// Settings → Network 卡片。
#[tauri::command]
pub async fn get_proxy_config(
    state: State<'_, AppState>,
) -> AppResult<ProxyConfig> {
    let url: Option<String> =
        crate::infra::db::settings::get(&state.db, PROXY_SETTING_KEY)
            .await
            .map_err(|e| {
                crate::AppError::Internal(format!("settings.get: {e}"))
            })?;
    let enabled: bool = crate::infra::db::settings::get(
        &state.db,
        "network.proxy.enabled",
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("settings.get: {e}")))?
    .map(|s: String| s == "true")
    .unwrap_or(false);
    let cfg = ProxyConfig {
        enabled,
        url: url.clone(),
        scheme: url.as_deref().and_then(derive_scheme),
        restart_required: false, // v0.56: approximated
    };
    Ok(cfg)
}

#[derive(Debug, Deserialize)]
pub struct SetProxyConfigArgs {
    /// true 启用代理,false 停用（URL 仍保留在存储中,
    /// 便于快速重新启用）。
    pub enabled: bool,
    /// 代理 URL。传 `null`（或空字符串）
    /// 完全清空 URL。
    pub url: Option<String>,
}

/// v0.56 —— 写入代理配置。改动在下次启动时生效
///（当前会话的 HTTP 客户端已构建）。
/// 返回后 L1 会显示「需要重启」横幅。
///
/// v0.60a —— 不再需要重启。HTTP 客户端通过
/// `commands::llm::replace_http_client()` 原子重建。
/// 仍会写入 JSON 文件（供下次启动的预 pool 读取）,
/// 但内存中的客户端会立即使用新代理。
#[tauri::command]
pub async fn set_proxy_config(
    state: State<'_, AppState>,
    args: SetProxyConfigArgs,
) -> AppResult<ProxyConfig> {
    // 当提供了 URL 时进行校验。
    let url = args.url.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(u) = url {
        let scheme = derive_scheme(u).ok_or_else(|| {
            crate::AppError::Invalid(format!(
                "unsupported proxy scheme in {u} (only http + socks5)"
            ))
        })?;
        if scheme == "http" || scheme == "socks5" {
            // 合法
        } else {
            return Err(crate::AppError::Invalid(format!(
                "unsupported proxy scheme: {scheme}"
            )));
        }
    }
    crate::infra::db::settings::set(
        &state.db,
        PROXY_SETTING_KEY,
        url.unwrap_or(""),
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("settings.set: {e}")))?;
    crate::infra::db::settings::set(
        &state.db,
        "network.proxy.enabled",
        if args.enabled { "true" } else { "false" },
    )
    .await
    .map_err(|e| crate::AppError::Internal(format!("settings.set: {e}")))?;
    // v0.56 —— 同时写入 JSON 配置文件。
    // 与 storage_path.json 的 v0.53a 流程一致。
    // 文件在 SQLite pool 打开之前启动时读取;
    // DB 行用于 L1 展示。这里写入
    // 当前配置（disable 时清空）。
    let app = crate::infra::scheduler::TAURI_APP
        .get()
        .ok_or_else(|| {
            crate::AppError::Internal(
                "TAURI_APP not initialized (network.proxy.set)".into(),
            )
        })?
        .clone();
    if args.enabled {
        if let Some(u) = url {
            write_proxy_config_file(&app, Some(u))?;
        }
    } else {
        write_proxy_config_file(&app, None)?;
    }
    // 审计 log。
    let _ = sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result)
         VALUES ('user', 'network.proxy.set', 'network.proxy',
                 ?, 'ok')",
    )
    .bind(format!(
        "{{\"enabled\":{},\"url\":{:?}}}",
        args.enabled,
        url.unwrap_or("")
    ))
    .execute(&state.db)
    .await;
    // v0.60a —— 热替换共享的 HTTP 客户端,使新代理
    // 立即生效。先设置 POLYROCKET_PROXY
    //（让工厂读取新值）,然后重建。
    //
    // SAFETY：设置环境变量是线程安全的；
    // `new_http_client` 中的并发读很少（仅在 init
    // 与热替换时调用）。最坏情况是存在短暂窗口,
    // 并行的 `http_client()` 可能在调用
    // `replace_http_client()` 之前读到新 env var,
    // 但读仅在首次调用时发生（OnceCell 已在
    // lib.rs::run() 中填充）。
    if args.enabled {
        if let Some(u) = url {
            // SAFETY：见上。
            std::env::set_var("POLYROCKET_PROXY", u);
        } else {
            std::env::remove_var("POLYROCKET_PROXY");
        }
    } else {
        std::env::remove_var("POLYROCKET_PROXY");
    }
    crate::commands::llm::replace_http_client();
    // 不再需要重启。
    Ok(ProxyConfig {
        enabled: args.enabled,
        url: url.map(String::from),
        scheme: url.and_then(|u| derive_scheme(&u)),
        restart_required: false,
    })
}

/// v0.56 —— 完全清空代理（URL +
/// enabled 标志）。下次启动使用直接出站 HTTP。
#[tauri::command]
pub async fn clear_proxy_config(
    state: State<'_, AppState>,
) -> AppResult<()> {
    crate::infra::db::settings::delete(&state.db, PROXY_SETTING_KEY)
        .await
        .map_err(|e| {
            crate::AppError::Internal(format!("settings.delete: {e}"))
        })?;
    crate::infra::db::settings::delete(&state.db, "network.proxy.enabled")
        .await
        .map_err(|e| {
            crate::AppError::Internal(format!("settings.delete: {e}"))
        })?;
    // 同时清空 JSON 配置文件。
    let app = crate::infra::scheduler::TAURI_APP
        .get()
        .ok_or_else(|| {
            crate::AppError::Internal(
                "TAURI_APP not initialized (network.proxy.clear)".into(),
            )
        })?
        .clone();
    write_proxy_config_file(&app, None)?;
    let _ = sqlx::query(
        "INSERT INTO audit_log (actor, action, target, payload, result)
         VALUES ('user', 'network.proxy.clear', 'network.proxy', 'null', 'ok')",
    )
    .execute(&state.db)
    .await;
    Ok(())
}

/// v0.56 —— 写入 `network_proxy.json` 配置文件。
/// 形式：`{"url": <string|null>}` —— 仅一个键,
/// 这样手动编辑文件的用户无法通过添加未知键
/// 而破坏应用。
fn write_proxy_config_file(
    app: &AppHandle,
    url: Option<&str>,
) -> AppResult<()> {
    let dir = crate::platform::paths::app_data_dir(app)?;
    let file = dir.join("network_proxy.json");
    let body = match url {
        Some(u) => serde_json::json!({"url": u}),
        None => serde_json::json!({"url": null}),
    };
    let s = serde_json::to_string_pretty(&body).map_err(|e| {
        crate::AppError::Internal(format!(
            "serialize network_proxy.json: {e}"
        ))
    })?;
    std::fs::write(&file, s).map_err(|e| {
        crate::AppError::Internal(format!(
            "write network_proxy.json: {e}"
        ))
    })?;
    Ok(())
}

/// v0.56 —— 从 `network_proxy.json` 中返回代理 URL,
/// 或 None。这是 `infra::http` 在启动时用来构建
/// 共享 `reqwest::Client` 的查询点。我们把它暴露在
/// 这里,让 L1 也能询问「Rust 当前看到的是什么？」
///（用于 Network 卡片）。
#[tauri::command]
pub async fn read_proxy_config_file(
    app: AppHandle,
) -> AppResult<Option<String>> {
    let dir = crate::platform::paths::app_data_dir(&app)?;
    let file = dir.join("network_proxy.json");
    if !file.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&file).map_err(|e| {
        crate::AppError::Internal(format!("read network_proxy.json: {e}"))
    })?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        crate::AppError::Internal(format!(
            "parse network_proxy.json: {e}"
        ))
    })?;
    Ok(v.get("url")
        .and_then(|u| u.as_str())
        .map(String::from))
}
/// 推导代理 URL 的 scheme。URL 没有以可识别的 scheme
/// 开头时返回 None。此处不校验完整 URL（host:port 等）——
/// reqwest 在构建客户端时会做这一步。
fn derive_scheme(url: &str) -> Option<String> {
    if let Some(rest) = url.strip_prefix("socks5://") {
        if rest.contains(':') {
            return Some("socks5".to_string());
        }
    }
    if let Some(rest) = url.strip_prefix("http://") {
        if rest.contains(':') {
            return Some("http".to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_scheme_socks5() {
        assert_eq!(
            derive_scheme("socks5://127.0.0.1:9050"),
            Some("socks5".to_string())
        );
    }

    #[test]
    fn derive_scheme_http() {
        assert_eq!(
            derive_scheme("http://proxy.example.com:8080"),
            Some("http".to_string())
        );
    }

    #[test]
    fn derive_scheme_unknown_returns_none() {
        assert_eq!(derive_scheme("ftp://x:1"), None);
        assert_eq!(derive_scheme("not-a-url"), None);
        // scheme 但无端口
        assert_eq!(derive_scheme("socks5://localhost"), None);
    }

    #[test]
    fn proxy_config_default_is_empty() {
        let c = ProxyConfig::default();
        assert!(!c.enabled);
        assert!(c.url.is_none());
        assert!(c.scheme.is_none());
        assert!(!c.restart_required);
    }

    #[test]
    fn proxy_config_serializes_to_json() {
        let c = ProxyConfig {
            enabled: true,
            url: Some("socks5://127.0.0.1:9050".to_string()),
            scheme: Some("socks5".to_string()),
            restart_required: true,
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["enabled"], true);
        assert_eq!(v["url"], "socks5://127.0.0.1:9050");
        assert_eq!(v["scheme"], "socks5");
        assert_eq!(v["restart_required"], true);
    }
}
