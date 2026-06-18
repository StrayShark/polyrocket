//! DeepSeek — OpenAI-compatible wire (https://api.deepseek.com/v1).

use crate::domain::llm::{CallError, CallRequest, CostRate, LlmClient, ProviderKind};
use crate::domain::llm::common;
use crate::domain::llm::openai::OpenAIClient;

/// DeepSeek API 客户端。**底层复用 `OpenAIClient`**（DeepSeek 用 OpenAI 兼容协议）。
///
/// **保留独立 struct 的原因**：
///   - CostRate 单独定价（DeepSeek 价格跟 OpenAI 不同）
///   - ProviderKind 区分（`dispatch` 按 kind 选 client）
///   - 未来 DeepSeek 走自有协议时（已有传闻）不破坏接口
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
    ) -> Result<crate::domain::llm::CallOutcome, CallError> {
        // DeepSeek reasons before answering (R1) — bump max_tokens default
        // if caller didn't set it, to avoid truncation on chain-of-thought.
        let mut req = req.clone();
        if req.max_tokens < 2048 {
            req.max_tokens = 2048;
        }
        self.inner.call(http, secret, &req, cost).await
    }
}
