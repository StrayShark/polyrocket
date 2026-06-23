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

/// Read the gate env vars. Returns true when one of:
///   - `POLYROCKET_ENV == "dev"` AND `POLYROCKET_KEYRING_ONLY != "1"` (legacy)
///   - keyring is disabled (`POLYROCKET_USE_KEYRING != "1"`) — env-only mode
///     is now the v0.119 default, and `.env` is the source of truth for
///     ALL secrets including LLM API keys.
///
/// v0.119 — second branch added because the env-only mode is the new
/// default: when keyring is bypassed, `.env` MUST be read into process
/// env so downstream `std::env::var` lookups (LLM dispatch, PM CLOB,
/// proxy endpoints) can find them. Without this, `MINIMAX_API_KEY`
/// stays unset and the LLM call fails with "env unset".
fn is_dev_sync_enabled() -> bool {
    let env_name = env::var("POLYROCKET_ENV").unwrap_or_default();
    let keyring_only = env::var("POLYROCKET_KEYRING_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let use_keyring = env::var("POLYROCKET_USE_KEYRING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if env_name == "dev" && !keyring_only { return true; }
    if !use_keyring { return true; } // env-only mode is the v0.119 default
    false
}

/// Entry point: called from `lib.rs::run()` setup hook.
/// Reads `.env` (if it exists), then:
///   1. **v0.119** — exports loaded keys into the process env via
///      `std::env::set_var` so downstream code (LLM dispatch, PM CLOB
///      status, etc.) can `std::env::var` them directly. Previously
///      `.env` was only used to seed the keyring; the keyring was then
///      bypassed in v0.119 env-only mode — so the process env stayed
///      empty and everything failed. This fixes that regression.
///   2. Mirrors recognised keys to the OS keyring for backwards
///      compat with the v0.117 client-paste path. PM creds always
///      synced; LLM keys gated by `POLYROCKET_ENV=dev`.
///
/// v0.119 — walks up from CWD to find `.env`. When `cargo run` is invoked
/// from `src-tauri/`, CWD is `src-tauri/` but the project `.env` lives
/// one level up. Also honors `POLYROCKET_ENV_FILE` if set.
pub fn maybe_load_dev_env() {
    let env_path = find_env_file();
    let Some(env_path) = env_path else {
        tracing::info!(
            "startup: no .env found (cwd={:?}) — nothing to load",
            env::current_dir().ok()
        );
        return;
    };
    let pairs = parse_env_file(&env_path);

    // v0.119 — Layer 1.5: export loaded keys into process env.
    // PM keys are always injected. LLM keys only injected when the dev
    // sync gate is open (POLYROCKET_ENV=dev AND POLYROCKET_KEYRING_ONLY!=1)
    // — same gating as the legacy keyring-sync path so prod never
    // accidentally reads dev creds.
    let dev_mode = is_dev_sync_enabled();
    let mut injected = 0usize;
    for (k, v) in &pairs {
        let is_pm = k.starts_with("POLYMARKET_") || k.starts_with("POLYROCKET_CLOB_");
        let is_llm = k.ends_with("_API_KEY") || k.ends_with("_API_SECRET") || k.ends_with("_BASE_URL");
        if is_pm {
            // Always inject — env-only is the default mode.
            if env::var(k).is_err() {
                // env::set_var is unsafe in newer Rust; guard with explicit unsafe block
                // (or use #[allow] since this is single-threaded startup).
                #[allow(unused_unsafe)]
                unsafe { env::set_var(k, v); }
                injected += 1;
            }
        } else if is_llm && dev_mode {
            if env::var(k).is_err() {
                #[allow(unused_unsafe)]
                unsafe { env::set_var(k, v); }
                injected += 1;
            }
        }
    }
    tracing::info!(
        "startup: injected {} env vars from {} (dev_mode={})",
        injected,
        env_path.display(),
        dev_mode
    );

    // Layer 1: PM credentials — always sync (not gated).
    sync_pm_to_keyring(&pairs);

    // Layer 2: LLM API keys — gated by dev mode.
    if !dev_mode {
        tracing::info!(
            "startup: LLM .env sync disabled (POLYROCKET_ENV / POLYROCKET_KEYRING_ONLY)"
        );
        return;
    }
    tracing::info!(
        "startup: dev .env sync — {} entries from {}",
        pairs.len(),
        env_path.display()
    );
    sync_llm_to_keyring(&pairs);
}

/// Find the project's `.env` file. Search order:
///   1. `POLYROCKET_ENV_FILE` env var (if set + exists)
///   2. CWD/.env
///   3. Walk up parent directories from CWD, looking for `.env` at each
///      level — stop at filesystem root or after 8 levels.
///   4. `~/global_env/.env` (cross-project shared credentials)
///   5. give up.
///
/// Once a path is found, merge with `~/global_env/.env` so PM credentials
/// (and any other keys missing locally) come from the global source.
/// The merged result is written to a per-process temp file and returned;
/// `parse_env_file` reads from that.
fn find_env_file() -> Option<std::path::PathBuf> {
    let mut local: Option<std::path::PathBuf> = None;
    // 1. explicit override
    if let Ok(p) = env::var("POLYROCKET_ENV_FILE") {
        let path = std::path::PathBuf::from(p);
        if path.exists() { local = Some(path); }
    }
    // 2. CWD/.env
    if local.is_none() {
        if let Ok(cwd) = env::current_dir() {
            let cand = cwd.join(".env");
            if cand.exists() { local = Some(cand); }
        }
    }
    // 3. walk up
    if local.is_none() {
        if let Ok(cwd) = env::current_dir() {
            let mut dir = cwd.as_path();
            for _ in 0..8 {
                let cand = dir.join(".env");
                if cand.exists() { local = Some(cand); break; }
                match dir.parent() {
                    Some(p) => dir = p,
                    None => break,
                }
            }
        }
    }
    // 4. global fallback (cross-project shared creds)
    let global = env::var("HOME").ok()
        .map(|h| std::path::PathBuf::from(h).join("global_env").join(".env"))
        .filter(|p| p.exists());

    match (local.as_ref(), global.as_ref()) {
        (None, None) => None,
        (Some(l), None) => Some(l.clone()),
        (None, Some(g)) => Some(g.clone()),
        (Some(l), Some(g)) => {
            // Merge: local wins on conflict, but PM keys + LLM keys that are
            // missing locally come from global. Writes to a temp file so
            // parse_env_file doesn't need a merge API.
            merge_env_files(l, g)
        }
    }
}

/// Merge local .env with global .env, writing to a temp file. Local wins on
/// conflicts; missing keys come from global. Returns the temp file path.
fn merge_env_files(local: &std::path::Path, global: &std::path::Path) -> Option<std::path::PathBuf> {
    let local_pairs = parse_env_file(local);
    let global_pairs = parse_env_file(global);
    let local_len = local_pairs.len();
    let global_len = global_pairs.len();
    use std::collections::HashMap;
    let mut merged: HashMap<String, String> = HashMap::new();
    for (k, v) in global_pairs {
        merged.insert(k, v);
    }
    for (k, v) in local_pairs {
        if !v.is_empty() {
            merged.insert(k, v);
        } else if !merged.contains_key(&k) {
            merged.insert(k, v);
        }
    }
    let tmp = std::env::temp_dir().join(format!(
        "polyrocket-env-merged-{}-{}.env",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let mut out = String::new();
    let mut keys: Vec<&String> = merged.keys().collect();
    keys.sort();
    for k in keys {
        if let Some(v) = merged.get(k) {
            out.push_str(&format!("{k}={v}\n"));
        }
    }
    if std::fs::write(&tmp, &out).is_ok() {
        tracing::info!(
            "startup: merged .env (local={} keys, global={} keys, output={})",
            local_len,
            global_len,
            tmp.display()
        );
        Some(tmp)
    } else {
        None
    }
}

/// Mirror Polymarket CLOB credentials from `.env` to keychain.
/// Runs on every boot regardless of dev-mode gate — users explicitly
/// author PM keys in `.env` and expect them to flow into the keychain
/// automatically. Skips any alias already populated (never overwrites).
///
/// v0.119 — when `is_disabled()` (default), this is a no-op. The
    /// secrets live in `.env` only, read directly via `keyring::get_key`'s
    /// env fallback. Saves the macOS Keychain ACL prompt that would
    /// otherwise block headless tests + dev workflow.
    fn sync_pm_to_keyring(pairs: &[(String, String)]) {
        use std::collections::HashMap;
        let map: HashMap<String, String> = pairs.iter().cloned().collect();

        if crate::platform::keyring::is_disabled() {
            let have_api = map.get("POLYMARKET_API_KEY").map(|v| !v.is_empty()).unwrap_or(false);
            tracing::info!(
                "startup: PM .env-only mode active (no keyring); pm_api_present={}",
                have_api
            );
            return;
        }

    let pm_triple: &[(&str, &str)] = &[
        ("POLYMARKET_API_KEY",        "polyrocket/pm/api"),
        ("POLYMARKET_API_SECRET",     "polyrocket/pm/secret"),
        ("POLYMARKET_API_PASSPHRASE", "polyrocket/pm/passphrase"),
    ];
    let mut written = 0usize;
    let mut skipped = 0usize;
    let mut missing = 0usize;
    for (env_var, alias) in pm_triple {
        match map.get(*env_var).filter(|v| !v.is_empty()) {
            Some(v) => {
                if keyring::has_key(alias) {
                    skipped += 1;
                } else if keyring::set_key(alias, v).is_ok() {
                    written += 1;
                    tracing::info!("startup: seeded PM keychain {alias} from .env");
                }
            }
            None => missing += 1,
        }
    }
    if written > 0 || skipped > 0 || missing > 0 {
        tracing::info!(
            "startup: PM .env sync — written={} skipped(already-keyed)={} missing={}",
            written, skipped, missing
        );
    }
}

/// Mirror LLM API keys + wallet key from `.env` to keychain.
/// Gated by `POLYROCKET_ENV=dev` (dev convenience). Production uses
/// Settings → LLM Management → Add key path.
/// - Skips any alias already populated (never overwrites).
/// - Never logs the secret.
fn sync_llm_to_keyring(pairs: &[(String, String)]) {
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

    // Wallet — also gated by dev mode (production uses onboarding flow)
    if let Some(v) = map.get("POLYROCKET_WALLET_PRIVATE_KEY") {
        let alias = env::var("POLYROCKET_WALLET_ALIAS").unwrap_or_else(|_| "primary".to_string());
        let a = keyring::wallet_alias(&alias);
        if !keyring::has_key(&a) { let _ = keyring::set_key(&a, v); written += 1; } else { skipped += 1; }
    }

    tracing::info!("startup: LLM .env sync done — {written} written, {skipped} already present");
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
