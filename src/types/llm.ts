// LLM DTOs (mirror src-tauri/src/commands/llm.rs + llm_mgmt.rs)

export type ProviderKind =
  | 'openai'
  | 'anthropic'
  | 'google'
  | 'deepseek'
  | 'openai_compat'
  | 'anthropic_compat';

export interface LlmProvider {
  id: string;
  display_name: string;
  kind: ProviderKind;
  api_base: string | null;
  default_model: string;
  enabled: boolean;
  cost_per_1k_in: number | null;
  cost_per_1k_out: number | null;
  timeout_ms: number;
  max_retries: number;
  health_status: string | null;
  health_latency_p50_ms: number | null;
  last_health_check_at: number | null;
  last_health_error: string | null;
}

export interface LlmProviderKey {
  id: string;
  provider_id: string;
  alias: string;
  keyring_alias: string;
  priority: number;
  enabled: boolean;
  created_at: number;
  /** True if the OS keyring entry exists for this key. */
  has_secret: boolean;
}

export interface LlmCallLog {
  id: number;
  provider_id: string;
  key_id: string | null;
  analysis_id: number | null;
  called_at: number;
  model_used: string;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  cost_cents: number;
  success: boolean;
  error_code: string | null;
  error_message: string | null;
  http_status: number | null;
  latency_ms: number | null;
}

export type ErrorCode =
  | 'auth'
  | 'rate_limit'
  | 'timeout'
  | 'network'
  | 'parse'
  | 'model_not_found'
  | 'quota'
  | 'unknown';

export interface LlmAnalysis {
  id: number;
  market_id: string;
  requested_at: number;
  finished_at: number | null;
  prompt_version: string;
  n_providers_requested: number;
  n_providers_succeeded: number;
  consensus_prob: number | null;
  consensus_side: string | null;
  consensus_confidence: number | null;
  status: 'pending' | 'ok' | 'error';
}

export interface LlmRecommendation {
  id: number;
  analysis_id: number;
  provider_id: string;
  side: string;
  predicted_prob: number;
  confidence: number;
  rationale: string | null;
  cost_cents: number;
  latency_ms: number;
}

export interface LlmPerformance {
  model_version: string;
  n_predictions: number;
  win_rate: number;
  brier_score: number;
  log_loss: number;
  avg_edge: number;
}

export interface LlmHeatmapCell {
  model_version: string;
  prompt_version: string;
  n: number;
  win_rate: number;
  brier: number;
}

export interface LlmScatterPoint {
  analysis_id: number;
  market_id: string;
  predicted_prob: number;
  market_prob: number;
  outcome: number | null;
  resolved_at: number | null;
}

export interface LlmTimeseriesPoint {
  bucket_ts: number;
  n_calls: number;
  n_success: number;
  cost_cents: number;
  avg_latency_ms: number;
}

export interface ConnectivityTestResult {
  provider_id: string;
  key_id: string;
  ok: boolean;
  http_status: number | null;
  latency_ms: number;
  error_code: ErrorCode | null;
  error_message: string | null;
}

export interface LlmTrafficSummary {
  window_start: number;
  window_end: number;
  total_calls: number;
  success_calls: number;
  total_cost_cents: number;
  per_provider: Array<{
    provider_id: string;
    calls: number;
    cost_cents: number;
    avg_latency_ms: number;
  }>;
}

export interface UpsertLlmProviderArgs {
  id: string;
  display_name: string;
  kind: ProviderKind;
  api_base?: string;
  default_model: string;
  enabled?: boolean;
  cost_per_1k_in?: number;
  cost_per_1k_out?: number;
  timeout_ms?: number;
  max_retries?: number;
}

export interface UpsertLlmKeyArgs {
  provider_id: string;
  id?: string;
  alias: string;
  keyring_alias: string;
  priority?: number;
  enabled?: boolean;
  secret?: string;
}
