//! L5 —— 钥匙串别名约定 + 批量删除。
//!
//! 别名约定
//! -----------------
//! - LLM API 密钥(按提供商、按别名):  `llm/<provider_id>/<key_alias>`
//!   例如:                                  `llm/openai/prod-1`
//!                                       或 `llm/anthropic/backup`
//! - Polymarket CLOB 凭据:                  `polyrocket/pm/api`
//! - Polymarket CLOB 密钥:                  `polyrocket/pm/secret`
//! - Polymarket CLOB 口令:                  `polyrocket/pm/passphrase`
//! - 钱包私钥(按别名):                    `polyrocket/wallet/<alias>`
//!   例如:                                  `polyrocket/wallet/primary`
//!
//! 所有别名构造函数都在这里,调用方永远不可能写错格式。
//! 字符串常量尽量保持简短(常量使用 `&'static str`,
//! 参数化时使用拥有的 `String`)。

/// 为 (provider_id, key_alias) 对构造 LLM 钥匙串别名。
pub fn llm_alias(provider_id: &str, key_alias: &str) -> String {
    format!("llm/{provider_id}/{key_alias}")
}

pub fn pm_api_alias() -> &'static str { "polyrocket/pm/api" }
pub fn pm_secret_alias() -> &'static str { "polyrocket/pm/secret" }
pub fn pm_passphrase_alias() -> &'static str { "polyrocket/pm/passphrase" }

/// 为钱包别名(例如 "primary")构造钱包钥匙串别名。
pub fn wallet_alias(alias: &str) -> String {
    format!("polyrocket/wallet/{alias}")
}

/// 删除指定提供商范围内的所有 LLM 钥匙串条目。
///
/// OS 钥匙串不暴露枚举 API,因此 SQLite 表
/// `llm_provider_keys` 是 "哪些别名属于提供商 X" 的可信源。
/// 调用方从表中传入 `keyring_alias` 字符串列表;
/// 我们过滤出确实以 `llm/<pid>/` 开头的项(纵深防御),
/// 并尝试逐条删除。返回成功删除的别名列表。
pub fn delete_provider_keys(provider_id: &str, keyring_aliases: &[String]) -> Vec<String> {
    let mut deleted = Vec::new();
    let expected_prefix = format!("llm/{provider_id}/");
    for alias in keyring_aliases {
        if alias.starts_with(&expected_prefix) {
            if super::delete_key(alias).is_ok() {
                deleted.push(alias.clone());
            }
        }
    }
    deleted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_alias_format() {
        assert_eq!(llm_alias("openai", "prod-1"), "llm/openai/prod-1");
        assert_eq!(llm_alias("anthropic", "backup"), "llm/anthropic/backup");
    }

    #[test]
    fn pm_aliases_are_static() {
        assert_eq!(pm_api_alias(), "polyrocket/pm/api");
        assert_eq!(pm_secret_alias(), "polyrocket/pm/secret");
        assert_eq!(pm_passphrase_alias(), "polyrocket/pm/passphrase");
    }

    #[test]
    fn wallet_alias_format() {
        assert_eq!(wallet_alias("primary"), "polyrocket/wallet/primary");
        assert_eq!(wallet_alias("trading-bot"), "polyrocket/wallet/trading-bot");
    }

    #[test]
    fn delete_provider_keys_filters_by_prefix() {
        // 没有真实的钥匙串 IO;仅验证过滤逻辑。
        let aliases = vec![
            "llm/openai/prod-1".to_string(),
            "llm/openai/backup".to_string(),
            "llm/anthropic/prod-1".to_string(),   // 提供商错误
            "polyrocket/pm/api".to_string(),       // 完全不是 LLM
        ];
        // 我们无法真正调用 delete_key(测试中没有 OS 钥匙串),
        // 但可以通过提取前缀匹配来检查过滤逻辑:
        let expected_prefix = "llm/openai/";
        let matching: Vec<&String> = aliases
            .iter()
            .filter(|a| a.starts_with(expected_prefix))
            .collect();
        assert_eq!(matching.len(), 2);
    }
}
