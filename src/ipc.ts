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
import type {
  Wallet,
  AddWalletArgs,
} from '@/types/wallet';
import type { Market, ListMarketsArgs, MarketSummary } from '@/types/market';
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
export const fetchActiveMarkets = () => invoke<MarketSummary[]>('fetch_active_markets');

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
export const recordLlmDecision = (analysisId: number, decision: string) =>
  invoke<void>('record_llm_decision', { analysisId, decision });
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
        'provider_auto_disable' | 'daily_brief' | 'mirror_decision' | 'info',
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

// ---------------------------------------------------------------- Brief (M12)
export const dailyBriefGet = (limit = 5) =>
  invoke<DailyBriefEntry[]>('daily_brief_get', { limit });
export const dailyBriefDismiss = (marketId: string) =>
  invoke<void>('daily_brief_dismiss', { marketId });
export const dailyBriefRefresh = () => invoke<BriefRefreshResult>('daily_brief_refresh');
export const dailyBriefSetPrefs = (args: SetBriefPrefsArgs) =>
  invoke<void>('daily_brief_set_prefs', { args });
