/**
 * L1 ↔ L2 IPC client. Typed wrappers over `invoke('cmd_name', { args })`.
 *
 * Every function corresponds 1:1 to a `#[tauri::command]` in
 * `src-tauri/src/commands/*.rs`. If you add a Rust command, add a
 * wrapper here in the same commit (CI check enforces module map).
 *
 * Layer rules: this module is the ONLY place that calls `invoke`.
 * Routes / components import from here, never `@tauri-apps/api/core`.
 * See docs/overview.md §1.2 (L1 → L2, single direction).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Wallet,
  AddWalletArgs,
} from '@/types/wallet';
import type { Market, ListMarketsArgs } from '@/types/market';
import type { Signal, ListSignalsArgs } from '@/types/signal';
import type { Bet, PlaceJumpArgs, PlaceSignedArgs, ListBetsArgs } from '@/types/bet';
import type {
  LlmProvider,
  LlmProviderKey,
  ConnectivityTestResult,
  LlmTrafficSummary,
  LlmAnalysis,
  LlmRecommendation,
  LlmPerformance,
  LlmHeatmapCell,
  LlmScatterPoint,
  LlmTimeseriesPoint,
  LlmCallLog,
  UpsertLlmProviderArgs,
  UpsertLlmKeyArgs,
} from '@/types/llm';
import type {
  CopyTarget,
  CopyEvent,
  AddCopyTargetArgs,
  DashboardKpis,
  DailyBriefEntry,
  BriefRefreshResult,
  SetBriefPrefsArgs,
  SchedulerStatus,
  SchedulerTriggerResult,
  SecretsStatus,
  AuditEntry,
} from '@/types/shared';

// ---------------------------------------------------------------- Wallet (M4)
export const listWallets = () => invoke<Wallet[]>('list_wallets');
export const addWallet = (args: AddWalletArgs) => invoke<Wallet>('add_wallet', { args });

// ---------------------------------------------------------------- Market (M1)
export const listMarkets = (args: ListMarketsArgs = {}) =>
  invoke<Market[]>('list_markets', { args });
export const syncMarkets = () => invoke<number>('sync_markets');

// ---------------------------------------------------------------- Signal (M2)
export const listActiveSignals = (args: ListSignalsArgs = {}) =>
  invoke<Signal[]>('list_active_signals', { args });
export const recomputeSignals = () => invoke<number>('recompute_signals');

// ---------------------------------------------------------------- Bet (M3)
export const placeJumpLink = (args: PlaceJumpArgs) =>
  invoke<string>('place_jump_link', { args });
export const placeSignedOrder = (args: PlaceSignedArgs) =>
  invoke<Bet>('place_signed_order', { args });
export const listBets = (args: ListBetsArgs = {}) =>
  invoke<Bet[]>('list_bets', { args });

// ---------------------------------------------------------------- Copy (M5)
export const listCopyTargets = () => invoke<CopyTarget[]>('list_copy_targets');
export const addCopyTarget = (args: AddCopyTargetArgs) =>
  invoke<CopyTarget>('add_copy_target', { args });
export const recentCopyEvents = (targetId?: string, limit = 50) =>
  invoke<CopyEvent[]>('recent_copy_events', { targetId, limit });

// ---------------------------------------------------------------- PnL (M6)
export const dashboardKpis = () => invoke<DashboardKpis>('dashboard_kpis');

// ---------------------------------------------------------------- LLM Analysis (M10)
export const listLlmProviders = () => invoke<LlmProvider[]>('list_llm_providers');
export const upsertLlmProvider = (args: UpsertLlmProviderArgs) =>
  invoke<LlmProvider>('upsert_llm_provider', { args });
export const llmAnalyze = (marketId: string, providers?: string[]) =>
  invoke<LlmAnalysis>('llm_analyze', { marketId, providers });
export const llmPerformance = () => invoke<LlmPerformance[]>('llm_performance');

/**
 * v0.16b — `record_llm_decision` arg shape.
 *
 * The Rust side takes a `RecordDecisionArgs` struct
 * (see `commands::llm::RecordDecisionArgs`). Tauri 2
 * auto-converts the JS object's camelCase keys to
 * the Rust struct's snake_case fields, so we send
 * `{ analysisId, userDecision, ... }` and Tauri maps
 * them to `analysis_id`, `user_decision`, etc.
 *
 * `user_decision` is one of: `'follow_top' | 'manual_yes'
 * | 'manual_no' | 'skip' | 're_analyze'`.
 */
export interface RecordLlmDecisionArgs {
  analysisId: string;
  userDecision: 'follow_top' | 'manual_yes' | 'manual_no' | 'skip' | 're_analyze';
  userDecidedSide?: 'YES' | 'NO' | null;
  followedLlmId?: number | null;
  betId?: string | null;
  contextSnapshot?: string | null;
}
export const recordLlmDecision = (args: RecordLlmDecisionArgs) =>
  invoke<number>('record_llm_decision', { args });
export const llmStatsHeatmap = () => invoke<LlmHeatmapCell[]>('llm_stats_heatmap');
export const llmStatsScatter = (marketId?: string) =>
  invoke<LlmScatterPoint[]>('llm_stats_scatter', { marketId });
export const llmStatsTimeseries = (windowHours = 24) =>
  invoke<LlmTimeseriesPoint[]>('llm_stats_timeseries', { windowHours });
export const llmStatsDecision = () =>
  invoke<Array<{ decision: string; count: number; win_rate: number }>>(
    'llm_stats_decision',
  );
export const llmGetRecommendation = (analysisId: number) =>
  invoke<LlmRecommendation>('llm_get_recommendation', { analysisId });
export const llmListAnalyses = (marketId?: string) =>
  invoke<LlmAnalysis[]>('llm_list_analyses', { marketId });

// ---------------------------------------------------------------- LLM Mgmt (M11)
export const llmProviderList = () => invoke<LlmProvider[]>('llm_provider_list');
export const llmProviderUpsert = (args: UpsertLlmProviderArgs) =>
  invoke<LlmProvider>('llm_provider_upsert', { args });
export const llmProviderDelete = (id: string) =>
  invoke<void>('llm_provider_delete', { id });
export const llmKeyList = (providerId?: string) =>
  invoke<LlmProviderKey[]>('llm_key_list', { providerId });
export const llmKeyUpsert = (args: UpsertLlmKeyArgs) =>
  invoke<LlmProviderKey>('llm_key_upsert', { args });
export const llmKeySetSecret = (keyId: string, secret: string) =>
  invoke<void>('llm_key_set_secret', { keyId, secret });
export const llmKeyDelete = (keyId: string) => invoke<void>('llm_key_delete', { keyId });
export const llmTestConnectivity = (providerId: string, keyId?: string) =>
  invoke<ConnectivityTestResult>('llm_test_connectivity', { providerId, keyId });
export const llmTrafficSummary = (windowHours = 24) =>
  invoke<LlmTrafficSummary>('llm_traffic_summary', { windowHours });
export const llmHealthHistory = (providerId: string, limit = 50) =>
  invoke<LlmCallLog[]>('llm_health_history', { providerId, limit });
export const llmStatsByConfidence = () =>
  invoke<Array<{ bucket: number; n: number; win_rate: number }>>(
    'llm_stats_by_confidence',
  );
export const llmStatsByPrompt = () =>
  invoke<Array<{ prompt_version: string; n: number; win_rate: number }>>(
    'llm_stats_by_prompt',
  );
export const llmStatsCostEfficiency = () =>
  invoke<Array<{ provider_id: string; cost_cents: number; wins: number; roi: number }>>(
    'llm_stats_cost_efficiency',
  );
export const llmStatsExport = (format: 'csv' | 'json' = 'csv') =>
  invoke<string>('llm_stats_export', { format });

// ---------------------------------------------------------------- Secrets (M11)
export const llmPmSetCredentials = (apiKey: string, secret: string, passphrase: string) =>
  invoke<void>('llm_pm_set_credentials', { apiKey, secret, passphrase });
export const llmPmClearCredentials = () => invoke<void>('llm_pm_clear_credentials');
export const polyrocketWalletSetPk = (alias: string, pk: string) =>
  invoke<void>('polyrocket_wallet_set_pk', { alias, pk });
export const polyrocketWalletClearPk = (alias: string) =>
  invoke<void>('polyrocket_wallet_clear_pk', { alias });
export const secretsStatus = () => invoke<SecretsStatus>('secrets_status');

// ---------------------------------------------------------------- Scheduler (L2)
export const schedulerStatus = () => invoke<SchedulerStatus>('scheduler_status');
export const schedulerRunHealthProbeNow = () =>
  invoke<SchedulerTriggerResult>('scheduler_run_health_probe_now');
export const schedulerRunDailyBriefNow = () =>
  invoke<SchedulerTriggerResult>('scheduler_run_daily_brief_now');

// ---------------------------------------------------------------- Notify (X2)
export const sendNotification = (
  kind: 'new_signal' | 'order_fill' | 'keyring_ok' | 'keyring_error' |
        'provider_auto_disable' | 'daily_brief' | 'mirror_decision' |
        'auto_promote' | 'info',
  title: string,
  body: string,
  prefsEnabled = true,
) => invoke<number>('send_notification', {
  args: { kind, title, body, prefs_enabled: prefsEnabled },
});
export const requestNotificationPermission = () =>
  invoke<boolean>('request_notification_permission');
export const notificationPermissionState = () =>
  invoke<string>('notification_permission_state');

// ---------------------------------------------------------------- Audit (X1)
export const listAuditLog = (limit = 200) => invoke<AuditEntry[]>('list_audit_log', { limit });
export const auditCountForActor = (actor: string) =>
  invoke<number>('audit_count_for_actor', { actor });
// v0.8c — manual trigger for the daily retention purge.
export const purgeAuditLogNow = () => invoke<number>('purge_audit_log_now');

// v0.13c — read the user-overridden retention policy. Returns the
// effective policy (user overrides merged with defaults).
export const getAuditRetention = () =>
  invoke<AuditRetentionView>('get_audit_retention');

// v0.13c — set the user's retention policy. Triggers an immediate
// purge and returns the number of rows deleted.
export const setAuditRetention = (args: SetAuditRetentionArgs) =>
  invoke<number>('set_audit_retention', { args });

export interface AuditRetentionView {
  /** Age cutoff in ms. Rows older than this are eligible for purge. */
  retain_recent_ms: number;
  /** Hard cap on total row count. */
  max_rows: number;
  /** Safety floor — never auto-purge below this many rows. */
  min_keep_rows: number;
}

export interface SetAuditRetentionArgs {
  retain_recent_ms?: number;
  max_rows?: number;
  min_keep_rows?: number;
}

// ---------------------------------------------------------------- Brief (M12)
export const dailyBriefGet = (limit = 5) =>
  invoke<DailyBriefEntry[]>('daily_brief_get', { limit });
export const dailyBriefDismiss = (marketId: string) =>
  invoke<void>('daily_brief_dismiss', { marketId });
export const dailyBriefRefresh = () => invoke<BriefRefreshResult>('daily_brief_refresh');
export const dailyBriefSetPrefs = (args: SetBriefPrefsArgs) =>
  invoke<void>('daily_brief_set_prefs', { args });

// ---------------------------------------------------------------- Seed (v0.8a)
// First-run demo data so the UI shows a populated dashboard out of the box.
// Idempotent: the second call is a no-op unless `force: true`.
export const seedDemoData = (force = false) =>
  invoke<number>('seed_demo_data', { args: { force } });
export const isSeeded = () => invoke<boolean>('is_seeded');

// ---------------------------------------------------------------- Sidecar health (v0.10d)
// Rolling "last N probes" snapshot for the topbar status badge.
export type SidecarHealthKind = 'ok' | 'failed' | 'unknown';
export interface SidecarHealthRow {
  at_ms: number;
  kind: SidecarHealthKind;
  latency_ms: number | null;
  error: string | null;
}
export interface SidecarHealthSnapshot {
  last_24h: SidecarHealthRow[];
  success_count: number;
  failure_count: number;
  last_success_at_ms: number | null;
  last_failure_at_ms: number | null;
}
export const sidecarHealthNow = () =>
  invoke<SidecarHealthSnapshot>('sidecar_health_now');
export const sidecarHealthSnapshot = () =>
  invoke<SidecarHealthSnapshot>('sidecar_health_snapshot');

// ---------------------------------------------------------------- Sidecar predict (v0.12d)
// v0.12a — predict returns the model_version in addition to
// the per-row predictions. The L1 ModelLab page shows a pill
// like "scoring with logistic-train-441c352b" so the user
// knows which model produced the current scores.
export interface Prediction {
  market_id: string;
  prob: number;
  confidence: number;
  rationale: string | null;
}
export interface PredictResult {
  predictions: Prediction[];
  model_version: string | null;
  /** v0.13b — Brier score of the most recent promoted model. */
  brier_score: number | null;
}
export const sidecarPredict = (markets: Array<[string, number]>) =>
  invoke<PredictResult>('sidecar_predict', { args: { markets } });

// v0.13d — async-friendly version. Returns the full PredictResult
// (predictions + model_version + brier_score). Falls back to an
// empty PredictResult when the sidecar is not running.
export const sidecarPredictAsync = (
  markets: Array<[string, number]>,
  timeoutMs?: number,
) =>
  invoke<PredictResult>('sidecar_predict_async', {
    args: { markets, timeout_ms: timeoutMs ?? null },
  });

// ---------------------------------------------------------------- LLM analyze progress (v0.15a)
// Event payloads for the four `llm_analyze:*` events emitted from
// `commands::llm::llm_analyze`. See `domain::llm::progress` for the
// Rust side. Each event has a stable shape; the L1 can listen to
// all four and update UI as they arrive.

export interface AnalyzeStartedEvent {
  analysis_id: string;
  market_id: string;
  prompt_version: string;
  providers: string[];
  started_at: number;
}

export interface ProviderDoneEvent {
  analysis_id: string;
  provider_id: string;
  ok: boolean;
  latency_ms: number;
  tokens_in: number | null;
  tokens_out: number | null;
  cost_cents: number;
  /** matches `domain::llm::err::*` plus the literal `"none"` and
   * `"parse"` (for successful-HTTP-but-bad-JSON responses). */
  error_kind: string;
  error_message: string | null;
  finished_at: number;
}

export interface ConsensusDoneEvent {
  analysis_id: string;
  status: 'completed' | 'partial' | 'failed';
  n_success: number;
  n_failed: number;
  consensus_pred: number | null;
  consensus_side: string | null;
  consensus_conf: number | null;
}

export interface AnalyzeFinishedEvent {
  analysis_id: string;
  status: 'completed' | 'partial' | 'failed';
  total_latency_ms: number;
  total_cost_cents: number;
  n_success: number;
  n_failed: number;
  finished_at: number;
}

/** Listen for `llm_analyze:started` events. Returns an unlisten
 * function. Use in `useEffect`'s cleanup to avoid leaks. */
export const onAnalyzeStarted = (cb: (e: AnalyzeStartedEvent) => void): Promise<UnlistenFn> =>
  listen<AnalyzeStartedEvent>('llm_analyze:started', (msg) => cb(msg.payload));

/** Listen for `llm_analyze:provider_done` events. Fires once per
 * provider, in any order (since the fan-out is parallel). */
export const onProviderDone = (cb: (e: ProviderDoneEvent) => void): Promise<UnlistenFn> =>
  listen<ProviderDoneEvent>('llm_analyze:provider_done', (msg) => cb(msg.payload));

/** Listen for `llm_analyze:consensus_done` events. Fires once per
 * analyze, after all providers finished and consensus computed. */
export const onConsensusDone = (cb: (e: ConsensusDoneEvent) => void): Promise<UnlistenFn> =>
  listen<ConsensusDoneEvent>('llm_analyze:consensus_done', (msg) => cb(msg.payload));

/** Listen for `llm_analyze:finished` events. The terminal event
 * the L1 typically `await`s to know the analyze is done. */
export const onAnalyzeFinished = (cb: (e: AnalyzeFinishedEvent) => void): Promise<UnlistenFn> =>
  listen<AnalyzeFinishedEvent>('llm_analyze:finished', (msg) => cb(msg.payload));

// ---------------------------------------------------------------- Train job (v0.17a)
// Event payloads for the `train_job:*` events emitted from
// `commands::sidecar::train_job`. See `domain::lab::train_progress`
// for the Rust side. Two events: started (before the sidecar
// call) and finished (after the sweep completes or fails).

export interface TrainStartedEvent {
  job_id: string;
  /** Total trials the Python sidecar will run (1-4). */
  n_trials: number;
  /** Per-trial training epochs (default 80). */
  epochs: number;
  started_at: number;
}

export interface TrainTrialDto {
  lr: number;
  reg: number;
  brier: number;
  /** `{"w0", "w1", "w2"}` — trained weights for this trial. */
  weights: Record<string, number>;
}

export interface TrainFinishedEvent {
  job_id: string;
  /** "completed" | "failed" */
  status: 'completed' | 'failed' | string;
  /** Lower is better. `null` on failure. */
  best_brier: number | null;
  /** Best trial's weights: `{"w0", "w1", "w2"}`. `null` on failure. */
  best_params: Record<string, number> | null;
  /** Per-trial stats. Empty on failure. */
  trials: TrainTrialDto[];
  duration_ms: number;
  /** Absolute path of the candidate JSON. `null` on failure. */
  candidate_path: string | null;
  /** Human-readable error message. `null` on success. */
  message: string | null;
  finished_at: number;
}

/** Wire-format mirror of the Rust `TrainResult` (returned by
 * the `train_job` IPC). Same fields as the finished event
 * except no `finished_at` (the IPC is the source of truth for
 * "this just finished", the event is the source of truth for
 * "any subscriber should know"). */
export interface TrainResult {
  job_id: string;
  status: 'completed' | 'failed' | string;
  best_brier: number | null;
  best_params: Record<string, number> | null;
  trials: TrainTrialDto[];
  duration_ms: number;
  candidate_path: string | null;
  message: string | null;
}

/** Args for the `train_job` IPC. All fields optional — the
 * Python sidecar uses sensible defaults (4 trials, 80 epochs,
 * 60s timeout). */
export interface TrainJobArgs {
  n_trials?: number;
  epochs?: number;
  timeout_ms?: number;
}

/** Kick off a model training sweep on the Python sidecar.
 * v0.17a — wraps `commands::sidecar::train_job`. Returns
 * the full `TrainResult` when the sweep completes. */
export const trainJob = (args: TrainJobArgs = {}) =>
  invoke<TrainResult>('train_job', { args });

/** Listen for `train_job:started` events. The L1 typically
 * uses this to render a "Training…" pill right after the
 * user clicks Train. */
export const onTrainStarted = (cb: (e: TrainStartedEvent) => void): Promise<UnlistenFn> =>
  listen<TrainStartedEvent>('train_job:started', (msg) => cb(msg.payload));

/** Listen for `train_job:finished` events. The terminal event
 * the L1 awaits to know the sweep is done. Carries the full
 * result (per-trial stats + best Brier + params). */
export const onTrainFinished = (cb: (e: TrainFinishedEvent) => void): Promise<UnlistenFn> =>
  listen<TrainFinishedEvent>('train_job:finished', (msg) => cb(msg.payload));

// ---------------------------------------------------------------- Promote model (v0.18a)
// Wire-format mirror of the Rust `PromoteResult` (returned by
// the `promote_model` IPC). v0.18a — promote is a fast file
// move (~10ms); no progress events.
//
// `promoted: true` means the candidate was successfully renamed
// to active.json. The new model version is in `model_version`.
//
// `promoted: false` means the promote was refused (no candidate,
// job_id mismatch, or sidecar not running). The `message` field
// has a human-readable diagnostic; the L1 shows it in a toast.

export interface PromoteResult {
  promoted: boolean;
  /** "ok" | "failed" */
  status: 'ok' | 'failed' | string;
  /** Path of the previous active.json. `null` on first promote. */
  previous_path: string | null;
  /** Path of the new active.json (the candidate was renamed to this). */
  active_path: string | null;
  /** Wall-clock time of the promote in ms. `null` on failure. */
  promoted_at_ms: number | null;
  /** New model version (e.g. `logistic-train-441c352b`). Empty on failure. */
  model_version: string;
  /** Human-readable error message. `null` on success. */
  message: string | null;
}

/** Args for the `promote_model` IPC. v0.18a — `job_id`
 * is optional. If set, the Python sidecar refuses to
 * promote a candidate from a different job (race-condition
 * protection).
 *
 * v0.21a — `trial_index` is also optional. If set, the
 * Python sidecar promotes that specific trial from
 * `all_trials[]` (bulk promote) instead of the best.
 * 0-indexed. If both `job_id` and `trial_index` are set,
 * both checks apply.
 */
export interface PromoteModelArgs {
  job_id?: string;
  /** v0.21a — bulk promote. 0-indexed trial number.
   * `undefined` (or omitted) means "promote the best". */
  trial_index?: number;
}

/** Promote the current candidate to the active slot. v0.18a.
 * Returns the full `PromoteResult`. */
export const promoteModel = (args: PromoteModelArgs = {}) =>
  invoke<PromoteResult>('promote_model', { args });

// =================================================================
// ================= v0.19b — list_promote_history =================
// =================================================================

/** One entry in the promotion history. v0.19b — mirrors the
 * Python sidecar's `promotion_history` array in active.json.
 *
 * Each successful promote appends one entry. The list is
 * capped at 20 most-recent entries on the Python side.
 */
export interface PromoteHistoryEntry {
  /** Train job_id, e.g. "train-441c352b". */
  job_id: string;
  /** Derived model version, e.g. "logistic-train-441c352b". */
  model_version: string;
  /** Wall-clock time of the promote in ms (Unix epoch). */
  promoted_at_ms: number;
  /** Best Brier score from the train sweep (lower is better).
   * `null` if the train payload didn't include it. */
  best_brier: number | null;
  /** Hyperparameters of the best trial, or `null` if not
   * stored (e.g. v0.18 active.json files). */
  best_params: Record<string, unknown> | null;
  /** v0.24a — which trial of the train sweep was promoted.
   * `null` = best (default); `number` = trial n (bulk).
   * Older history entries from before v0.21a won't have
   * this field; treat missing as "best" for UI purposes. */
  trial_index?: number | null;
  /** v0.41a — human-readable reason for the promote.
   * Surfaced as a hover tooltip on the history row in
   * the ModelLab page. Format: "Promoted as best trial"
   * or "Promoted as trial N of M". Older entries from
   * before v0.41 won't have this field; treat missing
   * as "Promoted" (generic). */
  reason?: string | null;
}

/** Wire-format mirror of the Rust `PromoteHistoryResult`
 * (returned by the `list_promote_history` IPC). v0.19b —
 * read-only audit query. No params. */
export interface PromoteHistoryResult {
  /** `true` if the history was successfully read. (Even
   * with no entries, the response is ok=true with a
   * helpful message.) */
  ok: boolean;
  /** History entries, oldest first. */
  entries: PromoteHistoryEntry[];
  /** `len(entries)` for convenience. */
  count: number;
  /** Human-readable message. `null` on success with entries. */
  message: string | null;
}

/** Query the promote history. v0.19b — no args.
 *
 * Returns the last 20 promotions as a list (capped on
 * the Python side). The currently active model is NOT in
 * the list — to get the active model, use `predict`'s
 * `model_version` field instead. The list is for audit
 * ("which model was active at which time") not for
 * status display.
 */
export const listPromoteHistory = () =>
  invoke<PromoteHistoryResult>('list_promote_history', { args: {} });

// =================================================================
// =============== v0.33b — list_promote_history_archive =============
// =================================================================

/** v0.33b — args for `listPromoteHistoryArchive`. All
 * fields optional. The archive is the durable long-term
 * trail of promotions that fell off the 20-entry in-memory
 * `promotion_history[]` cap. Append-only JSONL, never
 * auto-pruned. */
export interface ListPromoteHistoryArchiveArgs {
  /** Lower bound on `promoted_at_ms`. Default 0. */
  from_ms?: number;
  /** Upper bound on `promoted_at_ms`. Default MAX. */
  to_ms?: number;
  /** Pagination offset. Default 0. */
  offset?: number;
  /** Pagination limit. Default 100, capped at 1000. */
  limit?: number;
}

/** v0.33b — wire-format mirror of the Python sidecar's
 * archive.jsonl. Each entry is one archived promotion
 * (one that fell off the 20-cap). The fields mirror
 * `PromoteHistoryEntry` plus `archived_at_ms`. */
export interface PromoteHistoryArchiveEntry {
  job_id: string;
  model_version: string;
  promoted_at_ms: number;
  best_brier: number | null;
  best_params: Record<string, number> | null;
  weights: { w0: number; w1: number; w2: number } | null;
  trial_index: number | null;
  /** v0.33b — when this entry was written to the
   * archive file. May differ from `promoted_at_ms` if
   * the sidecar was offline and the entry was written
   * later. */
  archived_at_ms: number;
}

/** v0.33b — response of `listPromoteHistoryArchive`. */
export interface PromoteHistoryArchiveResult {
  /** `true` if the archive was read successfully. */
  ok: boolean;
  /** Entries matching the filter (after pagination). */
  entries: PromoteHistoryArchiveEntry[];
  /** Total entries in the file (before pagination). */
  total: number;
  /** Optional message (error or "no archive yet"). */
  message: string | null;
}

/** v0.33b — query the archived promote history.
 *
 * The Python sidecar's `archive.jsonl` file contains
 * every promotion that fell off the 20-entry in-memory
 * `promotion_history[]` cap. The L1 can use this IPC
 * to show a "View archive" panel on the ModelLab page
 * so the user can audit promotions beyond the last 20.
 *
 * The file is at `~/.polyrocket/sidecar/models/archive.jsonl`
 * (overridable via `POLYROCKET_SIDECAR_MODEL_DIR`).
 */
export const listPromoteHistoryArchive = (
  args: ListPromoteHistoryArchiveArgs = {},
) =>
  invoke<PromoteHistoryArchiveResult>(
    'list_promote_history_archive',
    { args },
  );

// =================================================================
// ==================== v0.20b — rollback_model =====================
// =================================================================

/** Args for the `rollback_model` IPC. v0.20b.
 *
 * `model_version` is the version to roll back to,
 * e.g. "logistic-train-441c352b". The L1 should pass
 * back a value it got from the history panel verbatim
 * (exact match). The version must be a history entry
 * with `weights` stored (v0.20a+); v0.19 entries
 * without weights are refused with a clear error.
 */
export interface RollbackModelArgs {
  model_version: string;
}

/** Wire-format mirror of the Rust `RollbackResult`
 * (returned by the `rollback_model` IPC). v0.20b —
 * `rolled_back: bool` is the primary success indicator.
 *
 * On success, the active.json has been rewritten with
 * the target version's weights. On failure, the active
 * model is unchanged and the `message` field has a
 * human-readable diagnostic.
 */
export interface RollbackResult {
  rolled_back: boolean;
  /** "ok" | "failed" */
  status: 'ok' | 'failed' | string;
  /** Path of the previous active.json (always the same
   * path; rollback is a write to the same file). */
  previous_path: string | null;
  /** Path of the new active.json (same as previous_path). */
  active_path: string | null;
  /** Wall-clock time of the rollback in ms. `null` on failure. */
  rolled_back_at_ms: number | null;
  /** The version that was rolled back to. Empty on failure. */
  model_version: string;
  /** Human-readable error message. `null` on success. */
  message: string | null;
}

/** Roll the active model back to a previous version.
 * v0.20b. Looks up the entry in `promotion_history` by
 * `model_version` and restores its weights as the new
 * active model.
 *
 * Failure modes (all return `rolled_back: false`):
 * - active.json is missing → "no active model"
 * - model_version not in history → "not found"
 * - entry is from v0.19 (no weights) → "no weights"
 */
export const rollbackModel = (args: RollbackModelArgs) =>
  invoke<RollbackResult>('rollback_model', { args });

// =================================================================
// ================== v0.23b — auto_promote_if_better ===============
// =================================================================

/** Args for the `auto_promote_if_better` IPC. v0.23b.
 *
 * `brier_margin` is how much better the candidate must
 * be (lower Brier = better) for the auto-promote to
 * happen. Default 0.005.
 *
 * `trial_index` is which trial to use (None = best).
 */
export interface AutoPromoteIfBetterArgs {
  /** How much better the candidate must be. Default 0.005. */
  brier_margin?: number;
  /** Which trial to use (None = best, 0..n-1 for a specific trial). */
  trial_index?: number;
}

/** Wire-format mirror of the Rust `AutoPromoteIfBetterResult`
 * (returned by the `auto_promote_if_better` IPC). v0.23b.
 *
 * `promoted` and `skipped` are mutually exclusive:
 *   - promoted=true, skipped=false: the candidate was
 *     meaningfully better, it WAS promoted
 *   - promoted=false, skipped=true: the candidate was
 *     NOT meaningfully better, no action taken
 *   - promoted=false, skipped=false: error case
 *     (e.g. no candidate, invalid margin)
 */
export interface AutoPromoteIfBetterResult {
  promoted: boolean;
  skipped: boolean;
  /** Human-readable reason. */
  reason: string;
  candidate_brier: number | null;
  active_brier: number | null;
  /** The brier_margin used for the comparison. */
  margin: number;
  model_version: string | null;
  promoted_at_ms: number | null;
  message: string | null;
}

/** Promote the candidate only if it's meaningfully better
 * than the active model. v0.23b.
 *
 * One-click action: click after each train, the sidecar
 * compares the Brier scores and either promotes the
 * candidate (if it's at least `brier_margin` better) or
 * no-ops (with a clear "skipped" reason).
 *
 * If there's no active model yet, the candidate is
 * auto-promoted (no comparison possible, it's best by
 * definition).
 */
export const autoPromoteIfBetter = (args: AutoPromoteIfBetterArgs = {}) =>
  invoke<AutoPromoteIfBetterResult>('auto_promote_if_better', { args });

// =================================================================
// =================== v0.25b — promote_all_trials ==================
// =================================================================

/** Wire-format mirror of one per-trial result inside the
 * `results` list. v0.25b. */
export interface PromoteAllTrialResult {
  /** 0-indexed trial number. */
  trial_index: number;
  /** `true` if this trial was successfully promoted. */
  promoted: boolean;
  /** "ok" | "failed" — mirrors the Python's per-call status. */
  status: 'ok' | 'failed' | string;
  /** New model version (e.g. "logistic-train-XYZ-t2"). */
  model_version: string;
  /** Wall-clock time of the promote in ms. `null` on failure. */
  promoted_at_ms: number | null;
  /** Human-readable error message. `null` on success. */
  message: string | null;
}

/** Wire-format mirror of the Rust `PromoteAllTrialsResult`
 * (returned by the `promote_all_trials` IPC). v0.25b. */
export interface PromoteAllTrialsResult {
  /** `true` if all trial promotes succeeded. */
  ok: boolean;
  /** Per-trial results, in trial_index order. */
  results: PromoteAllTrialResult[];
  /** `len(results)`. */
  count: number;
  /** Overall error message (e.g. "no candidate"). `null`
   * if all promotes succeeded. */
  message: string | null;
}

/** Bulk-promote every trial from the current candidate. v0.25b.
 *
 * For A/B comparison: the user can see how all 4 trials
 * perform on real markets, then rollback to the winner
 * via the v0.20c Rollback button. Without this, the user
 * would need to click "Promote" 4 times.
 *
 * All 4 trials appear in the history panel after this
 * call, each with its own `-tN` model version suffix
 * and `trial_index` field. The currently active model
 * is the LAST one promoted (trial 3), but the user can
 * rollback to any of them.
 */
export const promoteAllTrials = () =>
  invoke<PromoteAllTrialsResult>('promote_all_trials', { args: {} });

// =================================================================
// ==================== v0.28a — auto_promote_config =================
// =================================================================

/** v0.28a — args for `setAutoPromoteConfig`. Both fields
 * are optional: `undefined` means "leave unchanged" so
 * the L1 can update only the field the user changed in
 * the UI (e.g. just the toggle, not the margin). */
export interface SetAutoPromoteConfigArgs {
  enabled?: boolean;
  brier_margin?: number;
}

/** v0.28a — wire-format mirror of the Rust
 * `AutoPromoteConfigDto` (returned by both `get` and `set`
 * IPCs). The Rust `AppState` holds the current values;
 * the L1 pushes them via `setAutoPromoteConfig` on
 * mount of the Settings page. */
export interface AutoPromoteConfigDto {
  enabled: boolean;
  brier_margin: number;
}

/** v0.28a — push the user's auto-promote settings to
 * Rust. After this call, the `train_job` Rust handler
 * will read these values and spawn the auto-promote
 * worker if `enabled === true` and the train succeeded.
 *
 * Returns the new merged config (so the L1 can confirm
 * what Rust now has).
 *
 * Call this on mount of `Settings.tsx` so the values
 * persist across reloads. The L1 zustand store
 * (prefs-store) is the source of truth for the UI; Rust
 * is the consumer for `train_job`.
 */
export const setAutoPromoteConfig = (args: SetAutoPromoteConfigArgs = {}) =>
  invoke<AutoPromoteConfigDto>('set_auto_promote_config', { args });

/** v0.28a — read the current auto-promote config from
 * Rust. Returns defaults if the L1 has never pushed
 * any config. */
export const getAutoPromoteConfig = () =>
  invoke<AutoPromoteConfigDto>('get_auto_promote_config');

// =================================================================
// ================== v0.28a — auto_promote:finished =================
// =================================================================

/** v0.28a — payload of the `auto_promote:finished` event
 * emitted by the background worker spawned from
 * `train_job` (only if auto-promote is enabled).
 *
 * The L1 listens for this event on the ModelLab page to
 * auto-refresh the page (and optionally show a toast).
 */
export interface AutoPromoteFinishedEvent {
  /** The `job_id` from the train that triggered the
   * auto-promote. Lets the L1 correlate the event with
   * the train it just kicked off. */
  job_id: string;
  /** `true` if the candidate was actually promoted. */
  promoted: boolean;
  /** Human-readable status from the sidecar. e.g.
   * "auto-promoted: improvement 0.012 > margin 0.005"
   * or "sidecar not running" or "auto_promote parse: …". */
  message: string;
  /** New model version, if promoted. */
  model_version: string | null;
  /** When the auto-promote finished (unix millis). */
  finished_at: number;
}

/** v0.28a — listen for `auto_promote:finished` events.
 * The ModelLab page uses this to auto-refresh the
 * PromoteHistory panel + Brier chart when a background
 * auto-promote completes. */
export const onAutoPromoteFinished = (
  cb: (e: AutoPromoteFinishedEvent) => void,
): Promise<UnlistenFn> =>
  listen<AutoPromoteFinishedEvent>('auto_promote:finished', (msg) => cb(msg.payload));
