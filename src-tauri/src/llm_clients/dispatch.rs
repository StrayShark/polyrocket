//! Provider dispatch — key rotation + retry/backoff + per-call log.
//!
//! Public surface
//! --------------
//! - [`dispatch`] — given a list of eligible keys (alias + id) for a
//!   provider, pick the highest-priority enabled one, try the call,
//!   on retryable error advance to the next key. 429/5xx/timeout are
//!   retryable; 401/403/parse/model_not_found stop immediately.
//! - [`RetryPolicy`] — max attempts, base delay, max delay, jitter.
//! - [`CallLog`] — shape of one row written to `llm_call_logs`.

use crate::platform::keyring;
use crate::llm_clients::{CallError, CallRequest, CostRate, LlmClient, err};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct KeyHandle {
    pub id: String,           // DB row id
    pub alias: String,        // human alias (e.g. "prod-1")
    pub keyring_alias: String,// keyring entry name
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,    // total attempts across keys (1 = no retry)
    pub base_delay: Duration, // 1st backoff
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 2,
            base_delay: Duration::from_millis(800),
            max_delay: Duration::from_secs(8),
        }
    }
}

impl RetryPolicy {
    pub fn from_provider_row(max_retries: i64) -> Self {
        Self {
            max_attempts: (max_retries as u32).max(1),
            ..Self::default()
        }
    }

    /// Exponential backoff with full jitter (AWS pattern).
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let exp = 2u64.saturating_pow(attempt.saturating_sub(1));
        let raw = self.base_delay.saturating_mul(exp as u32);
        let raw = raw.min(self.max_delay);
        let jitter_min = self.base_delay.as_nanos() as u64;
        let jitter_max = raw.as_nanos() as u64;
        if jitter_max <= jitter_min {
            return Duration::from_nanos(jitter_min);
        }
        let nanos = jitter_min + (rand::random::<u64>() % (jitter_max - jitter_min));
        Duration::from_nanos(nanos)
    }
}

#[derive(Debug, Clone)]
pub struct CallLog {
    pub provider_id: String,
    pub key_id: Option<String>,
    pub analysis_id: Option<String>,
    pub latency_ms: u64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_cents: f64,
    pub http_status: Option<u16>,
    pub success: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub prompt_version: Option<String>,
    pub predicted_prob: Option<f64>,
    pub recommended_side: Option<String>,
    pub caller: String,
    pub retry_count: i64,
}

impl CallLog {
    fn empty_log(provider_id: &str, analysis_id: Option<&str>, prompt_version: Option<&str>, caller: &str) -> Self {
        Self {
            provider_id: provider_id.into(),
            key_id: None,
            analysis_id: analysis_id.map(str::to_string),
            latency_ms: 0,
            tokens_in: 0,
            tokens_out: 0,
            cost_cents: 0.0,
            http_status: None,
            success: false,
            error_code: None,
            error_message: None,
            prompt_version: prompt_version.map(str::to_string),
            predicted_prob: None,
            recommended_side: None,
            caller: caller.into(),
            retry_count: 0,
        }
    }
}

pub struct DispatchOutcome {
    pub outcome: Result<crate::llm_clients::CallOutcome, CallError>,
    pub key_used: Option<KeyHandle>,
    pub attempts: u32,
    pub log: CallLog,
}

/// Try a call against the given key list, rotating on retryable errors.
pub async fn dispatch(
    client: &dyn LlmClient,
    http: &reqwest::Client,
    keys: &[KeyHandle],
    req: &CallRequest,
    cost: CostRate,
    policy: RetryPolicy,
    provider_id: &str,
    analysis_id: Option<&str>,
    prompt_version: Option<&str>,
    caller: &str,
) -> DispatchOutcome {
    if keys.is_empty() {
        return DispatchOutcome {
            outcome: Err(CallError {
                http_status: None,
                code: err::AUTH,
                message: format!("no enabled keys for provider {provider_id}"),
            }),
            key_used: None,
            attempts: 0,
            log: CallLog {
                error_code: Some(err::AUTH.into()),
                error_message: Some(format!("no keys configured for provider {provider_id}")),
                ..CallLog::empty_log(provider_id, analysis_id, prompt_version, caller)
            },
        };
    }

    let mut last_err: Option<CallError> = None;
    let mut attempts: u32 = 0;
    let max = policy.max_attempts.min(keys.len() as u32);

    for key in keys.iter().take(max as usize) {
        attempts += 1;
        let secret = match keyring::get_key(&key.keyring_alias) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    "keyring get_key failed provider={} key={} err={}",
                    provider_id, key.alias, e
                );
                last_err = Some(CallError {
                    http_status: None,
                    code: err::AUTH,
                    message: format!("keyring get_key({}): {e}", key.keyring_alias),
                });
                continue;
            }
        };

        match client.call(http, &secret, req, cost).await {
            Ok(mut outcome) => {
                let parse_ok = outcome.parse_ok;
                let parse_error = outcome.parse_error.clone();
                let mut log = CallLog {
                    latency_ms: outcome.latency_ms,
                    tokens_in: outcome.tokens_in,
                    tokens_out: outcome.tokens_out,
                    cost_cents: outcome.cost_cents,
                    http_status: Some(outcome.http_status),
                    success: true,
                    key_id: Some(key.id.clone()),
                    ..CallLog::empty_log(provider_id, analysis_id, prompt_version, caller)
                };
                log.retry_count = (attempts as i64) - 1;
                if !parse_ok {
                    log.success = false;
                    log.error_code = Some(err::PARSE.into());
                    log.error_message = parse_error;
                }
                return DispatchOutcome {
                    outcome: Ok(outcome),
                    key_used: Some(KeyHandle {
                        id: key.id.clone(),
                        alias: key.alias.clone(),
                        keyring_alias: key.keyring_alias.clone(),
                    }),
                    attempts,
                    log,
                };
            }
            Err(e) => {
                tracing::warn!(
                    "LLM call failed provider={} key={} attempt={} code={} status={:?} msg={}",
                    provider_id, key.alias, attempts, e.code, e.http_status, e.message
                );
                let retryable = matches!(e.code, err::RATE_LIMIT | err::TIMEOUT | err::NETWORK);
                if !retryable {
                    let mut log = CallLog {
                        http_status: e.http_status,
                        success: false,
                        error_code: Some(e.code.to_string()),
                        error_message: Some(e.message.clone()),
                        key_id: Some(key.id.clone()),
                        ..CallLog::empty_log(provider_id, analysis_id, prompt_version, caller)
                    };
                    log.retry_count = (attempts as i64) - 1;
                    return DispatchOutcome {
                        outcome: Err(e),
                        key_used: Some(KeyHandle {
                            id: key.id.clone(),
                            alias: key.alias.clone(),
                            keyring_alias: key.keyring_alias.clone(),
                        }),
                        attempts,
                        log,
                    };
                }
                last_err = Some(e);
                if attempts < max {
                    let d = policy.delay_for(attempts);
                    tracing::debug!("LLM backoff {}ms before next key", d.as_millis());
                    tokio::time::sleep(d).await;
                }
            }
        }
    }

    // All attempts failed.
    let final_err = last_err.unwrap_or_else(|| CallError {
        http_status: None,
        code: err::UNKNOWN,
        message: "all attempts failed (no error captured)".into(),
    });
    let mut log = CallLog {
        http_status: final_err.http_status,
        success: false,
        error_code: Some(final_err.code.to_string()),
        error_message: Some(final_err.message.clone()),
        key_id: keys.first().map(|k| k.id.clone()),
        ..CallLog::empty_log(provider_id, analysis_id, prompt_version, caller)
    };
    log.retry_count = (attempts as i64) - 1;
    DispatchOutcome {
        outcome: Err(final_err),
        key_used: keys.first().cloned().map(|k| KeyHandle {
            id: k.id.clone(),
            alias: k.alias.clone(),
            keyring_alias: k.keyring_alias.clone(),
        }),
        attempts,
        log,
    }
}
