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
import type { Bet, PaperFill, PlaceJumpArgs, PlaceSignedArgs, ListBetsArgs, ValidateOrderArgsArgs } from '@/types/bet';
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
  PaperPnlSummary,
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

// v0.46a — list resolved markets formatted as
// backtest samples. The L1 uses this to pre-fill
// the BacktestReport textarea via the
// "Pull from resolved markets" button.
//
// Known limitation: price is fixed at 0.5 and
// market_age_hours at 24 (the "predict 1 day
// before close" convention). Real price
// history is not stored; this is a degenerate
// but consistent sanity check (the model should
// at least beat 0.5 on settled markets).
export interface ResolvedMarketSample {
  market_id: string;
  question: string;
  outcome: string;
  market_age_hours: number;
  price: number;
}
export interface ListResolvedMarketsForBacktestArgs {
  category?: string;
  limit?: number;
  /** Only markets that ended at or after this
   * unix-ms timestamp. Useful for "last 30
   * days" filters. */
  since_ms?: number;
}
export const listResolvedMarketsForBacktest = (
  args: ListResolvedMarketsForBacktestArgs = {},
) => invoke<ResolvedMarketSample[]>('list_resolved_markets_for_backtest', { args });

// ---------------------------------------------------------------- Signal (M2)
export const listActiveSignals = (args: ListSignalsArgs = {}) =>
  invoke<Signal[]>('list_active_signals', { args });
export const recomputeSignals = () => invoke<number>('recompute_signals');

// ---------------------------------------------------------------- Bet (M3)
export const placeJumpLink = (args: PlaceJumpArgs) =>
  invoke<string>('place_jump_link', { args });
export const placeSignedOrder = (args: PlaceSignedArgs) =>
  invoke<Bet>('place_signed_order', { args });

/** v0.50a — pure validation IPC. The L1 calls this
 * before `placeSignedOrder` for instant feedback
 * (e.g. "limit orders require limit_price") without
 * the round-trip to the DB. Returns the parsed size
 * on success; throws on validation failure. */
export const validateOrderArgs = (args: ValidateOrderArgsArgs) =>
  invoke<number>('validate_order_args', { args });

export const listBets = (args: ListBetsArgs = {}) =>
  invoke<Bet[]>('list_bets', { args });

// ---------------------------------------------------------------- Copy (M5)
export const listCopyTargets = () => invoke<CopyTarget[]>('list_copy_targets');
export const addCopyTarget = (args: AddCopyTargetArgs) =>
  invoke<CopyTarget>('add_copy_target', { args });
export const recentCopyEvents = (targetId?: string, limit = 50) =>
  invoke<CopyEvent[]>('recent_copy_events', { targetId, limit });

// ---------------------------------------------------------------- Mirror (M5 executor)
import type {
  EnqueueMirrorArgs,
  ListMirrorsArgs,
  MirrorRow,
  MirrorQueueStats,
  RunMirrorPassArgs,
  ExecutorPassResult,
} from '@/types/mirror';
export const enqueueMirror = (args: EnqueueMirrorArgs) =>
  invoke<MirrorRow>('enqueue_mirror', { args });
export const listMirrors = (args: ListMirrorsArgs = {}) =>
  invoke<MirrorRow[]>('list_mirrors', { args });
export const runMirrorExecutorPass = (args: RunMirrorPassArgs) =>
  invoke<ExecutorPassResult>('run_mirror_executor_pass', { args });
export const mirrorQueueStats = () =>
  invoke<MirrorQueueStats>('mirror_queue_stats');

// v0.44c — paper trading mode IPCs
export const setMirrorPaperMode = (args: { enabled: boolean }) =>
  invoke<boolean>('set_mirror_paper_mode', { args });
export const getMirrorPaperMode = () =>
  invoke<boolean>('get_mirror_paper_mode');
export const listPaperFills = (args: { limit?: number } = {}) =>
  invoke<PaperFill[]>('list_paper_fills', { args });

// ---------------------------------------------------------------- PnL (M6)
export const dashboardKpis = () => invoke<DashboardKpis>('dashboard_kpis');
// v0.45b — paper trading PnL summary (settled fills only)
export const paperPnlSummary = () =>
  invoke<PaperPnlSummary>('paper_pnl_summary');
// v0.48a — manual trigger for the model
// degradation check. The 7th scheduler loop
// also runs this hourly; the IPC is for "Check
// now" buttons.
export const degradationCheckNow = () =>
  invoke<void>('degradation_check_now', { args: {} });

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
  /** v0.42e-3 — optional whitelist of job_ids. When
   * supplied, only entries with matching `job_id`
   * are returned. Used by the `ModelComparison`
   * component to fetch weights for the 2-3
   * selected entries without pulling the whole
   * archive. Empty array → no entries; missing
   * → no filter. */
  job_ids?: string[];
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
// ==================== v0.43 — backtest_model ====================
// =================================================================

/** v0.43 — one backtest sample. The L1 builds this
 * list from the markets DB (resolved markets only)
 * and passes it through. */
export interface BacktestSample {
  /** The market price at predict-time (0..1). */
  price: number;
  /** How many hours since the market opened at
   * predict-time. The model's `w2` weights age. */
  market_age_hours: number;
  /** Resolution outcome (0 = NO, 1 = YES). */
  outcome: number;
  /** Optional human-readable label (e.g. the
   * market question). Surfaced in the top
   * winners/losers list. Empty string is fine. */
  label?: string;
}

/** v0.43 — one entry in the calibration histogram.
 * The L1 renders this as a small bar chart:
 * "in this prediction bucket, the actual
 * resolution rate was X (vs predicted Y)". */
export interface BacktestCalibrationBucket {
  /** Human-readable bucket label, e.g. "[0.4, 0.6)". */
  bucket: string;
  /** Mean predicted probability in this bucket.
   * `null` when the bucket is empty. */
  predicted_avg: number | null;
  /** Actual resolution rate in this bucket.
   * `null` when the bucket is empty. */
  actual_rate: number | null;
  /** Number of samples in this bucket. */
  count: number;
}

/** v0.43 — one entry in the top winners / top
 * losers list. Includes enough context to render
 * a tooltip on hover. */
export interface BacktestTopSample {
  /** Echoed from the input `label`. */
  label: string;
  /** Per-sample Brier. */
  brier: number;
  /** The model's prediction. */
  predicted: number;
  /** The actual outcome. */
  outcome: number;
}

/** v0.43 — wire-format mirror of the Python
 * sidecar's `backtest_model` response. */
export interface BacktestResult {
  /** `true` on success; `false` for unknown
   * model / empty samples / all-malformed /
   * missing weights. */
  ok: boolean;
  /** Echoed from the request. */
  model_version: string;
  /** Number of samples that passed validation
   * and contributed to Brier. */
  sample_count: number;
  /** Mean squared error of predictions vs
   * outcomes. `null` when `ok=false`. */
  brier_mean: number | null;
  /** Per-sample Brier scores, in input order.
   * Useful for client-side histograms. */
  brier_breakdown: number[];
  /** 5 calibration buckets in [0, 1]. */
  calibration: BacktestCalibrationBucket[];
  /** 3 lowest-Brier samples (best predictions). */
  top_winners: BacktestTopSample[];
  /** 3 highest-Brier samples (worst predictions),
   * reversed (worst first). */
  top_losers: BacktestTopSample[];
  /** Human-readable status / error message. */
  message: string | null;
}

/** v0.43 — args for the `backtestModel` IPC. */
export interface BacktestModelArgs {
  /** The model to backtest, e.g.
   * "logistic-train-441c352b". Looked up in
   * `archive.jsonl` first, then `active.json`. */
  model_version: string;
  /** The list of (price, market_age_hours, outcome)
   * samples to replay the model against. */
  samples: BacktestSample[];
}

/** v0.43 — replay a saved model against a list of
 * (price, market_age_hours, outcome) samples and
 * return Brier + calibration + per-sample
 * predictions. Closes the v0.17-v0.41 model
 * lifecycle gap: there's no way to ask "how
 * would this model have done on real
 * resolutions" without this.
 *
 * The sidecar is pure (no IO beyond reading the
 * model file), so per-call cost is O(samples) —
 * fast for hundreds of samples, slow for
 * millions. The L1 should pre-filter to a
 * reasonable time window.
 */
export const backtestModel = (args: BacktestModelArgs) =>
  invoke<BacktestResult>('backtest_model', { args });

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
// ================== v0.42c — telemetry enabled toggle ============
// =================================================================

/** v0.42c — args for `setTelemetryEnabled`. The L1
 * pushes this from the Settings telemetry toggle. */
export interface SetTelemetryEnabledArgs {
  enabled: boolean;
}

/** v0.42c — runtime override of the telemetry gate.
 *
 * The default is whatever the env var `POLYROCKET_TELEMETRY`
 * was at process start (typically `false`). After this
 * call, the env var is ignored for the lifetime of the
 * process — the user's choice wins.
 *
 * When ON, every Rust lifecycle event (train started /
 * completed / failed, promote completed, scheduler tick,
 * etc.) writes one NDJSON line to stderr. Capture with:
 *
 *     polyrocket 2> telemetry.log
 *
 * When OFF, `emit()` is a no-op (atomic load, zero
 * overhead).
 */
export const setTelemetryEnabled = (args: SetTelemetryEnabledArgs) =>
  invoke<boolean>('set_telemetry_enabled', { args });

/** v0.42c — read the current telemetry state. Returns
 * the current effective value (env var at startup
 * unless the L1 has overridden it). */
export const getTelemetryEnabled = () =>
  invoke<boolean>('get_telemetry_enabled');

// =================================================================
// ================== v0.49a — telemetry log file retention ========
// =================================================================

/** v0.49a — one row in the on-disk telemetry log
 * inventory. Returned by `listTelemetryLogs`. The
 * current process's active session file is flagged
 * `isCurrent = true`. The L1 TelemetryCard uses this
 * to show "this session" / "older sessions" with their
 * sizes. */
export interface TelemetryLogInfo {
  /** File name only, e.g. `session-1740000000.jsonl`. */
  name: string;
  /** Absolute path on disk. The L1 does not open
   * this directly (Rust is the IO owner); the
   * path is shown in the UI for reference only. */
  path: string;
  /** File size in bytes. */
  sizeBytes: number;
  /** File modification time, unix seconds. */
  modifiedUnix: number;
  /** True if this is the active session file
   * (the current process is appending to it). */
  isCurrent: boolean;
}

/** v0.49a — list the on-disk telemetry session
 * files in the current log dir. The current process's
 * file is flagged `isCurrent = true`. Empty list
 * when no log dir is set yet (e.g. before startup
 * setup finished). */
export const listTelemetryLogs = () =>
  invoke<TelemetryLogInfo[]>('list_telemetry_logs');

/** v0.49a — manually trigger a retention sweep.
 * Deletes session files older than
 * `POLYROCKET_TELEMETRY_RETENTION_DAYS` (default 14).
 * Returns the count of files deleted. The same
 * sweep runs automatically on startup. */
export const purgeTelemetryLogs = () =>
  invoke<number>('purge_telemetry_logs');

// =================================================================
// ================== v0.49b — active model ========================
// =================================================================

/** v0.49b — single source of truth for "what
 * model is currently active and what are its
 * training metrics". The L1 always reads the
 * active model through this IPC instead of
 * duplicating the active.json file path logic.
 *
 * Returns `null` when no model has been promoted
 * yet (typical first-run state).
 *
 * The `weights` field is reserved for v0.50+,
 * where the sidecar will start writing the
 * trained weights into active.json. Today it's
 * always `null`. */
export interface ActiveModel {
  /** e.g. "logistic-train-441c352b" */
  modelVersion: string;
  /** Train-time Brier score. `null` when missing. */
  bestBrier: number | null;
  /** Hyperparameters of the best trial. */
  bestParams: Record<string, unknown> | null;
  /** Wall-clock time of the last promote, ms. */
  promotedAtMs: number | null;
  /** v0.50+ — currently always `null`. */
  weights: number[] | null;
  /** Absolute path to active.json (for the UI to
   * show "data lives at ..." for power users). */
  sourcePath: string;
}

/** v0.49b — read the active model from disk.
 * Returns `null` when no model has been promoted
 * yet (active.json is missing). Returns an error
 * when active.json exists but is malformed. */
export const getActiveModel = () =>
  invoke<ActiveModel | null>('get_active_model');

// =================================================================
// ================== v0.49c — scheduler self-test =================
// =================================================================

/** v0.49c — one loop's status from the
 * scheduler self-test. The Rust side captures
 * the unix-ms of each loop's most recent tick
 * in a process-global atomic; the L1 polls this
 * snapshot to render a row of green/red dots. */
export interface SchedulerLoopStatus {
  /** Stable loop name, e.g. "health_probe".
   * Matches the variant names in
   * `infra::scheduler::record_tick`. */
  name: string;
  /** Unix-ms of the most recent tick. 0 = never
   * ticked (still in its initial stagger sleep). */
  lastTickUnixMs: number;
  /** Milliseconds since the last tick.
   * `null` if the loop has never ticked. */
  ageMs: number | null;
  /** True when `ageMs <= 3 * expected_interval_ms`. */
  healthy: boolean;
}

/** v0.49c — the scheduler self-test snapshot.
 * Returned by `schedulerSelfTestNow`. Cheap to
 * call (just reads atomic counters, no IO). */
export interface SchedulerSelfTest {
  /** Unix-seconds when this process started. */
  processStartedAtUnix: number;
  /** Unix-ms when the self-test ran. */
  checkedAtUnixMs: number;
  /** True when every loop is healthy. */
  allHealthy: boolean;
  /** Per-loop status, alphabetically sorted. */
  loops: SchedulerLoopStatus[];
}

/** v0.49c — return a snapshot of the 8
 * background scheduler loops' liveness. The
 * L1 Settings card renders this as a row of
 * green/red dots. */
export const schedulerSelfTestNow = () =>
  invoke<SchedulerSelfTest>('scheduler_self_test_now');

// =================================================================
// ================== v0.50c — fill analytics ======================
// =================================================================

/** v0.50c — one bucket in the order-type
 * breakdown of fill_analytics. */
export interface OrderTypeBucket {
  orderType: 'market' | 'limit' | 'stop_loss';
  count: number;
  settled: number;
  won: number;
  /** Realized PnL in USDC across settled rows in
   * this bucket. Positive = gains. */
  realizedPnlUsdc: number;
}

/** v0.50c + v0.51b — fill analytics summary, returned
 * by `fillAnalytics`. Aggregates the `bets` table.
 *
 * v0.51b adds slippage + time-to-fill + partial-fill
 * metrics. For the v0.5d deterministic stub these
 * are 0 / 0ms / 0 (since fill_price = price and
 * filled_at = placed_at by construction). v0.51+ wires
 * the real CLOB and these reflect actual execution. */
export interface FillAnalytics {
  totalFills: number;
  openCount: number;
  wonCount: number;
  lostCount: number;
  cancelledCount: number;
  /** Average (settled_at - placed_at) in ms.
   * `null` when no bets have settled yet. */
  avgTimeToSettlementMs: number | null;
  /** won / (won + lost + cancelled). 0.0 when
   * nothing is settled. */
  winRate: number;
  /** Realized PnL across settled rows, USDC. */
  realizedPnlUsdc: string;
  /** Per-order-type breakdown (3 buckets always
   * present, even when empty). */
  byOrderType: OrderTypeBucket[];
  /** Number of fills that were post_only. */
  postOnlyCount: number;
  /** postOnlyCount / totalFills. 0.0 when 0 fills. */
  postOnlyRate: number;
  /** v0.51b — average |fill_price - price| across
   * filled rows. `null` when no rows have a
   * fill_price (pre-v0.51b DB). */
  avgSlippage: number | null;
  /** v0.51b — average (filled_at - placed_at) in
   * ms. `null` when no rows have a filled_at. */
  avgTimeToFillMs: number | null;
  /** v0.51b — count of partial fills. */
  partialFillCount: number;
  /** v0.51b — partialFillCount / totalFills. 0.0 when 0. */
  partialFillRate: number;
}

/** v0.50c — return the fill analytics summary. */
export const fillAnalytics = () =>
  invoke<FillAnalytics>('fill_analytics');

// =================================================================
// ================== v0.51a — CLOB snapshots =====================
// =================================================================

/** v0.51a — one snapshot of the full order book
 * for a market. `bids` is sorted DESC by price
 * (best bid first); `asks` is sorted ASC (best ask
 * first). Each tuple is `[price, size]`. */
export interface ClobSnapshot {
  marketId: string;
  capturedAt: number;
  /** `[price, size]` tuples, sorted DESC by price. */
  bids: Array<[number, number]>;
  /** `[price, size]` tuples, sorted ASC by price. */
  asks: Array<[number, number]>;
}

/** v0.51a — current CLOB feed status.
 *
 * Returns "not_configured" when the env vars
 * `POLYROCKET_CLOB_API_KEY` /
 * `POLYROCKET_CLOB_API_SECRET` /
 * `POLYROCKET_CLOB_API_PASSPHRASE` aren't all set.
 *
 * Returns "configured" when credentials are present
 * but the actual WebSocket listener isn't yet wired
 * (v0.51+).
 *
 * The L1 falls back to v0.47a `price_snapshots`
 * when state is "not_configured". */
export interface ClobFeedStatus {
  state: 'not_configured' | 'configured' | 'connected';
  /** Distinct (market_id, captured_at) pairs in the
   * table. Counts each "snapshot" once even though
   * it has many rows (one per price level). */
  totalSnapshots: number;
  /** Number of distinct markets with at least one
   * snapshot. */
  marketsWithSnapshots: number;
}

/** v0.51a — return the current CLOB feed status. */
export const clobFeedStatus = () =>
  invoke<ClobFeedStatus>('clob_feed_status');

/** v0.51a — args for `recordClobSnapshotNow`.
 * Bids and asks are `[price, size]` tuples. */
export interface RecordClobSnapshotArgs {
  marketId: string;
  capturedAt: number;
  bids: Array<[number, number]>;
  asks: Array<[number, number]>;
}

/** v0.51a — manual one-shot insert. Records one
 * snapshot and returns the number of rows
 * (bids.length + asks.length). */
export const recordClobSnapshotNow = (args: RecordClobSnapshotArgs) =>
  invoke<number>('record_clob_snapshot_now', { args });

/** v0.51a — return the most recent snapshot for
 * a market. Returns `null` when the market has
 * no snapshots. */
export const latestClobSnapshot = (marketId: string) =>
  invoke<ClobSnapshot | null>('latest_clob_snapshot', { marketId });

// =================================================================
// ================== v0.53a — storage path ========================
// =================================================================

/** v0.53a — current storage path information.
 * Returned by `getStorageInfo`. The L1 uses this
 * to render the "Welcome > Storage path" step
 * and the Settings → Storage card. */
export interface StorageInfo {
  /** OS-recommended path (Tauri's app_data_dir). */
  defaultPath: string;
  /** The path polyrocket will use on the next
   * launch. Equals `defaultPath` when the user
   * hasn't picked a custom one. */
  currentPath: string;
  /** True when currentPath !== defaultPath. */
  isCustom: boolean;
  /** True when currentPath exists on disk. */
  exists: boolean;
  /** True when the current user can write to
   * currentPath. */
  writable: boolean;
  /** Free space in bytes. `null` when the OS
   * query fails (sandbox / exotic fs). The L1
   * hides the metric when this is null. */
  freeBytes: number | null;
  /** True when the user picked a custom path but
   * the current process is still using the default
   * (i.e. the user needs to restart). The L1
   * surfaces a "Restart now" button when this is
   * true. */
  restartRequired: boolean;
}

/** v0.53a — return the current storage path
 * information. Cheap (just std::fs metadata +
 * settings lookup). */
export const getStorageInfo = () =>
  invoke<StorageInfo>('get_storage_info');

/** v0.53a — write a custom storage path. The
 * change takes effect on the NEXT launch
 * (the current process's DB is already open).
 *
 * Throws AppError::Invalid when the path is not
 * absolute, doesn't exist, isn't a directory, or
 * isn't writable. The L1 must call `getStorageInfo`
 * again after this to refresh the UI; the
 * `restartRequired` flag will be true. */
export const setStoragePath = (path: string) =>
  invoke<void>('set_storage_path', { args: { path } });

/** v0.53a — clear the custom storage path.
 * Next launch falls back to the OS default. */
export const resetStoragePath = () =>
  invoke<void>('reset_storage_path');

// =================================================================
// ================== v0.56 — network proxy / Tor =================
// =================================================================

/** v0.56 — proxy configuration as the user
 * sees it. `enabled` is a separate boolean so
 * the user can disable the proxy without
 * clearing the URL (faster toggle on/off). */
export interface ProxyConfig {
  enabled: boolean;
  url: string | null;
  scheme: 'http' | 'socks5' | null;
  restartRequired: boolean;
}

/** v0.56 — return the current proxy
 * configuration. The L1 uses this to render
 * the Settings → Network card. */
export const getProxyConfig = () =>
  invoke<ProxyConfig>('get_proxy_config');

/** v0.56 — write the proxy configuration. The
 * change takes effect on next launch (active
 * session's HTTP client is already built). The
 * L1 surfaces a "Restart required" banner after
 * this returns. */
export const setProxyConfig = (
  enabled: boolean,
  url: string | null,
): Promise<ProxyConfig> =>
  invoke<ProxyConfig>('set_proxy_config', {
    args: { enabled, url: url ?? null },
  });

/** v0.56 — clear the proxy entirely (URL +
 * enabled flag). Next launch uses direct
 * outbound HTTP. */
export const clearProxyConfig = () =>
  invoke<void>('clear_proxy_config');

/** v0.56 — read the proxy URL from the
 * `network_proxy.json` file (the source of
 * truth at startup). Useful for the L1's
 * "Restart required" hint + sanity check. */
export const readProxyConfigFile = () =>
  invoke<string | null>('read_proxy_config_file');

// =================================================================
// ================== v0.55 — model explainability =================
// =================================================================

/** v0.55 — input sample for an explainability
 * query. Both fields are optional; omitting
 * both gives a default sample (price=0.5, age=24h). */
export interface ExplainSample {
  /** The market price, 0..1. */
  price?: number;
  /** The market age in hours, >=0. */
  market_age_hours?: number;
}

/** v0.55 — one feature's contribution to a
 * single prediction. The L1 renders this as a
 * horizontal bar chart. */
export interface ExplainFeature {
  feature: string;
  value: number;
  weight: number;
  /** Contribution to (p - 0.5). Positive =
   * "moved the prediction higher", negative =
   * "moved it lower". */
  contribution: number;
  abs_contribution: number;
}

/** v0.55 — outcome of `explainModel`. The
 * `features` array is sorted by
 * `abs_contribution` descending. */
export interface ExplainResult {
  ok: boolean;
  model_version: string;
  features: ExplainFeature[];
  prediction: number | null;
  sample: { price: number; market_age_hours: number } | null;
  message: string;
}

/** v0.55 — args for `explainModel`. The
 * `sample` is optional; when omitted, the
 * sidecar uses a default sample so the user
 * gets a "what would the model say for a
  * typical market" view. */
export const explainModel = (
  model_version: string,
  sample?: ExplainSample,
): Promise<ExplainResult> =>
  invoke<ExplainResult>('explain_model', {
    args: { model_version, sample: sample ?? null },
  });

// =================================================================
// ================== v0.59 — KernelSHAP ===========================
// =================================================================

/** v0.59 — one feature's SHAP value. Same
 * shape as ExplainFeature (v0.55) but with
 * `shap_value` / `abs_shap` instead of
 * `contribution` / `abs_contribution` to make
 * the SHAP math explicit in the L1. */
export interface ShapFeature {
  feature: string;
  value: number;
  weight: number;
  /** The SHAP value φ_i. Positive = "moved the
   * prediction higher", negative = "moved it
   * lower". Satisfies:
   *   Σφ_i = f(x) - E[f(x)]
   * (the SHAP efficiency axiom). */
  shap_value: number;
  /** |shap_value|. Used for sorting + chart
   * bar length. */
  abs_shap: number;
}

/** v0.59 — outcome of `shapExplain`. Includes
 * the `baseline_prediction` (empty-coalition
 * value) and `target_prediction` (full
 * coalition) so the L1 can show "the SHAP
 * values sum to f(x) - E[f(x)]" as a hint.
 * `efficiency_diff` is the residual after
 * fitting; should be ~0 within float
 * tolerance. */
export interface ShapResult {
  ok: boolean;
  model_version: string;
  /** Always "kernel_shap" today. Future
   * variants (e.g. "tree_shap" for tree-based
   * models) can set this differently. */
  method: string;
  features: ShapFeature[];
  baseline_prediction: number | null;
  target_prediction: number | null;
  efficiency_diff: number | null;
  sample: { price: number; market_age_hours: number } | null;
  message: string;
}

/** v0.59 — compute true SHAP values via the
 * sidecar's KernelExplainer. For the 3-
 * feature polyrocket model the cost is 8
 * coalition evaluations (~100µs); for a
 * tree-based model with M > 5 we'd need
 * TreeSHAP. v0.59 candidate. */
export const shapExplain = (
  model_version: string,
  sample?: ExplainSample,
): Promise<ShapResult> =>
  invoke<ShapResult>('shap_explain', {
    args: { model_version, sample: sample ?? null },
  });

// =================================================================
// ================== v0.54b — storage migration ===================
// =================================================================

/** v0.54b — outcome of `migrateStoragePath`. The
 * L1 uses `noop` to skip the "data migrated"
 * toast (when the source had nothing to copy,
 * e.g. clean install on a new path). */
export interface MigrateStoragePathResult {
  from: string;
  to: string;
  filesCopied: number;
  bytesCopied: number;
  overwritten: boolean;
  noop: boolean;
}

/** v0.54b — copy `polyrocket.db` + `logs/` from
 * the source (currently-active path) to `dest`.
 * The dest must be the path the user just set via
 * `setStoragePath` (or the OS default when they
 * called `resetStoragePath`).
 *
 * Idempotency: a second call with the same args
 * is a no-op (returns `noop: true`).
 *
 * Throws AppError::Invalid when:
 *   - dest is not absolute / doesn't exist / isn't
 *     a directory / isn't writable
 *   - dest is non-empty AND overwrite=false
 *
 * For large DBs this can take a few seconds.
 * The L1 should call it from a background
 * mutation so the UI doesn't freeze. */
export const migrateStoragePath = (
  dest: string,
  overwrite = false,
): Promise<MigrateStoragePathResult> =>
  invoke<MigrateStoragePathResult>('migrate_storage_path', {
    args: { dest, overwrite },
  });

// =================================================================
// ================== v0.54a — tauri-plugin-dialog =================
// =================================================================

/** v0.54a — open a native directory picker. Returns
 * the absolute path the user picked, or `null` if
 * they cancelled. Wraps the `dialog:open` plugin
 * command — used by the Welcome → Storage step's
 * "Browse..." button so the user doesn't have to
 * type a full path. */
export const pickDirectory = (): Promise<string | null> =>
  invoke<string | null>('pick_directory');

/** v0.54a — open a native file picker. Used by the
 * LLM management page to import API keys from a
 * `.env`-shaped file, and by the wallet manager to
 * import a wallet JSON. `filters` is optional; when
 * omitted the user sees all files. Returns the
 * absolute path the user picked, or `null` if
 * they cancelled. */
export interface FileFilter {
  name: string;
  extensions: string[];
}
export const pickFile = (
  filters: FileFilter[] = [],
  multiple = false,
): Promise<string | null | string[]> =>
  invoke<string | null | string[]>('pick_file', {
    args: { filters, multiple },
  });

// =================================================================
// ================== v0.51c — CLOB submit ========================
// =================================================================

/** v0.51c — outcome of a CLOB submit attempt.
 * Mirrors `domain::polymarket::ClobOrderResult`.
 * The L1 reads this from the `placeSignedOrder`
 * response's audit-log payload (audit_log has
 * via_http/fill_price/fill_size/partial fields
 * recorded per v0.51c). For the L1 place-bet
 * form (v0.52+), the full submit response is
 * surfaced via the BetDto's filled_at/fill_price/
 * fill_size/partial fields. */
export interface ClobOrderResult {
  ok: boolean;
  txHash: string;
  filledAtMs: number;
  fillPrice: number;
  fillSize: string;
  partial: boolean;
  error: string;
  /** True when the call was made via the HTTP
   * path (creds present + reachable). False when
   * the deterministic stub was used. */
  viaHttp: boolean;
}

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
