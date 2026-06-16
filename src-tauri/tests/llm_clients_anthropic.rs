//! Integration tests for the Anthropic Messages API client.

use mockito::Server;
use polyrocket_lib::domain::llm::{
    self, AnthropicClient, CallRequest, CostRate, LlmClient, ProviderKind,
};

fn fake_anthropic_response() -> serde_json::Value {
    serde_json::json!({
        "id": "msg_test_abc123",
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "text",
            "text": "{\"probability\":0.62,\"side\":\"NO\",\"confidence\":0.71,\"reasoning\":\"liquidity concerns\"}"
        }],
        "model": "claude-sonnet-4-5",
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 89,
            "output_tokens": 47
        }
    })
}

#[tokio::test]
async fn anthropic_parses_tokens_and_text() {
    let mut server = Server::new_async().await;
    let _m = server.mock("POST", "/v1/messages")
        .with_status(200)
        .with_header("content-type", "application/json")
        .match_header("x-api-key", "sk-ant-test")
        .match_header("anthropic-version", "2023-06-01")
        .with_body(fake_anthropic_response().to_string())
        .create_async()
        .await;

    let client = AnthropicClient::with_base(server.url());
    let http = polyrocket_lib::domain::llm::new_http_client();
    let req = CallRequest::new("claude-sonnet-4-5")
        .system("You are a precise prediction-market analyst.")
        .user("Will BTC > $100k?")
        .max_tokens(256);
    let cost = CostRate { per_1k_in_cents: 0.300, per_1k_out_cents: 1.500 };
    let out = client.call(&http, "sk-ant-test", &req, cost).await.expect("call must succeed");
    assert_eq!(out.tokens_in, 89);
    assert_eq!(out.tokens_out, 47);
    // expected cost: 89/1000 * 0.300 + 47/1000 * 1.500 = 0.0267 + 0.0705 = 0.0972
    assert!((out.cost_cents - 0.0972).abs() < 1e-6, "got {}", out.cost_cents);
    assert!(out.text.contains("liquidity concerns"));
    assert_eq!(client.kind(), ProviderKind::Anthropic);
}

#[tokio::test]
async fn anthropic_401_returns_auth_error() {
    let mut server = Server::new_async().await;
    let _m = server.mock("POST", "/v1/messages")
        .with_status(401)
        .with_body(r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#)
        .create_async()
        .await;

    let client = AnthropicClient::with_base(server.url());
    let http = polyrocket_lib::domain::llm::new_http_client();
    let req = CallRequest::new("claude-sonnet-4-5").user("hi");
    let err = client.call(&http, "sk-ant-bad", &req, CostRate::default()).await
        .expect_err("must error on 401");
    assert_eq!(err.code, "auth");
    assert_eq!(err.http_status, Some(401));
}

#[tokio::test]
async fn anthropic_429_returns_rate_limit() {
    let mut server = Server::new_async().await;
    let _m = server.mock("POST", "/v1/messages")
        .with_status(429)
        .with_body(r#"{"type":"error","error":{"type":"rate_limit_error","message":"too many requests"}}"#)
        .create_async()
        .await;

    let client = AnthropicClient::with_base(server.url());
    let http = polyrocket_lib::domain::llm::new_http_client();
    let req = CallRequest::new("claude-sonnet-4-5").user("hi");
    let err = client.call(&http, "sk-ant-x", &req, CostRate::default()).await
        .expect_err("must error on 429");
    assert_eq!(err.code, "rate_limit");
}

#[tokio::test]
async fn anthropic_529_overloaded_returns_network() {
    // 529 is Anthropic-specific "overloaded" — classify as network so it
    // triggers retry (and the dispatch loop will rotate keys).
    let mut server = Server::new_async().await;
    let _m = server.mock("POST", "/v1/messages")
        .with_status(529)
        .with_body(r#"{"type":"error","error":{"type":"overloaded_error","message":"server overloaded"}}"#)
        .create_async()
        .await;

    let client = AnthropicClient::with_base(server.url());
    let http = polyrocket_lib::domain::llm::new_http_client();
    let req = CallRequest::new("claude-sonnet-4-5").user("hi");
    let err = client.call(&http, "sk-ant-x", &req, CostRate::default()).await
        .expect_err("must error on 529");
    // 5xx maps to network; same retry semantics
    assert_eq!(err.code, "network", "529 should be network-retryable");
    assert_eq!(err.http_status, Some(529));
}
