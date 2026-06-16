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

use std::time::Duration;

/// Build a long-lived HTTP client. Caller is expected to keep this
/// for the process lifetime.
///
/// Pool size 8 per host covers our 5 LLM providers + 2 Polymarket
/// endpoints (gamma + CLOB) + 1 spare for transient bursts.
pub fn new_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("polyrocket/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(8)
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
}
