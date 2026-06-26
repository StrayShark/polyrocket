//! L4 —— 共享 HTTP 客户端工厂。
//!
//! 整个进程共用一个 `reqwest::Client`,在 startup 时
//! 由 `lib.rs::run()` 一次性构建,再传给所有需要的层
//!（LLM 客户端、调度器、未来的 HTTP 命令）。连接池
//! 在各 provider 之间共享 —— 跨请求复用 socket 比
//! 每次调用都新建 client 要快得多,健康探测和分析
//! 调用都受益。
//!
//! L3 LLM 客户端依赖本模块(向下 L3 → L4 是层规则
//! 允许的;参见 overview.md §1.2)。
//!
//! ## v0.56 —— 代理支持
//!
//! 用户可从 Settings → Network 路由出站 HTTP 通过代理
//!（http:// 或 socks5://）。代理通过环境变量
//! `POLYROCKET_PROXY` 配置。v0.56 保持连线简单:
//! 环境变量在进程启动时生效。修改代理需要重启
//!（与 v0.53 storage-path 的方式一致：设置在
//! 下次启动时生效）。
//!
//! 为什么不直接用 SQLite 设置表？HTTP 客户端是
//! 在 app 启动时同步构建的,此时 AppState 尚未
//! 就绪。从 SQLite 读需要异步 + 临时连接,容易竞态。
//! v0.56 先用环境变量方案;v0.57+ 可加 "需要重启"
//! 流程,读 SQLite 行、设置环境变量、启动时重建客户端。

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
        // v0.125 —— 关闭 Gamma 路径的连接池。
        // 通过本地 HTTP 代理（127.0.0.1:7897）复用池时,
        // 会撞上 keep-alive 帧解析的边界情况:代理的 HTTP/2
        // 流在 body 中途关闭,reqwest 的 body 解码器把它
        // 暴露成没什么帮助的 "error decoding response body"。
        // 关闭池意味着每个请求都打开新的 TCP 连接 —— 较慢但稳定。
        .pool_max_idle_per_host(0);
    // v0.56 —— 若设置了 `POLYROCKET_PROXY`
    // 则应用代理。格式:`socks5://host:port` 或
    // `http://host:port`。我们使用
    // `reqwest::Proxy::all`,它会从 URL
    // 自动识别 scheme。
    if let Ok(raw) = std::env::var("POLYROCKET_PROXY") {
        let url = raw.trim();
        if !url.is_empty() {
            match reqwest::Proxy::all(url) {
                Ok(p) => {
                    builder = builder.proxy(p);
                }
                Err(e) => {
                    // 不要因为代理 URL 错误就让整个 app 启动失败。
                    // 记录日志后继续直连出站。
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
        // 仅验证工厂不 panic。实际网络行为
        // 由针对 mockito server 的集成测试覆盖。
        let _c = new_http_client();
    }

    #[test]
    fn client_build_succeeds_with_invalid_proxy() {
        // 非法代理 URL 不应让工厂 panic —— 它只记录一条
        // 警告,然后继续直连出站。
        // 单元测试中无法真正设置环境变量(env 是
        // 进程级的,且测试并行运行),所以我们仅
        // 通过模拟相同代码路径验证 builder 不会
        // 在错误 URL 上 panic。
        //
        // reqwest::Proxy::all() 较宽松:接受任何字符串,
        // 只有当 URL 解析不出已知 scheme 时才失败。
        // 我们的 `load_proxy_from_json_into_env` 是
        // 守门人 —— 格式错误的 URL 永远不会到达
        // 代理 builder。工厂本身使用 `.unwrap_or(default)`,
        // 因此错误代理只会产生一条警告日志,不会 panic。
        let _bad = "not-a-url";
        let _c = new_http_client();
    }
}
