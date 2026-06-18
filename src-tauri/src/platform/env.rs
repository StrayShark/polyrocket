//! L5 — Environment + dev .env sync.
//!
//! Two responsibilities, separated by purpose:
//!   1. `parse_env_file` — pure parser, no IO side effects, no env access.
//!      Used at startup (dev only) and by tests.
//!   2. `maybe_load_dev_env` + `sync_env_to_keyring` — read the
//!      `POLYROCKET_ENV` / `POLYROCKET_KEYRING_ONLY` gates, then
//!      optionally seed the OS keyring from `.env`.
//!
//! Gate rules (governance §11)
//! -------------------------
//! - `POLYROCKET_ENV=dev` AND `POLYROCKET_KEYRING_ONLY=0` → read `.env`,
//!   write every recognised `*_API_KEY` to the keyring under its
//!   canonical alias **only if that alias is currently empty**.
//! - In any other combination → no-op. The client-paste path
//!   (Settings → LLM Management → Add key) is the only source of truth.

use std::time::Duration;
use std::env;
use crate::platform::keyring;

// ============================================================
// 1. Pure .env parser (no IO side effects, no env access)
// ============================================================

/// Parse a `.env` file into `(key, value)` pairs.
///
/// Rules:
/// - empty lines and `# comments` are skipped
/// - `export FOO=bar` is accepted (leading `export ` stripped)
/// - surrounding `"` or `'` quotes are stripped
/// - empty values are skipped
/// - lines without `=` are skipped with a tracing::warn
pub fn parse_env_file(path: &std::path::Path) -> Vec<(String, String)> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let mut out = Vec::new();
    for (lineno, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        let Some((k, v)) = line.split_once('=') else {
            tracing::warn!(".env line {}: missing '=' (skipped)", lineno + 1);
            continue;
        };
        let key = k.trim().to_string();
        let val = v.trim();
        let val = if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
            || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
        {
            &val[1..val.len() - 1]
        } else {
            val
        };
        if val.is_empty() {
            continue;
        }
        out.push((key, val.to_string()));
    }
    out
}

// ============================================================
// 2. Dev-mode .env → keyring sync (gated)
// ============================================================

/// Read the gate env vars. Returns true only when:
///   `POLYROCKET_ENV == "dev"` AND `POLYROCKET_KEYRING_ONLY != "1"`.
fn is_dev_sync_enabled() -> bool {
    let env_name = env::var("POLYROCKET_ENV").unwrap_or_default();
    if env_name != "dev" { return false; }
    let keyring_only = env::var("POLYROCKET_KEYRING_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    !keyring_only
}

/// Entry point: called from `lib.rs::run()` setup hook.
/// Reads `.env` (if it exists), then mirrors recognised keys to
/// the OS keyring — only for aliases that don't already have a value.
pub fn maybe_load_dev_env() {
    if !is_dev_sync_enabled() {
        tracing::info!(
            "startup: dev .env sync disabled (POLYROCKET_ENV / POLYROCKET_KEYRING_ONLY)"
        );
        return;
    }
    let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let env_path = cwd.join(".env");
    if !env_path.exists() {
        tracing::info!("startup: no .env at {} — nothing to sync", env_path.display());
        return;
    }
    let pairs = parse_env_file(&env_path);
    tracing::info!(
        "startup: dev .env sync — {} entries from {}",
        pairs.len(),
        env_path.display()
    );
    sync_env_to_keyring(&pairs);
}

/// Mirror recognised env vars to the OS keyring.
/// - For LLM: env var name maps to (provider_id, key_alias)
/// - For PM / wallet: known fixed aliases
/// - Skips any alias that is already populated (never overwrites)
/// - Never logs the secret
fn sync_env_to_keyring(pairs: &[(String, String)]) {
    use std::collections::HashMap;
    let map: HashMap<String, String> = pairs.iter().cloned().collect();
    let mut written = 0usize;
    let mut skipped = 0usize;

    // LLM providers — env var name → (provider_id, key_alias)
    let llm_map: &[(&str, &str, &str)] = &[
        ("OPENAI_API_KEY",       "openai",    "openai-prod-1"),
        ("OPENAI_BACKUP_KEY",    "openai",    "openai-backup"),
        ("ANTHROPIC_API_KEY",    "anthropic", "anthropic-prod-1"),
        ("ANTHROPIC_BACKUP_KEY", "anthropic", "anthropic-backup"),
        ("GOOGLE_API_KEY",       "google",    "google-prod-1"),
        ("DEEPSEEK_API_KEY",     "deepseek",  "deepseek-prod-1"),
        ("CUSTOM_LLM_API_KEY",   "custom",    "custom-prod-1"),
    ];
    for (env_var, provider_id, key_alias) in llm_map {
        if let Some(v) = map.get(*env_var) {
            let alias = keyring::llm_alias(provider_id, key_alias);
            if keyring::has_key(&alias) {
                skipped += 1;
            } else {
                if let Err(e) = keyring::set_key(&alias, v) {
                    tracing::warn!("startup: failed to write {alias}: {e}");
                } else {
                    written += 1;
                    tracing::info!("startup: seeded keychain {alias} from .env");
                }
            }
        }
    }

    // Polymarket CLOB
    if let Some(v) = map.get("POLYMARKET_API_KEY") {
        let a = keyring::pm_api_alias();
        if !keyring::has_key(a) { let _ = keyring::set_key(a, v); written += 1; } else { skipped += 1; }
    }
    if let Some(v) = map.get("POLYMARKET_API_SECRET") {
        let a = keyring::pm_secret_alias();
        if !keyring::has_key(a) { let _ = keyring::set_key(a, v); written += 1; } else { skipped += 1; }
    }
    if let Some(v) = map.get("POLYMARKET_API_PASSPHRASE") {
        let a = keyring::pm_passphrase_alias();
        if !keyring::has_key(a) { let _ = keyring::set_key(a, v); written += 1; } else { skipped += 1; }
    }

    // Wallet
    if let Some(v) = map.get("POLYROCKET_WALLET_PRIVATE_KEY") {
        let alias = env::var("POLYROCKET_WALLET_ALIAS").unwrap_or_else(|_| "primary".to_string());
        let a = keyring::wallet_alias(&alias);
        if !keyring::has_key(&a) { let _ = keyring::set_key(&a, v); written += 1; } else { skipped += 1; }
    }

    tracing::info!("startup: .env sync done — {written} written, {skipped} already present");
}

// ============================================================
// 3. Typed env helpers (used by infra/scheduler.rs)
// ============================================================

/// 读 env var → u64。**缺失或解析失败** → 返回 `default`。
///
/// **调用方**：scheduler (`SchedulerConfig::from_env`) 用这个读 `POLYROCKET_*_MIN/SEC`。
pub fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
/// 读 env var → u32。**缺失或解析失败** → 返回 `default`。
pub fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
/// 读 env var → i32。**缺失或解析失败** → 返回 `default`（包括负数）。
pub fn env_i32(name: &str, default: i32) -> i32 {
    env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
/// 读 env var → `Duration`（秒为单位）。**缺失或解析失败** → `default_secs`。
pub fn env_duration_secs(name: &str, default_secs: u64) -> Duration {
    Duration::from_secs(env_u64(name, default_secs))
}
/// v0.42a — string env var. Returns `None` if unset OR
/// set-but-empty. The other typed helpers also use
/// `var().ok().and_then(parse)` and silently treat
/// unparseable as "use default"; for telemetry we
/// need the raw "1" / "true" string match, so this
/// returns the raw `Option<String>`.
pub fn env_str(name: &str) -> Option<String> {
    env::var(name).ok().filter(|s| !s.is_empty())
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_kv() {
        let p = std::path::Path::new("/tmp/_nope_env");
        let pairs = parse_env_file(p);
        assert!(pairs.is_empty()); // missing file → []
    }

    #[test]
    fn parses_inline_content() {
        let dir = std::env::temp_dir().join("polyrocket_env_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        let body = [
            "# comment line",
            "OPENAI_API_KEY=sk-test-123",
            "EMPTY_VAR=",
            "QUOTED=\"value with spaces\"",
            "SINGLE='single quoted'",
            "export PRE=exported-value",
            "NOEQUALS",
            "",
        ].join("\n");
        std::fs::write(&path, body).unwrap();
        let pairs = parse_env_file(&path);
        let get = |k: &str| pairs.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone());
        assert_eq!(get("OPENAI_API_KEY").as_deref(), Some("sk-test-123"));
        assert_eq!(get("EMPTY_VAR"), None);  // empty value skipped
        assert_eq!(get("QUOTED").as_deref(), Some("value with spaces"));
        assert_eq!(get("SINGLE").as_deref(), Some("single quoted"));
        assert_eq!(get("PRE").as_deref(), Some("exported-value"));
        assert_eq!(get("NOEQUALS"), None);  // no '=' → skipped
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn env_helpers_default_when_unset() {
        // Use a name we never set in this test process.
        let v = env_u64("POLYROCKET_TEST_NONEXISTENT_U64", 42);
        assert_eq!(v, 42);
        let v = env_u32("POLYROCKET_TEST_NONEXISTENT_U32", 7);
        assert_eq!(v, 7);
        let v = env_i32("POLYROCKET_TEST_NONEXISTENT_I32", -3);
        assert_eq!(v, -3);
    }
}
