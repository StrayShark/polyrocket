//! L5 —— 环境与开发态 .env 同步。
//!
//! 两项职责,按用途分离:
//!   1. `parse_env_file` —— 纯解析器,无 IO 副作用,无环境访问。
//!      在启动时(仅开发态)以及测试中使用。
//!   2. `maybe_load_dev_env` + `sync_env_to_keyring` —— 读取
//!      `POLYROCKET_ENV` / `POLYROCKET_KEYRING_ONLY` 开关,然后
//!      可选地从 `.env` 向 OS 钥匙串播种。
//!
//! 开关规则(治理规范 §11)
//! -------------------------
//! - `POLYROCKET_ENV=dev` 且 `POLYROCKET_KEYRING_ONLY=0` → 读取 `.env`,
//!   把每个识别到的 `*_API_KEY` 写入钥匙串的规范别名,
//!   **仅在该别名当前为空时**。
//! - 其他任何组合 → 无操作。客户端粘贴路径
//!   (Settings → LLM Management → Add key) 是唯一的可信源。

use std::time::Duration;
use std::env;
use crate::platform::keyring;

// ============================================================
// 1. 纯 .env 解析器(无 IO 副作用,无环境访问)
// ============================================================

/// 将 `.env` 文件解析为 `(key, value)` 对。
///
/// 规则:
/// - 空行和 `# 注释` 被跳过
/// - 接受 `export FOO=bar`(剥离前缀 `export `)
/// - 剥离首尾的 `"` 或 `'` 引号
/// - 空值被跳过
/// - 不含 `=` 的行被跳过,并打印 `tracing::warn`
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
// 2. 开发态 .env → 钥匙串同步(受开关控制)
// ============================================================

/// 读取开关环境变量。满足以下任一条件时返回 true:
///   - `POLYROCKET_ENV == "dev"` 且 `POLYROCKET_KEYRING_ONLY != "1"`(旧逻辑)
///   - 钥匙串被禁用(`POLYROCKET_USE_KEYRING != "1"`)—— env-only 模式
///     现在是 v0.119 的默认,`.env` 是所有密钥(包括 LLM API key)
///     的唯一可信源。
///
/// v0.119 —— 新增第二条分支,因为 env-only 模式已成为新默认:
/// 当绕过钥匙串时,必须把 `.env` 读入进程环境,
/// 以便下游的 `std::env::var` 查询(LLM 分发、PM CLOB、
/// 代理端点)能够找到它们。否则 `MINIMAX_API_KEY`
/// 会保持未设置,LLM 调用将以 "env unset" 失败。
fn is_dev_sync_enabled() -> bool {
    let env_name = env::var("POLYROCKET_ENV").unwrap_or_default();
    let keyring_only = env::var("POLYROCKET_KEYRING_ONLY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let use_keyring = env::var("POLYROCKET_USE_KEYRING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if env_name == "dev" && !keyring_only { return true; }
    if !use_keyring { return true; } // env-only 模式是 v0.119 的默认
    false
}

// ============================================================
// v0.122a —— Python sidecar 迁移的紧急停止开关。
//
// `POLYROCKET_DISABLE_SIDECAR=1` 会短路每个 sidecar Tauri 命令
// (predict / train_job / promote_model 等),返回清晰的 "disabled" 错误。
// 用作 v0.122b-f 增量迁移期间的回退出口;v0.122g 之后
// 该标志失效,sidecar 进程完全消失。
//
// 语义时间线:
//   v0.122a(当前)    : 标志未设 → Python sidecar(当前)。
//                      标志已设   → "sidecar disabled" 错误。
//   v0.122b-v0.122f  : 标志未设 → 新的 Rust 实现(按迁移方法)。
//                      标志已设   → 任何未迁移的方法回退到 Python。
//   v0.122g+         : 不再读取该标志(Python 已消失)。
//
// 真值: "1"、"true"、"yes"(大小写不敏感)。
// ============================================================

/// 当用户显式设置 `POLYROCKET_DISABLE_SIDECAR=1`
/// 以退出 Python sidecar 时返回 true。
///
/// **v0.122a**: 默认 = false。Python sidecar 仍是所有 11 个 sidecar 方法的权威路径。
/// **v0.122b+**: 默认 = true(迁移完成后生效)。
/// 在此之前,调用方看到 "sidecar disabled",可通过取消该标志
/// 回退到 Python 处理尚未迁移的方法。
pub fn is_sidecar_disabled() -> bool {
    env::var("POLYROCKET_DISABLE_SIDECAR")
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

/// 当 [`is_sidecar_disabled`] 返回 true 时,
/// 每个 sidecar IPC 入口点返回的标准错误字符串。该措辞已冻结
/// —— v0.122 集成测试匹配此字符串。
pub const SIDECAR_DISABLED_MSG: &str =
    "sidecar disabled (POLYROCKET_DISABLE_SIDECAR=1; v0.122+ migration in progress)";

/// 入口函数:从 `lib.rs::run()` 的 setup 钩子调用。
/// 读取 `.env`(若存在),然后:
///   1. **v0.119** —— 通过 `std::env::set_var` 将加载的键
///      导出到进程环境,以便下游代码(LLM 分发、PM CLOB
///      状态等)可以直接 `std::env::var`。之前 `.env` 仅用于
///      向钥匙串播种;而在 v0.119 env-only 模式下钥匙串被绕过
///      —— 导致进程环境一直为空,所有操作失败。本函数修复了该回归。
///   2. 将识别到的键镜像到 OS 钥匙串,以向后兼容
///      v0.117 客户端粘贴路径。PM 凭据始终同步;
///      LLM 键受 `POLYROCKET_ENV=dev` 开关控制。
///
/// v0.119 —— 从 CWD 向上查找 `.env`。当从 `src-tauri/` 调用
/// `cargo run` 时,CWD 是 `src-tauri/`,但项目 `.env` 在上一级。
/// 同时支持环境变量 `POLYROCKET_ENV_FILE`(若已设置)。
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

    // v0.119 —— Layer 1.5: 把加载的键导出到进程环境。
    // PM 键始终注入。LLM 键仅在 dev 同步开关打开时注入
    // (POLYROCKET_ENV=dev 且 POLYROCKET_KEYRING_ONLY!=1)
    // —— 与旧版钥匙串同步路径使用相同的开关,确保生产环境
    // 绝不会意外读取开发态凭据。
    let dev_mode = is_dev_sync_enabled();
    let mut injected = 0usize;
    for (k, v) in &pairs {
        let is_pm = k.starts_with("POLYMARKET_") || k.starts_with("POLYROCKET_CLOB_");
        let is_llm = k.ends_with("_API_KEY") || k.ends_with("_API_SECRET") || k.ends_with("_BASE_URL");
        // v0.124 —— 同时转发代理 URL。HTTP 客户端工厂
        // 读取 `POLYROCKET_PROXY` 以通过用户的本地代理路由
        // (从屏蔽直接出站到那些主机的网络访问
        // gamma-api.polymarket.com + clob.polymarket.com 时需要)。
        // 始终注入,使 sync IPC 和 wallet balance IPC
        // 都无需用户在 shell 中手动 export 即可工作。
        let is_proxy = k == "POLYROCKET_PROXY";
        if is_pm || is_proxy {
            // 始终注入 —— env-only 是默认模式。
            if env::var(k).is_err() {
                // env::set_var 在新版 Rust 中是 unsafe;用显式 unsafe 块保护
                // (或使用 #[allow],因为这是单线程启动)。
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

    // Layer 1: PM 凭据 —— 始终同步(不受开关控制)。
    sync_pm_to_keyring(&pairs);

    // Layer 2: LLM API 键 —— 受 dev 模式开关控制。
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

/// 查找项目的 `.env` 文件。搜索顺序:
///   1. `POLYROCKET_ENV_FILE` 环境变量(若已设置且文件存在)
///   2. CWD/.env
///   3. 从 CWD 向上逐层查找父目录中的 `.env` —— 在文件系统根或 8 层后停止
///   4. `~/global_env/.env`(跨项目共享凭据)
///   5. 放弃。
///
/// 一旦找到路径,会与 `~/global_env/.env` 合并,以使 PM 凭据
///(以及其他本地缺失的键)来自全局源。合并结果写入
/// 一个 per-process 临时文件并返回;`parse_env_file` 从该文件读取。
fn find_env_file() -> Option<std::path::PathBuf> {
    let mut local: Option<std::path::PathBuf> = None;
    // 1. 显式覆盖
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
    // 3. 向上遍历
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
    // 4. 全局回退(跨项目共享凭据)
    let global = env::var("HOME").ok()
        .map(|h| std::path::PathBuf::from(h).join("global_env").join(".env"))
        .filter(|p| p.exists());

    match (local.as_ref(), global.as_ref()) {
        (None, None) => None,
        (Some(l), None) => Some(l.clone()),
        (None, Some(g)) => Some(g.clone()),
        (Some(l), Some(g)) => {
            // 合并:本地在冲突时胜出,但本地缺失的 PM 键和 LLM 键
            // 来自全局。写入临时文件,以使 parse_env_file
            // 不需要合并 API。
            merge_env_files(l, g)
        }
    }
}

/// 将本地 .env 与全局 .env 合并,写入临时文件。本地键在冲突时胜出;
/// 全局键用于填补缺失。返回临时文件路径。
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

/// 将 Polymarket CLOB 凭据从 `.env` 镜像到钥匙串。
/// 每次启动都会执行,不受 dev 模式开关影响 —— 用户显式
/// 在 `.env` 中编写 PM 密钥,并期望它们自动流入钥匙串。
/// 跳过任何已经填充的别名(永不覆盖)。
///
/// v0.119 —— 当 `is_disabled()`(默认)时,本函数为空操作。密钥
/// 仅存在于 `.env`,通过 `keyring::get_key` 的环境变量回退
/// 直接读取。避免 macOS 钥匙串的 ACL 提示阻塞无头测试
/// 和开发工作流。
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

/// 将 LLM API 密钥和钱包密钥从 `.env` 镜像到钥匙串。
/// 受 `POLYROCKET_ENV=dev` 开关控制(开发态便利功能)。生产环境
/// 使用 Settings → LLM Management → Add key 路径。
/// - 跳过任何已经填充的别名(永不覆盖)。
/// - 永不记录密钥本身。
fn sync_llm_to_keyring(pairs: &[(String, String)]) {
    use std::collections::HashMap;
    let map: HashMap<String, String> = pairs.iter().cloned().collect();
    let mut written = 0usize;
    let mut skipped = 0usize;

    // LLM 提供商 —— 环境变量名 → (provider_id, key_alias)
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

    // 钱包 —— 同样受 dev 模式开关控制(生产环境使用 onboarding 流程)
    if let Some(v) = map.get("POLYROCKET_WALLET_PRIVATE_KEY") {
        let alias = env::var("POLYROCKET_WALLET_ALIAS").unwrap_or_else(|_| "primary".to_string());
        let a = keyring::wallet_alias(&alias);
        if !keyring::has_key(&a) { let _ = keyring::set_key(&a, v); written += 1; } else { skipped += 1; }
    }

    tracing::info!("startup: LLM .env sync done — {written} written, {skipped} already present");
}

// ============================================================
// 3. 类型化环境变量助手(由 infra/scheduler.rs 使用)
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
/// v0.42a —— 字符串型环境变量。未设置或设置为空时返回 `None`。
/// 其他类型化助手也使用 `var().ok().and_then(parse)`,
/// 并静默地将不可解析值视为 "使用默认值";
/// 而遥测需要原始的 "1" / "true" 字符串匹配,
/// 因此本函数返回原始的 `Option<String>`。
pub fn env_str(name: &str) -> Option<String> {
    env::var(name).ok().filter(|s| !s.is_empty())
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_kv() {
        let p = std::path::Path::new("/tmp/_nope_env");
        let pairs = parse_env_file(p);
        assert!(pairs.is_empty()); // 文件缺失 → []
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
        assert_eq!(get("EMPTY_VAR"), None);  // 空值被跳过
        assert_eq!(get("QUOTED").as_deref(), Some("value with spaces"));
        assert_eq!(get("SINGLE").as_deref(), Some("single quoted"));
        assert_eq!(get("PRE").as_deref(), Some("exported-value"));
        assert_eq!(get("NOEQUALS"), None);  // 没有 '=' → 跳过
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn env_helpers_default_when_unset() {
        // 使用一个在本测试进程中从未设置过的变量名。
        let v = env_u64("POLYROCKET_TEST_NONEXISTENT_U64", 42);
        assert_eq!(v, 42);
        let v = env_u32("POLYROCKET_TEST_NONEXISTENT_U32", 7);
        assert_eq!(v, 7);
        let v = env_i32("POLYROCKET_TEST_NONEXISTENT_I32", -3);
        assert_eq!(v, -3);
    }

    // ----- v0.122a: POLYROCKET_DISABLE_SIDECAR 标志测试 -----

    /// 助手:清除环境变量以便测试默认分支。
    fn clear_sidecar_flag() {
        // SAFETY: 通过同一个 `env_helpers` 测试模块串行化。
        // 没有并发测试设置此变量。
        unsafe { env::remove_var("POLYROCKET_DISABLE_SIDECAR") };
    }

    #[test]
    #[serial_test::serial]
    fn sidecar_disabled_defaults_to_false() {
        clear_sidecar_flag();
        assert!(
            !is_sidecar_disabled(),
            "默认(变量未设置)必须为 false —— 在 v0.122b 之前 Python sidecar 仍是路径"
        );
    }

    #[test]
    #[serial_test::serial]
    fn sidecar_disabled_recognises_truthy_values() {
        for truthy in ["1", "true", "TRUE", "True", "yes", "YES"] {
            // SAFETY: 参见 clear_sidecar_flag。
            unsafe { env::set_var("POLYROCKET_DISABLE_SIDECAR", truthy) };
            assert!(
                is_sidecar_disabled(),
                "POLYROCKET_DISABLE_SIDECAR={truthy:?} 必须被识别"
            );
        }
        clear_sidecar_flag();
    }

    #[test]
    #[serial_test::serial]
    fn sidecar_disabled_rejects_falsy_values() {
        for falsy in ["0", "false", "no", "off", "anything-else"] {
            // SAFETY: 参见 clear_sidecar_flag。
            unsafe { env::set_var("POLYROCKET_DISABLE_SIDECAR", falsy) };
            assert!(
                !is_sidecar_disabled(),
                "POLYROCKET_DISABLE_SIDECAR={falsy:?} 必须禁用 sidecar"
            );
        }
        clear_sidecar_flag();
    }

    #[test]
    #[serial_test::serial]
    fn sidecar_disabled_trims_whitespace() {
        // SAFETY: 参见 clear_sidecar_flag。
        unsafe { env::set_var("POLYROCKET_DISABLE_SIDECAR", "  1  ") };
        assert!(
            is_sidecar_disabled(),
            "首尾空白不能改变真值语义"
        );
        clear_sidecar_flag();
    }

    #[test]
    fn sidecar_disabled_msg_is_stable() {
        // v0.122 集成测试匹配此确切字符串。
        // 若改写错误信息,需同步更新测试。
        assert_eq!(
            SIDECAR_DISABLED_MSG,
            "sidecar disabled (POLYROCKET_DISABLE_SIDECAR=1; v0.122+ migration in progress)"
        );
    }
}
