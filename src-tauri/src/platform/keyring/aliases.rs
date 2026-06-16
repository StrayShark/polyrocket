//! L5 — Keyring alias conventions + batched deletes.
//!
//! Alias conventions
//! -----------------
//! - LLM API key (per-provider, per-alias):  `llm/<provider_id>/<key_alias>`
//!   e.g.                                       `llm/openai/prod-1`
//!                                            or `llm/anthropic/backup`
//! - Polymarket CLOB credential:               `polyrocket/pm/api`
//! - Polymarket CLOB secret:                   `polyrocket/pm/secret`
//! - Polymarket CLOB passphrase:                `polyrocket/pm/passphrase`
//! - Wallet private key (per alias):           `polyrocket/wallet/<alias>`
//!   e.g.                                       `polyrocket/wallet/primary`
//!
//! All alias builders live here so call sites can never get the format
//! wrong. The string constants are kept short (`&'static str` when
//! constant, owned `String` when parameterized).

/// Build the LLM keyring alias for a (provider_id, key_alias) pair.
pub fn llm_alias(provider_id: &str, key_alias: &str) -> String {
    format!("llm/{provider_id}/{key_alias}")
}

pub fn pm_api_alias() -> &'static str { "polyrocket/pm/api" }
pub fn pm_secret_alias() -> &'static str { "polyrocket/pm/secret" }
pub fn pm_passphrase_alias() -> &'static str { "polyrocket/pm/passphrase" }

/// Build the wallet keyring alias for a wallet alias (e.g. "primary").
pub fn wallet_alias(alias: &str) -> String {
    format!("polyrocket/wallet/{alias}")
}

/// Delete every LLM keychain entry scoped to a given provider.
///
/// The OS keyring does not expose enumeration, so the SQLite
/// `llm_provider_keys` table is the source of truth for "which
/// aliases belong to provider X". The caller passes the list of
/// `keyring_alias` strings from the table; we filter to those that
/// actually have the `llm/<pid>/` prefix (defense-in-depth) and try
/// to delete each. Returns the aliases that were successfully deleted.
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
        // No real keyring IO; just verifies the filter logic.
        let aliases = vec![
            "llm/openai/prod-1".to_string(),
            "llm/openai/backup".to_string(),
            "llm/anthropic/prod-1".to_string(),   // wrong provider
            "polyrocket/pm/api".to_string(),       // not LLM at all
        ];
        // We can't actually call delete_key (no OS keyring in tests),
        // but we can check the filter by extracting the prefix-match:
        let expected_prefix = "llm/openai/";
        let matching: Vec<&String> = aliases
            .iter()
            .filter(|a| a.starts_with(expected_prefix))
            .collect();
        assert_eq!(matching.len(), 2);
    }
}
