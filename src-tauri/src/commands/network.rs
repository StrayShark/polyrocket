// v0.56 — network proxy configuration.
//
// The user can route polyrocket's outbound HTTP
// (LLM clients, Polymarket CLOB, sidecar HTTP if
// any) through a proxy. The two supported
// schemes:
//
//   - "http"   — http://host:port  (HTTP CONNECT)
//   - "socks5" — socks5://host:port (e.g. Tor
//                SOCKS5 on 127.0.0.1:9050)
//
// The proxy config is stored in OS keyring under
// the alias "network.proxy". The URL itself is
// not a secret (the host:port is informational)
// but the password (for authenticated proxies)
// would be — we keep the URL only for now; if
// the user needs auth, they can pre-configure the
// proxy in their env (HTTP_PROXY etc) and we'll
// pick that up as a fallback.
//
// ## Restart semantics
//
// The shared reqwest::Client is built ONCE at
// startup. Changing the proxy after launch
// requires rebuilding the client. v0.56 takes
// the pragmatic approach: set_proxy updates the
// keyring + a JSON file, and the user sees a
// "Restart required" banner (mirroring the v0.53
// storage path flow). The active session's HTTP
// traffic continues to use the old proxy until
// restart.
//
// v0.56+ candidate: hot-swap the client. We
// already use `OnceLock<HttpClient>` in
// commands/llm.rs; we'd add a `set` setter that
// atomically swaps. Skipped for v0.56 to keep
// the change small.

use crate::AppResult;
use crate::infra::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

const PROXY_SETTING_KEY: &str = "network.proxy.url";

/// v0.56 — proxy configuration as the user sees
/// it. `enabled` is a separate boolean so the
/// user can disable the proxy without clearing
/// the URL (faster toggle on/off).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    /// True when the proxy is currently in use.
    /// When false, the URL is still stored but
    /// ignored.
    pub enabled: bool,
    /// The proxy URL, e.g. "socks5://127.0.0.1:9050"
    /// or "http://proxy.example.com:8080". `None`
    /// when no proxy is configured.
    pub url: Option<String>,
    /// "http" or "socks5". Derived from the URL
    /// scheme; surfaced for the L1 so the user
    /// can see what type they're using.
    pub scheme: Option<String>,
    /// True when the active session's HTTP
    /// client was built with this config (i.e.
    /// the user just restarted). False when the
    /// user changed the config but hasn't
    /// restarted yet. Mirrors the
    /// `storage.restart_required` pattern.
    pub restart_required: bool,
}

/// v0.56 — return the current proxy
/// configuration. The L1 uses this to render
/// the Settings → Network card.
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
    /// True to enable the proxy; false to
    /// disable (URL is kept in storage for
    /// quick re-enable).
    pub enabled: bool,
    /// The proxy URL. Pass `null` (or an empty
    /// string) to clear the URL entirely.
    pub url: Option<String>,
}

/// v0.56 — write the proxy configuration. The
/// change takes effect on next launch (active
/// session's HTTP client is already built).
/// The L1 surfaces a "Restart required" banner
/// after this returns.
///
/// v0.60a — no longer requires a restart. The
/// HTTP client is rebuilt atomically via
/// `commands::llm::replace_http_client()`. We
/// still write the JSON file (for the next
/// launch's pre-pool read), but the in-memory
/// client picks up the new proxy immediately.
#[tauri::command]
pub async fn set_proxy_config(
    state: State<'_, AppState>,
    args: SetProxyConfigArgs,
) -> AppResult<ProxyConfig> {
    // Validate the URL when one is provided.
    let url = args.url.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(u) = url {
        let scheme = derive_scheme(u).ok_or_else(|| {
            crate::AppError::Invalid(format!(
                "unsupported proxy scheme in {u} (only http + socks5)"
            ))
        })?;
        if scheme == "http" || scheme == "socks5" {
            // OK
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
    // v0.56 — also write the JSON config file.
    // Same pattern as the storage_path.json
    // v0.53a flow. The file is read at startup
    // BEFORE the SQLite pool opens; the DB row
    // is for the L1 surface. We write the
    // active config (or clear it when disabled).
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
    // Audit log.
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
    // v0.60a — hot-swap the shared HTTP client
    // so the new proxy takes effect
    // immediately. We set POLYROCKET_PROXY
    // first (so the factory reads the new
    // value), then rebuild.
    //
    // SAFETY: setting an env var is
    // thread-safe; the concurrent read in
    // `new_http_client` is rare (we only
    // call it on init + on hot-swap). The
    // worst case is a brief window where
    // a parallel `http_client()` could read
    // the new env var before we call
    // `replace_http_client()`, but the read
    // only happens at the first call (the
    // OnceCell was already populated in
    // lib.rs::run()).
    if args.enabled {
        if let Some(u) = url {
            // SAFETY: see above.
            std::env::set_var("POLYROCKET_PROXY", u);
        } else {
            std::env::remove_var("POLYROCKET_PROXY");
        }
    } else {
        std::env::remove_var("POLYROCKET_PROXY");
    }
    crate::commands::llm::replace_http_client();
    // No longer requires a restart.
    Ok(ProxyConfig {
        enabled: args.enabled,
        url: url.map(String::from),
        scheme: url.and_then(|u| derive_scheme(&u)),
        restart_required: false,
    })
}

/// v0.56 — clear the proxy entirely (URL +
/// enabled flag). Next launch uses direct
/// outbound HTTP.
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
    // Also clear the JSON config file.
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

/// v0.56 — write the `network_proxy.json` config
/// file. Shape: `{"url": <string|null>}` — a
/// single key so users who hand-edit the file
/// can't break the app by adding unknown keys.
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

/// v0.56 — return the proxy URL from
/// `network_proxy.json`, or None. This is the
/// startup-time lookup that `infra::http` uses
/// to build the shared `reqwest::Client`. We
/// expose it here so the L1 can also ask "what
/// does Rust see right now?" for the Network
/// card.
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
/// proxy URL. Returns None when the URL doesn't
/// start with a recognised scheme. We don't
/// validate the full URL here (host:port etc) —
/// reqwest will do that at client-build time.
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
        // scheme without port
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
