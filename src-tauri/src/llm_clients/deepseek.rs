//! DeepSeek — OpenAI-compatible wire (https://api.deepseek.com/v1).

use crate::llm_clients::{CallError, CallRequest, CostRate, LlmClient, ProviderKind};
use crate::llm_clients::common;
use crate::llm_clients::openai::OpenAIClient;

pub struct DeepSeekClient {
    inner: OpenAIClient,
}

impl DeepSeekClient {
    pub fn new() -> Self {
        Self { inner: OpenAIClient::new("https://api.deepseek.com/v1") }
    }
}

impl Default for DeepSeekClient {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl LlmClient for DeepSeekClient {
    fn kind(&self) -> ProviderKind { ProviderKind::Deepseek }

    async fn call(
        &self,
        http: &reqwest::Client,
        secret: &str,
        req: &CallRequest,
        cost: CostRate,
    ) -> Result<crate::llm_clients::CallOutcome, CallError> {
        // DeepSeek reasons before answering (R1) — bump max_tokens default
        // if caller didn't set it, to avoid truncation on chain-of-thought.
        let mut req = req.clone();
        if req.max_tokens < 2048 {
            req.max_tokens = 2048;
        }
        self.inner.call(http, secret, &req, cost).await
    }
}
