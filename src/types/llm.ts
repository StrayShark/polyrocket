// LLM DTO（v0.16a —— 修正以匹配 Rust 真值来源）。
//
// 这些类型是以下内容的线格式镜像：
//   src-tauri/src/commands/llm.rs::LlmAnalysisDto
//   src-tauri/src/commands/llm.rs::LlmRecommendationDto
//   src-tauri/src/commands/llm.rs::LlmCallLog（相关字段）
//
// schema（src/db/schema/index.ts）在 `llm_analyses`
// 上使用 `text('id')` 作为主键（由 Rust 用
// `Uuid::new_v4().to_string()` 生成 UUID），但在
// `llm_recommendations.id` 和 `llm_call_logs.id` 上使用
// `integer` 自增。v0.15c 不得不通过 `as unknown as number`
// 来强制转换 analysis id，以绕过这种类型不匹配；
// v0.16a 修复了该类型，使强制转换得以消除。
//
// 命名：snake_case 字段与 Rust DTO 一致。
// Tauri 2 IPC 直接接受 snake_case 参数；
// 若要让 camelCase JSON 映射到 snake_case 的 Rust，
// 则使用 `args` 结构体（Tauri 会自动转换）。

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
  /** 表示 OS keyring 中是否存在该 key 的条目。 */
  has_secret: boolean;
}

export interface LlmCallLog {
  /** `llm_call_logs` 上的自增主键。 */
  id: number;
  provider_id: string;
  key_id: string | null;
  /** 与 `llm_analyses.id` 匹配的 UUID 字符串。
   * 对于非 analyze 调用（例如 `llm_test_connectivity`）为 null。 */
  analysis_id: string | null;
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

/**
 * Rust 端 `LlmAnalysisDto` 的线格式镜像。
 *
 * v0.16a —— `id` 是 UUID 字符串，而非数字。
 * schema 定义 `id: text('id').primaryKey()`，
 * 并由 Rust 用 `Uuid::new_v4().to_string()` 生成。
 * 之前的 `id: number` 是一个长期存在的 bug，
 * 被调用点的 `as unknown as number` 强制转换掩盖。
 *
 * 字段名与 Rust DTO 完全一致（snake_case）。
 */
export interface LlmAnalysis {
  id: string;
  market_id: string;
  signal_id: number | null;
  prompt_version: string;
  requested_at: number;
  completed_at: number | null;
  status: 'pending' | 'completed' | 'partial' | 'failed';
  consensus_predicted: number | null;
  consensus_side: string | null;
  consensus_conf: number | null;
  total_latency_ms: number | null;
  cost_cents: number | null;
  triggered_by: string;
  recommendations: LlmRecommendation[];
}

/**
 * Rust 端 `LlmRecommendationDto` 的线格式镜像。
 * `id` 是自增 integer（每行）；`analysis_id`
 * 是父级 analysis 的 UUID 字符串。
 *
 * v0.16a —— 从 `analysis_id: number` 修正为
 * `analysis_id: string`。新增 `provider_name`（查询中
 * JOIN `llm_providers` 后得到的 display_name）。
 */
export interface LlmRecommendation {
  id: number;
  analysis_id: string;
  provider_id: string;
  provider_name: string;
  predicted_prob: number | null;
  side: string | null;
  confidence: number | null;
  reasoning: string | null;
  latency_ms: number | null;
  tokens_in: number | null;
  tokens_out: number | null;
  cost_cents: number | null;
  parse_ok: boolean;
  parse_error: string | null;
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
