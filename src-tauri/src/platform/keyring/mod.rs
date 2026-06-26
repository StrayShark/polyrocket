//! L5 —— 跨平台 OS 钥匙串适配器。
//!
//! `keyring` crate(3.x)在内部处理各 OS 差异
//!(apple-native、windows-native、Linux 通过 dbus 的
//! sync-secret-service)。本模块是一个薄包装:
//!   1. 固定服务名(`com.polyrocket.wallet`),全应用共享。
//!   2. 添加上游 crate 未提供的若干助手(`has_key`、批量删除)。
//!   3. 重新导出 `aliases.rs` 中的别名构造函数。
//!
//! **安全模型**:此处密钥字符串为 `String`(而非 `&str`),
//! 因为我们接受调用方的所有权。我们永不记录密钥。
//! 永不将密钥写入 SQLite、JSON 或任何文件。
//!
//! **v0.119 —— 环境变量优先,无 UI 提示**:
//!
//! OS 钥匙串(macOS Keychain / Windows Credential Manager /
//! Linux Secret Service)**默认禁用**。所有密钥均来自
//! `.env`(通过进程环境变量)。原因:
//!   - `.env` 是单一可信源(已通过符号链接到
//!     `~/global_env/.env` 共享跨项目凭据)。
//!   - macOS 钥匙串 ACL 弹窗会在临时签名二进制上永远阻塞无头测试
//!     (并且会在开发中途打断用户)。
//!   - UI 永不要求用户输入密码/API 密钥 ——
//!     它们就放在 `.env` 中,完毕。
//!
//! 若要重新启用 OS 钥匙串存储(生产加固部署,
//! 避免密钥留在文件系统),设置:
//!   `POLYROCKET_USE_KEYRING=1`
//!
//! 设置该标志后,`set_key` 写入 OS 钥匙串,`get_key` 从
//! OS 钥匙串读取,不再查询 `.env`。UI 的密码输入框
//! 也会重新生效。

use keyring::Entry;

/// 向 OS 凭据存储注册的服务名。这是每个 polyrocket 别名所处的 "命名空间"。
const SERVICE: &str = "com.polyrocket.wallet";

/// 当 OS 钥匙串应被跳过、改用进程环境变量时返回 true。
/// 默认 = TRUE(仅使用环境变量)。设置 `POLYROCKET_USE_KEYRING=1`
/// 可重新启用钥匙串存储(生产加固模式)。
pub fn is_disabled() -> bool {
    // 反向逻辑:钥匙串默认关闭。
    let enabled = std::env::var("POLYROCKET_USE_KEYRING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    !enabled
}

/// 将钥匙串别名映射为进程环境变量名。仅在 `is_disabled()` 为 true
/// 时使用,以便 dev 模式绕过能够从 `std::env::var` 而非
/// OS 钥匙串找到密钥。
fn alias_to_env_var(alias: &str) -> Option<String> {
    match alias {
        // Polymarket CLOB(对应 `aliases.rs` 中的 pm_*_alias())
        "polyrocket/pm/api"        => Some("POLYMARKET_API_KEY".to_string()),
        "polyrocket/pm/secret"     => Some("POLYMARKET_API_SECRET".to_string()),
        "polyrocket/pm/passphrase" => Some("POLYMARKET_API_PASSPHRASE".to_string()),
        // 钱包 —— `polyrocket/wallet/{name}` → POLYROCKET_WALLET_PRIVATE_KEY
        a if a.starts_with("polyrocket/wallet/") => {
            Some("POLYROCKET_WALLET_PRIVATE_KEY".to_string())
        }
        // LLM —— `llm/{provider_id}/{alias}` → {PROVIDER}_API_KEY
        // 示例:
        //   llm/openai/openai-prod-1        → OPENAI_API_KEY
        //   llm/openai/openai-backup        → OPENAI_BACKUP_KEY
        //   llm/anthropic/anthropic-prod-1  → ANTHROPIC_API_KEY
        a if a.starts_with("llm/") => {
            // 剥离 "llm/" 前缀 → "{provider_id}/{alias}"
            let rest = &a[4..];
            let parts: Vec<&str> = rest.splitn(2, '/').collect();
            if parts.len() != 2 { return None; }
            let provider_id = parts[0];
            let key_alias = parts[1];
            // 将 (provider_id, key_alias) 映射为环境变量名。
            // 与 env.rs::sync_env_to_keyring 中的表保持一致,
            // 确保 dev 绕过与同步层写入的内容同步。
            let env_var = match (provider_id, key_alias) {
                ("openai", "openai-prod-1")     => "OPENAI_API_KEY",
                ("openai", "openai-backup")     => "OPENAI_BACKUP_KEY",
                ("anthropic", "anthropic-prod-1") => "ANTHROPIC_API_KEY",
                ("anthropic", "anthropic-backup") => "ANTHROPIC_BACKUP_KEY",
                ("google", "google-prod-1")     => "GOOGLE_API_KEY",
                ("deepseek", "deepseek-prod-1") => "DEEPSEEK_API_KEY",
                ("custom", "custom-prod-1")     => "CUSTOM_LLM_API_KEY",
                // 中国 LLM 提供商 —— 由 e2e_football bootstrap
                // 和 WelcomeStep 流程使用。别名为简写 "prod-1" 形式。
                ("MiniMax", "prod-1") => "MINIMAX_API_KEY",
                ("doubao",  "prod-1") => "DOUBAO_API_KEY",
                ("qwen",     "prod-1") => "QWEN_API_KEY",
                ("moonshot", "prod-1") => "MOONSHOT_API_KEY",
                ("zhipu",    "prod-1") => "ZHIPU_API_KEY",
                ("hunyuan",  "prod-1") => "HUNYUAN_API_KEY",
                // 通用回退:对任何别名使用 "{PROVIDER}_API_KEY"。
                // 这可覆盖所有简写别名("prod-1"、"prod-2"、"main" 等)。
                _ => {
                    return Some(format!("{}_API_KEY", provider_id.to_uppercase()));
                }
            };
            Some(env_var.to_string())
        }
        _ => None,
    }
}

/// 构造一个 `Entry`(惰性 —— 无 IO)。所有错误均为 `keyring::Error`,
/// 在调用处进行转换。
fn entry(alias: &str) -> crate::AppResult<Entry> {
    Entry::new(SERVICE, alias).map_err(|e| {
        crate::AppError::Internal(format!("keyring entry({alias}): {e}"))
    })
}

// ---------- 公共原始 API ----------

/// 在 `alias` 下存储一个密钥。覆盖该别名下的现有值。
pub fn set_key(alias: &str, secret: &str) -> crate::AppResult<()> {
    if is_disabled() {
        // dev 模式下为空操作。密钥仅保存在进程环境/.env 中。
        // 此处直接返回 Ok 会掩盖未真正写入的事实;
        // 因此改用 tracing 警告,让开发者知道调用是空操作。
        tracing::warn!(
            "keyring.set_key({alias}): bypass (POLYROCKET_DEV_NO_KEYRING=1); secret stays in env"
        );
        return Ok(());
    }
    let e = entry(alias)?;
    e.set_password(secret).map_err(|e| {
        crate::AppError::Internal(format!("keyring set_password({alias}): {e}"))
    })
}

/// 读取 `alias` 处的密钥。若条目不存在或 OS 拒绝访问,
/// 返回 `AppError::Internal`(我们故意不区分两种情况
/// —— 审计日志不需要泄露 "别名存在但被锁定" 与 "不存在" 的差异)。
///
/// v0.119 —— dev 模式绕过:当 `POLYROCKET_DEV_NO_KEYRING=1` 时,
/// 回退到进程环境变量(通过 `alias_to_env_var` 映射)。
pub fn get_key(alias: &str) -> crate::AppResult<String> {
    if is_disabled() {
        let env_var = alias_to_env_var(alias).ok_or_else(|| {
            crate::AppError::Internal(format!(
                "keyring.get_key({alias}): dev bypass enabled but no env-var mapping known"
            ))
        })?;
        return match std::env::var(&env_var) {
            Ok(v) if !v.is_empty() => Ok(v),
            Ok(_) => Err(crate::AppError::Internal(format!(
                "keyring.get_key({alias}): dev bypass — env {env_var} is empty"
            ))),
            Err(_) => Err(crate::AppError::Internal(format!(
                "keyring.get_key({alias}): dev bypass — env {env_var} unset"
            ))),
        };
    }
    let e = entry(alias)?;
    e.get_password().map_err(|err| {
        crate::AppError::Internal(format!("keyring get_password({alias}): {err}"))
    })
}

/// 删除 `alias` 处的密钥。幂等 —— 缺失的条目被静默视为成功。
pub fn delete_key(alias: &str) -> crate::AppResult<()> {
    if is_disabled() {
        // 无需删除 —— 密钥保存在进程环境中。
        return Ok(());
    }
    let e = entry(alias)?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(crate::AppError::Internal(format!(
            "keyring delete_credential({alias}): {err}"
        ))),
    }
}

/// 低开销的 "密钥是否存在且非空" 检查。
///
/// v0.119 —— dev 模式绕过:改为检查进程环境而非 OS 钥匙串。
pub fn has_key(alias: &str) -> bool {
    if is_disabled() {
        return alias_to_env_var(alias)
            .and_then(|env_var| std::env::var(&env_var).ok())
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    }
    matches!(get_key(alias), Ok(s) if !s.is_empty())
}

pub mod aliases;
pub use aliases::*;
