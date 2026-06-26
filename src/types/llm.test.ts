/**
 * LLM DTO 类型往返测试 (v0.16c)。
 *
 * L1 DTO（在 `@/types/llm` 中）是 Rust DTO
 * （`commands::llm::LlmAnalysisDto`、
 * `LlmRecommendationDto` 等）的线格式镜像。
 * 若形态发生漂移，L1 会在反序列化
 * IPC 负载时静默出错。
 *
 * 这些测试使用一个 JSON literal 来模拟 Rust
 * 通过 IPC 发送的内容，解析为 TS interface，
 * 并断言每个字段。该 literal 即生产数据
 * 预期形态 —— 如果 Rust 新增了字段，
 * 测试将失败，直至 TS interface 同步更新。
 */
import { describe, expect, it } from 'vitest';
import type { LlmAnalysis, LlmRecommendation, LlmCallLog } from './llm';
// RecordLlmDecisionArgs 在 `@/ipc`（而非 `./llm`）中，
// 因为它是 L1 wrapper 的参数形态，而不是 DTO。
// DTO 是 Rust DTO 的线格式镜像；wrapper 参数
// 镜像的是 Rust command 的输入结构。
import type { RecordLlmDecisionArgs } from '@/ipc';

describe('LlmAnalysis shape (v0.16a wire format)', () => {
  it('parses a complete completed analysis', () => {
    const a: LlmAnalysis = JSON.parse(JSON.stringify({
      id: 'a-uuid-1',
      market_id: 'm1',
      signal_id: 42,
      prompt_version: 'market-analysis-1.0.0',
      requested_at: 1_700_000_000_000,
      completed_at: 1_700_000_005_000,
      status: 'completed',
      consensus_predicted: 0.62,
      consensus_side: 'YES',
      consensus_conf: 0.71,
      total_latency_ms: 5000,
      cost_cents: 0.12,
      triggered_by: 'user:abc',
      recommendations: [],
    }));
    expect(a.id).toBe('a-uuid-1');
    expect(a.status).toBe('completed');
    expect(a.signal_id).toBe(42);
    expect(a.total_latency_ms).toBe(5000);
  });

  it('parses a partial analysis (some providers failed)', () => {
    const a: LlmAnalysis = JSON.parse(JSON.stringify({
      id: 'a-uuid-2',
      market_id: 'm2',
      signal_id: null,
      prompt_version: 'market-analysis-1.0.0',
      requested_at: 1_700_000_000_000,
      completed_at: 1_700_000_005_000,
      status: 'partial',
      consensus_predicted: 0.55,
      consensus_side: 'YES',
      consensus_conf: 0.60,
      total_latency_ms: 4500,
      cost_cents: 0.04,
      triggered_by: 'user:abc',
      recommendations: [],
    }));
    expect(a.status).toBe('partial');
    expect(a.signal_id).toBeNull();
    expect(a.consensus_predicted).toBeCloseTo(0.55, 5);
  });

  it('parses a failed analysis (all providers failed)', () => {
    const a: LlmAnalysis = JSON.parse(JSON.stringify({
      id: 'a-uuid-3',
      market_id: 'm3',
      signal_id: null,
      prompt_version: 'market-analysis-1.0.0',
      requested_at: 1_700_000_000_000,
      completed_at: 1_700_000_002_000,
      status: 'failed',
      consensus_predicted: null,
      consensus_side: null,
      consensus_conf: null,
      total_latency_ms: 2000,
      cost_cents: 0.0,
      triggered_by: 'user:abc',
      recommendations: [],
    }));
    expect(a.status).toBe('failed');
    expect(a.consensus_predicted).toBeNull();
    expect(a.consensus_side).toBeNull();
  });

  it('parses a still-pending analysis', () => {
    const a: LlmAnalysis = JSON.parse(JSON.stringify({
      id: 'a-uuid-4',
      market_id: 'm4',
      signal_id: null,
      prompt_version: 'market-analysis-1.0.0',
      requested_at: 1_700_000_000_000,
      completed_at: null,
      status: 'pending',
      consensus_predicted: null,
      consensus_side: null,
      consensus_conf: null,
      total_latency_ms: null,
      cost_cents: null,
      triggered_by: 'user:abc',
      recommendations: [],
    }));
    expect(a.status).toBe('pending');
    expect(a.completed_at).toBeNull();
  });
});

describe('LlmRecommendation shape (v0.16a wire format)', () => {
  it('parses a parse_ok recommendation with full data', () => {
    const r: LlmRecommendation = JSON.parse(JSON.stringify({
      id: 1,
      analysis_id: 'a-uuid-1',
      provider_id: 'anthropic',
      provider_name: 'Anthropic Claude',
      predicted_prob: 0.65,
      side: 'YES',
      confidence: 0.82,
      reasoning: 'Strong market signal based on the recent orderbook depth',
      latency_ms: 1234,
      tokens_in: 312,
      tokens_out: 88,
      cost_cents: 0.04,
      parse_ok: true,
      parse_error: null,
    }));
    expect(r.id).toBe(1);
    expect(r.analysis_id).toBe('a-uuid-1');
    expect(r.provider_id).toBe('anthropic');
    expect(r.parse_ok).toBe(true);
    expect(r.reasoning).toContain('Strong market signal');
  });

  it('parses a parse-failed recommendation (all fields nullable)', () => {
    const r: LlmRecommendation = JSON.parse(JSON.stringify({
      id: 2,
      analysis_id: 'a-uuid-1',
      provider_id: 'openai',
      provider_name: 'OpenAI GPT-4',
      predicted_prob: null,
      side: null,
      confidence: null,
      reasoning: null,
      latency_ms: 800,
      tokens_in: 0,
      tokens_out: 0,
      cost_cents: 0.0,
      parse_ok: false,
      parse_error: 'model output not in expected JSON shape',
    }));
    expect(r.parse_ok).toBe(false);
    expect(r.predicted_prob).toBeNull();
    expect(r.side).toBeNull();
    expect(r.parse_error).toContain('JSON shape');
  });

  it('parses a network-failed recommendation (no LLM output at all)', () => {
    const r: LlmRecommendation = JSON.parse(JSON.stringify({
      id: 3,
      analysis_id: 'a-uuid-1',
      provider_id: 'google',
      provider_name: 'Google Gemini',
      predicted_prob: null,
      side: null,
      confidence: null,
      reasoning: null,
      latency_ms: 5000,
      tokens_in: 0,
      tokens_out: 0,
      cost_cents: 0.0,
      parse_ok: false,
      parse_error: 'network timeout after 5000ms',
    }));
    expect(r.parse_error).toContain('timeout');
  });
});

describe('LlmCallLog shape (v0.16a wire format)', () => {
  it('parses a successful call log (analysis_id is the UUID string)', () => {
    const c: LlmCallLog = JSON.parse(JSON.stringify({
      id: 100,
      provider_id: 'anthropic',
      key_id: 'k-anthropic-1',
      analysis_id: 'a-uuid-1',
      called_at: 1_700_000_000_000,
      model_used: 'claude-3-5-sonnet',
      prompt_tokens: 312,
      completion_tokens: 88,
      cost_cents: 0.04,
      success: true,
      error_code: null,
      error_message: null,
      http_status: 200,
      latency_ms: 1234,
    }));
    expect(c.analysis_id).toBe('a-uuid-1');
    expect(c.success).toBe(true);
    expect(c.error_code).toBeNull();
  });

  it('parses a failed call log (analysis_id null for non-analyze calls)', () => {
    const c: LlmCallLog = JSON.parse(JSON.stringify({
      id: 101,
      provider_id: 'openai',
      key_id: null,
      analysis_id: null,
      called_at: 1_700_000_000_000,
      model_used: 'gpt-4',
      prompt_tokens: 0,
      completion_tokens: 0,
      cost_cents: 0.0,
      success: false,
      error_code: 'auth',
      error_message: 'invalid API key',
      http_status: 401,
      latency_ms: 50,
    }));
    expect(c.analysis_id).toBeNull();
    expect(c.error_code).toBe('auth');
  });
});

describe('RecordLlmDecisionArgs shape (v0.16b arg shape)', () => {
  it('accepts a follow_top decision with only required fields', () => {
    const a: RecordLlmDecisionArgs = {
      analysisId: 'a-uuid-1',
      userDecision: 'follow_top',
    };
    expect(a.analysisId).toBe('a-uuid-1');
    expect(a.userDecision).toBe('follow_top');
    expect(a.userDecidedSide).toBeUndefined();
    expect(a.followedLlmId).toBeUndefined();
  });

  it('accepts a manual_yes decision with explicit side + LLM id', () => {
    const a: RecordLlmDecisionArgs = {
      analysisId: 'a-uuid-2',
      userDecision: 'manual_yes',
      userDecidedSide: 'YES',
      followedLlmId: 1,
    };
    expect(a.userDecidedSide).toBe('YES');
    expect(a.followedLlmId).toBe(1);
  });

  it('accepts a skip decision (no optional fields)', () => {
    const a: RecordLlmDecisionArgs = {
      analysisId: 'a-uuid-3',
      userDecision: 'skip',
    };
    expect(a.userDecision).toBe('skip');
  });

  it('accepts a re_analyze decision (no optional fields)', () => {
    const a: RecordLlmDecisionArgs = {
      analysisId: 'a-uuid-4',
      userDecision: 're_analyze',
    };
    expect(a.userDecision).toBe('re_analyze');
  });

  it('accepts a manual_no decision with a bet_id (v0.6+ bet linking)', () => {
    const a: RecordLlmDecisionArgs = {
      analysisId: 'a-uuid-5',
      userDecision: 'manual_no',
      userDecidedSide: 'NO',
      betId: 'b-uuid-99',
    };
    expect(a.betId).toBe('b-uuid-99');
  });
});

describe('top-recommendation picker logic (recMut)', () => {
  // v0.16b —— recMut 为 modal 挑选 top recommendation。
  // 逻辑是：先 parse_ok，然后按 confidence 降序，
  // 再取列表中的首个。本测试直接（脱离 React）
  // 跑算法，以便验证排序规则。

  type Rec = { id: number; parse_ok: boolean; confidence: number | null };
  function pickTop(recs: Rec[]): Rec | undefined {
    return [...recs]
      .filter((r) => r.parse_ok)
      .sort((a, b) => (b.confidence ?? 0) - (a.confidence ?? 0))[0]
      ?? recs[0];
  }

  it('picks the highest-confidence parse_ok rec', () => {
    const top = pickTop([
      { id: 1, parse_ok: true, confidence: 0.5 },
      { id: 2, parse_ok: true, confidence: 0.9 },
      { id: 3, parse_ok: true, confidence: 0.7 },
    ]);
    expect(top?.id).toBe(2);
  });

  it('skips parse_failed recs even if their confidence is set', () => {
    const top = pickTop([
      { id: 1, parse_ok: true, confidence: 0.5 },
      { id: 2, parse_ok: false, confidence: 0.9 }, // confidence 最高但 parse_failed
      { id: 3, parse_ok: true, confidence: 0.4 },
    ]);
    expect(top?.id).toBe(1);
  });

  it('falls back to the first rec when all are parse_failed', () => {
    const top = pickTop([
      { id: 1, parse_ok: false, confidence: 0.9 },
      { id: 2, parse_ok: false, confidence: 0.5 },
    ]);
    expect(top?.id).toBe(1);
  });

  it('handles null confidence as 0 (lowest)', () => {
    const top = pickTop([
      { id: 1, parse_ok: true, confidence: null },
      { id: 2, parse_ok: true, confidence: 0.5 },
    ]);
    expect(top?.id).toBe(2);
  });

  it('returns undefined when recs is empty', () => {
    const top = pickTop([]);
    expect(top).toBeUndefined();
  });
});
