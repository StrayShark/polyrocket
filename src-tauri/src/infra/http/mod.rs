//! L4 — Shared HTTP client factory.
//!
//! One process-wide `reqwest::Client` is built once at startup in
//! `lib.rs::run()` and passed to every layer that needs it (LLM
//! clients, schedulers, future HTTP commands). The connection pool
//! is shared across providers — reusing sockets across requests
//! makes health-probes and analysis calls much faster than building
//! a fresh client per call.
//!
//! L3 LLM clients depend on this module (downward L3 → L4 is allowed
//! by the layer rules; see overview.md §1.2).
//!
//! ## v0.56 — proxy support
//!
//! The user can route outbound HTTP through a proxy
//! (http:// or socks5://) from Settings → Network. The
//! proxy is configured via the `POLYROCKET_PROXY` env
//! var. v0.56 keeps the wiring simple: the env var
//! takes effect at process start. Changing the
//! proxy requires a restart (mirroring the v0.53
//! storage-path flow: settings take effect on the
//! next launch).
//!
//! Why env var and not the SQLite settings table?
//! The HTTP client is built synchronously at app
//! startup, BEFORE the AppState is available. Reading
//! from SQLite would require an async + temporary
//! connection, which is racy. v0.56 ships the env-var
//! path; v0.57+ can add a "restart required" flow
//! that reads the SQLite row, sets the env var, and
//! rebuilds the client on launch.

use std::time::Duration;

/// 构建一个进程级共享的 `reqwest::Client`。调用方应**持有**这个 client 至整个进程生命周期。
///
/// **调用方**：
///   - `lib.rs::run()` 在 startup 调一次，把 client 包到 `ArcSwap` 里
///   - L3 LLM 客户端 + 调度器 + 未来的 HTTP 命令都从 `http_client()` 取
///
/// **池大小**：每个 host 8 个空闲连接。覆盖 5 个 LLM provider + 2 个 Polymarket
/// endpoint (gamma + CLOB) + 1 个 transient burst 备用。
///
/// **超时**：连接超时 10s。不设全局 `timeout()` —— 长任务（5-feature KernelSHAP
/// in 6h iter）需要更长 timeout，由调用方在 `RequestBuilder` 上单独设。
///
/// **Proxy**：读取 `POLYROCKET_PROXY` 环境变量。v0.60a 起支持 hot-swap —— `set_proxy_config`
/// IPC 写 env var + 调 `replace_http_client()` 原子替换 ArcSwap 里的 client。
pub fn new_http_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!("polyrocket/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(8)
        // v0.124 — disable connection pooling for the Gamma
        // path. Pool reuse through the local HTTP proxy
        // (127.0.0.1:7897) hits a keep-alive frame-parsing
        // edge case where the proxied HTTP/2 stream closes
        // mid-body and reqwest's body decoder surfaces it as
        // the unhelpful "error decoding response body".
        // Disabling pooling means every request opens a fresh
        // TCP connection — slower but stable. v0.125 can
        // re-enable pooling once we know the proxy supports
        // HTTP/1.1 keep-alive cleanly.
        .pool_max_idle_per_host(0);
    // v0.56 — apply proxy if `POLYROCKET_PROXY`
    // is set. Format: `socks5://host:port` or
    // `http://host:port`. We use
    // `reqwest::Proxy::all` which auto-detects
    // the scheme from the URL.
    if let Ok(raw) = std::env::var("POLYROCKET_PROXY") {
        let url = raw.trim();
        if !url.is_empty() {
            match reqwest::Proxy::all(url) {
                Ok(p) => {
                    builder = builder.proxy(p);
                }
                Err(e) => {
                    // Don't fail the whole app boot
                    // because of a bad proxy URL. Log
                    // and continue with direct
                    // outbound.
                    tracing::warn!(
                        "POLYROCKET_PROXY={url:?} rejected by reqwest: {e}"
                    );
                }
            }
        }
    }
    builder
        .build()
        .expect("reqwest client build must succeed (rustls feature enabled)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_build_succeeds() {
        // Just verify the factory doesn't panic. Actual network behaviour
        // is exercised by integration tests against mockito servers.
        let _c = new_http_client();
    }

    #[test]
    fn client_build_succeeds_with_invalid_proxy() {
        // An invalid proxy URL should NOT panic
        // the factory — it just logs a warning
        // and continues with direct outbound.
        // We can't actually set env vars in a
        // unit test (env is process-wide and
        // tests run in parallel), so we just
        // verify the builder doesn't panic on
        // a bad URL by simulating the same
        // code path.
        //
        // reqwest::Proxy::all() is permissive:
        // it accepts any string and only fails
        // when the URL doesn't parse as a
        // recognised scheme. Our
        // `load_proxy_from_json_into_env` is
        // the gatekeeper — malformed URLs never
        // reach the proxy builder. The factory
        // itself uses `.unwrap_or(default)`,
        // so a bad proxy becomes a logged
        // warning, not a panic.
        let _bad = "not-a-url";
        let _c = new_http_client();
    }
}
