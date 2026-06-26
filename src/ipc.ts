/**
 * L1 ↔ L2 IPC 客户端。对 `invoke('cmd_name', { args })` 的类型化包装。
 *
 * 每个函数与 `src-tauri/src/commands/*.rs` 中的一个
 * `#[tauri::command]` 一一对应。如果你新增了一个 Rust 命令，
 * 请在同一 commit 中在此处添加对应包装
 * （CI 会强制检查模块映射）。
 *
 * 分层规则：本模块是调用 `invoke` 的**唯一**位置。
 * 路由 / 组件从这里导入，**绝不**直接从
 * `@tauri-apps/api/core` 导入。参见 docs/overview.md §1.2
 * （L1 → L2，单向）。
 */

import { invoke } from '@tauri-apps/api/core';
import type { InvokeArgs } from '@tauri-apps/api/core';
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
export const listWallets = () => safeInvoke<Wallet[]>('list_wallets');
export const addWallet = (args: AddWalletArgs) => safeInvoke<Wallet>('add_wallet', { args });
// v0.123 —— 从 Polymarket CLOB（L2 HMAC）读取 USDC 余额。返回
// 一个 `BalanceResult`（ok/balance_usdc/reason）—— `ok=false` 是正常
// 结果（凭据缺失、地址缺失、网络不可用）。
export interface BalanceResult {
  ok: boolean;
  balance_usdc: number;
  raw_balance: string;
  reason: string;
  creds_present: boolean;
}
export const getWalletBalance = () => safeInvoke<BalanceResult>('get_wallet_balance');

// ---------------------------------------------------------------- Market (M1)
export const listMarkets = (args: ListMarketsArgs = {}) =>
  invoke<Market[]>('list_markets', { args });
export const syncMarkets = () => safeInvoke<number>('sync_markets');

// v0.46a —— 将已结算的市场格式化为
// 回测样本。L1 用它在用户点击
// 「Pull from resolved markets」按钮时
// 预填 BacktestReport 文本框。
//
// 已知限制：价格固定为 0.5，
// market_age_hours 固定为 24（「提前 1 天预测」
// 的约定）。未存储真实价格历史；
// 这只是一个退化但一致的健全性检查
// （模型至少应该在已结算市场上
// 优于 0.5）。
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
  /** 仅包含结束时间不早于此 unix-ms
   * 时间戳的市场。便于实现「最近 30 天」
   * 这样的过滤。 */
  since_ms?: number;
}
export const listResolvedMarketsForBacktest = (
  args: ListResolvedMarketsForBacktestArgs = {},
) => safeInvoke<ResolvedMarketSample[]>('list_resolved_markets_for_backtest', { args });

// ---------------------------------------------------------------- Signal (M2)
export const listActiveSignals = (args: ListSignalsArgs = {}) =>
  invoke<Signal[]>('list_active_signals', { args });
export const recomputeSignals = () => safeInvoke<number>('recompute_signals');

// ---------------------------------------------------------------- Bankroll (M11) v0.78
import type { ComputeAllocationArgs, AllocationResult, BankrollConfigDto } from '@/types/bankroll';
export type { ComputeAllocationArgs };

/**
 * v0.78 —— 计算资金分配预览。纯计算，无 DB
 * 写入。接收 signals + bankroll + （可选）
 * config + （可选）流动性映射。返回
 * 确定性的逐市场分配。
 *
 * 实际的 `apply_allocation`（写入 `bets` 表）
 * 在 v0.78e 中。
 */
export const computeAllocationPreview = (args: ComputeAllocationArgs) =>
  safeInvoke<AllocationResult>('compute_allocation_preview', { args });

/** v0.78d —— 获取按钱包划分的资金配置。未设置时返回默认值。 */
export const getBankrollConfig = (walletId: string) =>
  safeInvoke<BankrollConfigDto>('get_bankroll_config', { walletId });

/** v0.78d —— 设置按钱包划分的资金配置。先校验。 */
export const setBankrollConfig = (walletId: string, config: BankrollConfigDto) =>
  safeInvoke<void>('set_bankroll_config', { walletId, config });

/** v0.78e —— 应用分配结果。写入 allocation_batches。 */
export const applyAllocation = (
  walletId: string,
  result: AllocationResult,
  bankrollUsdc: string,
  config: BankrollConfigDto,
) =>
  safeInvoke<string>('apply_allocation', {
    walletId,
    result,
    bankrollUsdc,
    config,
  });

// ---------------------------------------------------------------- Bet (M3)
export const placeJumpLink = (args: PlaceJumpArgs) =>
  invoke<string>('place_jump_link', { args });
export const placeSignedOrder = (args: PlaceSignedArgs) =>
  invoke<Bet>('place_signed_order', { args });

/** v0.50a —— 纯校验 IPC。L1 在调用
 * `placeSignedOrder` 之前先调它，
 * 以获得即时反馈（例如「限价单需要 limit_price」），
 * 避免到 DB 的往返。成功时返回解析后的 size；
 * 校验失败时抛错。 */
export const validateOrderArgs = (args: ValidateOrderArgsArgs) =>
  invoke<number>('validate_order_args', { args });

export const listBets = (args: ListBetsArgs = {}) =>
  invoke<Bet[]>('list_bets', { args });

// ---------------------------------------------------------------- Copy (M5)
export const listCopyTargets = () => safeInvoke<CopyTarget[]>('list_copy_targets');
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

// v0.44c —— 模拟交易模式的 IPC
export const setMirrorPaperMode = (args: { enabled: boolean }) =>
  invoke<boolean>('set_mirror_paper_mode', { args });
export const getMirrorPaperMode = () =>
  invoke<boolean>('get_mirror_paper_mode');
export const listPaperFills = (args: { limit?: number } = {}) =>
  invoke<PaperFill[]>('list_paper_fills', { args });

// ---------------------------------------------------------------- PnL (M6)
export const dashboardKpis = () => safeInvoke<DashboardKpis>('dashboard_kpis');
// v0.45b —— 模拟交易 PnL 汇总（仅包含已结算成交）
export const paperPnlSummary = () =>
  invoke<PaperPnlSummary>('paper_pnl_summary');
// v0.48a —— 手动触发模型降级检查。
// 第 7 个调度循环也会每小时运行一次；
// 这个 IPC 是为「立即检查」按钮提供的。
export const degradationCheckNow = () =>
  invoke<void>('degradation_check_now', { args: {} });

// ---------------------------------------------------------------- LLM Analysis (M10)
export const listLlmProviders = () => safeInvoke<LlmProvider[]>('list_llm_providers');
export const upsertLlmProvider = (args: UpsertLlmProviderArgs) =>
  invoke<LlmProvider>('upsert_llm_provider', { args });
export const llmAnalyze = (marketId: string, providers?: string[]) =>
  invoke<LlmAnalysis>('llm_analyze', { marketId, providers });
export const llmPerformance = () => safeInvoke<LlmPerformance[]>('llm_performance');

/**
 * v0.16b —— `record_llm_decision` 的参数结构。
 *
 * Rust 端接受 `RecordDecisionArgs` 结构
 * （见 `commands::llm::RecordDecisionArgs`）。Tauri 2 会
 * 自动把 JS 对象的 camelCase key 转换为
 * Rust 结构体的 snake_case 字段，因此我们传入
 * `{ analysisId, userDecision, ... }`，Tauri 将它们
 * 映射为 `analysis_id`、`user_decision` 等。
 *
 * `user_decision` 取值之一：`'follow_top' | 'manual_yes'
 * | 'manual_no' | 'skip' | 're_analyze'`。
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
export const llmStatsHeatmap = () => safeInvoke<LlmHeatmapCell[]>('llm_stats_heatmap');
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
export const llmProviderList = () => safeInvoke<LlmProvider[]>('llm_provider_list');
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
export const llmKeyDelete = (keyId: string) => safeInvoke<void>('llm_key_delete', { keyId });
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
export const llmPmClearCredentials = () => safeInvoke<void>('llm_pm_clear_credentials');
export const polyrocketWalletSetPk = (alias: string, pk: string) =>
  invoke<void>('polyrocket_wallet_set_pk', { alias, pk });
export const polyrocketWalletClearPk = (alias: string) =>
  invoke<void>('polyrocket_wallet_clear_pk', { alias });
export const secretsStatus = () => safeInvoke<SecretsStatus>('secrets_status');

// ---------------------------------------------------------------- Scheduler (L2)
export const schedulerStatus = () => safeInvoke<SchedulerStatus>('scheduler_status');
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
) => safeInvoke<number>('send_notification', {
  args: { kind, title, body, prefs_enabled: prefsEnabled },
});
export const requestNotificationPermission = () =>
  invoke<boolean>('request_notification_permission');
export const notificationPermissionState = () =>
  invoke<string>('notification_permission_state');

// ---------------------------------------------------------------- Audit (X1)
export const listAuditLog = (limit = 200) => safeInvoke<AuditEntry[]>('list_audit_log', { limit });
export const auditCountForActor = (actor: string) =>
  invoke<number>('audit_count_for_actor', { actor });
// v0.8c —— 手动触发每日保留清理。
export const purgeAuditLogNow = () => safeInvoke<number>('purge_audit_log_now');

// v0.13c —— 读取用户覆盖的保留策略。
// 返回有效策略（用户覆盖与默认值的合并）。
export const getAuditRetention = () =>
  invoke<AuditRetentionView>('get_audit_retention');

// v0.13c —— 设置用户的保留策略。
// 立即触发一次清理，并返回被删除的行数。
export const setAuditRetention = (args: SetAuditRetentionArgs) =>
  invoke<number>('set_audit_retention', { args });

export interface AuditRetentionView {
  /** 以毫秒为单位的年龄阈值。超过该阈值的行
   * 可被清理。 */
  retain_recent_ms: number;
  /** 总行数的硬性上限。 */
  max_rows: number;
  /** 安全下限 —— 自动清理永远不会把行数
   * 降到该值以下。 */
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
export const dailyBriefRefresh = () => safeInvoke<BriefRefreshResult>('daily_brief_refresh');
export const dailyBriefSetPrefs = (args: SetBriefPrefsArgs) =>
  invoke<void>('daily_brief_set_prefs', { args });

// ---------------------------------------------------------------- Seed (v0.8a)
// 首次运行的演示数据，使 UI 开箱即显示一个完整的仪表盘。
// 幂等：除非 `force: true`，第二次调用是 no-op。
export const seedDemoData = (force = false) =>
  invoke<number>('seed_demo_data', { args: { force } });
export const isSeeded = () => safeInvoke<boolean>('is_seeded');

// ---------------------------------------------------------------- Sidecar health (v0.10d)
// 滚动「最近 N 次探测」快照，供顶栏状态徽章使用。
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
// v0.12a —— predict 在返回逐行预测的基础上
// 同时返回 model_version。L1 ModelLab 页面
// 显示一个类似「scoring with logistic-train-441c352b」
// 的小标签，让用户知道当前的分数是由哪个模型产出的。
export interface Prediction {
  market_id: string;
  prob: number;
  confidence: number;
  rationale: string | null;
}
export interface PredictResult {
  predictions: Prediction[];
  model_version: string | null;
  /** v0.13b —— 最近一次晋升模型的 Brier 分数。 */
  brier_score: number | null;
}
export const sidecarPredict = (markets: Array<[string, number]>) =>
  invoke<PredictResult>('sidecar_predict', { args: { markets } });

// v0.13d —— 异步友好的版本。返回完整的 PredictResult
// （predictions + model_version + brier_score）。
// 当 sidecar 未运行时回退到一个空的 PredictResult。
export const sidecarPredictAsync = (
  markets: Array<[string, number]>,
  timeoutMs?: number,
) =>
  invoke<PredictResult>('sidecar_predict_async', {
    args: { markets, timeout_ms: timeoutMs ?? null },
  });

// ---------------------------------------------------------------- LLM analyze progress (v0.15a)
// 由 `commands::llm::llm_analyze` 发出的四个
// `llm_analyze:*` 事件的事件负载。
// Rust 端请参见 `domain::llm::progress`。
// 每个事件都有稳定的结构；L1 可以监听
// 这四个事件并在收到时更新 UI。

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
  /** 匹配 `domain::llm::err::*` 以及字面值
   * `"none"` 和 `"parse"`（用于 HTTP 成功但 JSON
   * 解析失败的情况）。 */
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

/**
 * 检测是否运行在 Tauri runtime 中（相对于纯 Vite 开发模式）。
 *
 * 在纯 Vite 开发模式下（例如不带 `tauri dev` 的 `pnpm dev`），
 * `@tauri-apps/api/event.listen` 调用会失败并报
 * 「Cannot read properties of undefined (reading 'transformCallback')」，
 * 因为 IPC bridge 没有被注入。那些在 useEffect 中调用
 * onAnalyzeStarted / onTrainStarted 等的页面会在挂载时
 * 崩溃，导致页面无法渲染。
 *
 * 检测方式：`window.__TAURI_INTERNALS__` 在 Tauri 启动时被设置。
 * 退化方案是检查 `window.isTauri`（旧版约定）。
 */
const isTauriRuntime = (): boolean =>
  typeof window !== 'undefined' &&
  (Boolean((window as any).__TAURI_INTERNALS__) ||
    Boolean((window as any).__TAURI__) ||
    Boolean((window as any).isTauri));

/**
 * `listen()` 的安全包装。当运行在 Tauri 之外时
 * （Vite dev / Storybook / 单元测试）返回一个 no-op UnlistenFn。
 * 页面可以无条件调用 onAnalyzeStarted() 等而不会
 * 摧毁整个 React 树。
 */
const safeListen = <T>(event: string, cb: (msg: { payload: T }) => void): Promise<UnlistenFn> => {
  if (!isTauriRuntime()) {
    // No-op：没有 Tauri runtime 的开发模式。回调永远不会触发。
    // 页面应该已经处理「没有事件到达」的状态。
    return Promise.resolve(() => {});
  }
  return listen<T>(event, cb);
};

/**
 * 面向纯 Vite 开发模式的 `invoke()` 安全包装。
 * 在真实的 Tauri 构建中这是一个直传；在纯 Vite 开发
 * （无 Tauri runtime）下底层 `invoke` 会抛出
 * 「Cannot read properties of undefined (reading 'invoke')」，
 * 使整个 React 树崩溃。我们将其转换为一个带清晰消息的
 * 类型化 reject，以便页面能渲染正常的
 * error / empty 状态，而不是整页崩溃。
 *
 * 页面不应直接调用 `invoke` —— 应始终
 * 通过本文件中它们的类型化包装来调用。
 * （L1 ↔ L2 分层规则。）
 */
const safeInvoke = <T>(cmd: string, args?: InvokeArgs): Promise<T> => {
  if (!isTauriRuntime()) {
    return Promise.reject(
      new Error(
        `[polyrocket] IPC '${cmd}' unavailable: not running inside Tauri. ` +
          `Use 'pnpm tauri:dev' (or 'pnpm tauri:build') to run the full app, ` +
          `or mock the IPC in unit tests.`,
      ),
    );
  }
  return invoke<T>(cmd, args);
};

/** 监听 `llm_analyze:started` 事件。
 * 返回一个 unlisten 函数。在 `useEffect`
 * 的清理中使用以避免泄露。 */
export const onAnalyzeStarted = (cb: (e: AnalyzeStartedEvent) => void): Promise<UnlistenFn> =>
  safeListen<AnalyzeStartedEvent>('llm_analyze:started', (msg) => cb(msg.payload));

/** 监听 `llm_analyze:provider_done` 事件。
 * 每个 provider 触发一次，顺序任意
 * （因为 fan-out 是并行的）。 */
export const onProviderDone = (cb: (e: ProviderDoneEvent) => void): Promise<UnlistenFn> =>
  safeListen<ProviderDoneEvent>('llm_analyze:provider_done', (msg) => cb(msg.payload));

/** 监听 `llm_analyze:consensus_done` 事件。
 * 每次 analyze 触发一次，在所有 provider 完成
 * 并计算完共识之后。 */
export const onConsensusDone = (cb: (e: ConsensusDoneEvent) => void): Promise<UnlistenFn> =>
  safeListen<ConsensusDoneEvent>('llm_analyze:consensus_done', (msg) => cb(msg.payload));

/** 监听 `llm_analyze:finished` 事件。
 * L1 通常 `await` 这个终态事件来得知
 * analyze 已经完成。 */
export const onAnalyzeFinished = (cb: (e: AnalyzeFinishedEvent) => void): Promise<UnlistenFn> =>
  safeListen<AnalyzeFinishedEvent>('llm_analyze:finished', (msg) => cb(msg.payload));

// ---------------------------------------------------------------- Train job (v0.17a)
// 由 `commands::sidecar::train_job` 发出的
// `train_job:*` 事件的事件负载。
// Rust 端请参见 `domain::lab::train_progress`。
// 两个事件：started（在 sidecar 调用之前）
// 和 finished（在 sweep 完成或失败之后）。

export interface TrainStartedEvent {
  job_id: string;
  /** Python sidecar 将运行的 trial 总数（1-4）。 */
  n_trials: number;
  /** 每个 trial 的训练 epoch（默认 80）。 */
  epochs: number;
  started_at: number;
}

export interface TrainTrialDto {
  lr: number;
  reg: number;
  brier: number;
  /** `{"w0", "w1", "w2"}` —— 该 trial 训练得到的权重。 */
  weights: Record<string, number>;
}

export interface TrainFinishedEvent {
  job_id: string;
  /** "completed" | "failed" */
  status: 'completed' | 'failed' | string;
  /** 越低越好。失败时为 `null`。 */
  best_brier: number | null;
  /** 最佳 trial 的权重：`{"w0", "w1", "w2"}`。
   * 失败时为 `null`。 */
  best_params: Record<string, number> | null;
  /** 逐 trial 的统计。失败时为空。 */
  trials: TrainTrialDto[];
  duration_ms: number;
  /** 候选 JSON 的绝对路径。失败时为 `null`。 */
  candidate_path: string | null;
  /** 人类可读的错误消息。成功时为 `null`。 */
  message: string | null;
  finished_at: number;
}

/** Rust 端 `TrainResult` 的线格式镜像
 * （由 `train_job` IPC 返回）。与 finished 事件
 * 字段相同，但没有 `finished_at`（IPC 是
 * 「刚刚完成」这一事实的真相源，
 * 事件是「任何订阅者都应知道」的真相源）。 */
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

/** `train_job` IPC 的参数。所有字段均为可选 —— 
 * Python sidecar 使用合理的默认值
 * （4 trials、80 epochs、60s 超时）。 */
export interface TrainJobArgs {
  n_trials?: number;
  epochs?: number;
  timeout_ms?: number;
}

/** 在 Python sidecar 上启动模型训练 sweep。
 * v0.17a —— 包装 `commands::sidecar::train_job`。
 * 当 sweep 完成时返回完整的 `TrainResult`。 */
export const trainJob = (args: TrainJobArgs = {}) =>
  invoke<TrainResult>('train_job', { args });

/** 监听 `train_job:started` 事件。L1 通常
 * 用它在用户点击 Train 后立即渲染
 * 「Training…」小标签。 */
export const onTrainStarted = (cb: (e: TrainStartedEvent) => void): Promise<UnlistenFn> =>
  safeListen<TrainStartedEvent>('train_job:started', (msg) => cb(msg.payload));

/** 监听 `train_job:finished` 事件。L1
 * 用它来得知 sweep 已完成的终态事件。
 * 携带完整的结果（逐 trial 统计 + 最佳 Brier + 参数）。 */
export const onTrainFinished = (cb: (e: TrainFinishedEvent) => void): Promise<UnlistenFn> =>
  safeListen<TrainFinishedEvent>('train_job:finished', (msg) => cb(msg.payload));

// ---------------------------------------------------------------- Promote model (v0.18a)
// Rust 端 `PromoteResult` 的线格式镜像
// （由 `promote_model` IPC 返回）。
// v0.18a —— promote 是一个快速的文件移动
// （约 10 毫秒），没有进度事件。
//
// `promoted: true` 意味着候选已被成功重命名为
// active.json。新的模型版本位于 `model_version`。
//
// `promoted: false` 意味着 promote 被拒绝
// （没有候选、job_id 不匹配、或 sidecar 未运行）。
// `message` 字段包含人类可读的诊断信息；
// L1 将其显示在 toast 中。

export interface PromoteResult {
  promoted: boolean;
  /** "ok" | "failed" */
  status: 'ok' | 'failed' | string;
  /** 旧 active.json 的路径。首次 promote 时为 `null`。 */
  previous_path: string | null;
  /** 新 active.json 的路径（候选被重命名为该路径）。 */
  active_path: string | null;
  /** promote 的挂钟时间（毫秒）。失败时为 `null`。 */
  promoted_at_ms: number | null;
  /** 新模型版本（例如 `logistic-train-441c352b`）。失败时为空。 */
  model_version: string;
  /** 人类可读的错误消息。成功时为 `null`。 */
  message: string | null;
}

/** `promote_model` IPC 的参数。v0.18a —— `job_id`
 * 是可选的。若设置，Python sidecar 拒绝晋升
 * 来自不同 job 的候选（防止竞态）。
 *
 * v0.21a —— `trial_index` 也是可选的。若设置，
 * Python sidecar 会从 `all_trials[]` 中
 * 晋升该指定 trial（批量晋升），而非最佳 trial。
 * 0-indexed。若同时设置了 `job_id` 和 `trial_index`，
 * 两项检查都会生效。
 */
export interface PromoteModelArgs {
  job_id?: string;
  /** v0.21a —— 批量晋升。0-indexed 的 trial 编号。
   * `undefined`（或省略）表示「晋升最佳」。 */
  trial_index?: number;
}

/** 将当前候选晋升到活动槽位。v0.18a。
 * 返回完整的 `PromoteResult`。 */
export const promoteModel = (args: PromoteModelArgs = {}) =>
  invoke<PromoteResult>('promote_model', { args });

// =================================================================
// ================= v0.19b —— list_promote_history =================
// =================================================================

/** 晋升历史中的一条记录。v0.19b —— 对应 Python
 * sidecar 在 active.json 中的 `promotion_history`
 * 数组。
 *
 * 每次成功的晋升会追加一条记录。列表上限为
 * Python 端最近 20 条记录。
 */
export interface PromoteHistoryEntry {
  /** Train job_id，例如 "train-441c352b"。 */
  job_id: string;
  /** 派生的模型版本，例如 "logistic-train-441c352b"。 */
  model_version: string;
  /** 晋升时的挂钟时间（毫秒，Unix 纪元）。 */
  promoted_at_ms: number;
  /** 训练 sweep 中的最佳 Brier 分数（越低越好）。
   * 若 train 负载中未包含则为 `null`。 */
  best_brier: number | null;
  /** 最佳 trial 的超参数，若未存储则为 `null`
   * （例如 v0.18 的 active.json 文件）。 */
  best_params: Record<string, unknown> | null;
  /** v0.24a —— train sweep 中被晋升的是哪个 trial。
   * `null` 表示最佳（默认）；`number` 表示第 n 个
   * trial（批量）。v0.21a 之前的历史记录不会有
   * 此字段；UI 用途按「best」处理缺失字段。 */
  trial_index?: number | null;
  /** v0.41a —— 晋升的人类可读原因。
   * 在 ModelLab 页面的历史行 hover 时显示为 tooltip。
   * 格式：「Promoted as best trial」或
   * 「Promoted as trial N of M」。v0.41 之前的
   * 历史记录不会有此字段；缺失时按通用的
   * 「Promoted」处理。 */
  reason?: string | null;
}

/** Rust 端 `PromoteHistoryResult` 的线格式镜像
 * （由 `list_promote_history` IPC 返回）。
 * v0.19b —— 只读审计查询。无参数。 */
export interface PromoteHistoryResult {
  /** 成功读取历史时为 `true`。（即便
   * 没有条目，响应也是 ok=true 并带有
   * 提示性消息。） */
  ok: boolean;
  /** 历史条目，按时间正序排列。 */
  entries: PromoteHistoryEntry[];
  /** 方便起见的 `len(entries)`。 */
  count: number;
  /** 人类可读的消息。成功且有条目时为 `null`。 */
  message: string | null;
}

/** 查询晋升历史。v0.19b —— 无参数。
 *
 * 以列表形式返回最近 20 条晋升（在 Python
 * 端有上限）。当前激活的模型**不在**列表中
 * —— 要获取激活模型，请使用 `predict`
 * 的 `model_version` 字段。列表用于审计
 * （「某一时间哪个模型处于激活状态」），
 * 而非状态展示。
 */
export const listPromoteHistory = () =>
  invoke<PromoteHistoryResult>('list_promote_history', { args: {} });

// =================================================================
// =============== v0.33b —— list_promote_history_archive =============
// =================================================================

/** v0.33b —— `listPromoteHistoryArchive` 的参数。
 * 所有字段均为可选。归档是已超出 20 条
 * 内存 `promotion_history[]` 上限的晋升的
 * 持久长期轨迹。仅追加的 JSONL，永不自动剪裁。 */
export interface ListPromoteHistoryArchiveArgs {
  /** `promoted_at_ms` 的下界。默认 0。 */
  from_ms?: number;
  /** `promoted_at_ms` 的上界。默认 MAX。 */
  to_ms?: number;
  /** 分页偏移。默认 0。 */
  offset?: number;
  /** 分页大小。默认 100，上限 1000。 */
  limit?: number;
  /** v0.42e-3 —— 可选的 job_id 白名单。
   * 提供后仅返回 `job_id` 匹配的条目。
   * 供 `ModelComparison` 组件在不拉取
   * 整个归档的情况下，获取所选 2-3 条
   * 记录的权重。空数组 → 无条目；
   * 缺失 → 不过滤。 */
  job_ids?: string[];
}

/** v0.33b —— Python sidecar 的 archive.jsonl
 * 的线格式镜像。每条记录是一次归档的晋升
 * （一条超出 20 条上限的晋升）。
 * 字段对应 `PromoteHistoryEntry`，外加 `archived_at_ms`。 */
export interface PromoteHistoryArchiveEntry {
  job_id: string;
  model_version: string;
  promoted_at_ms: number;
  best_brier: number | null;
  best_params: Record<string, number> | null;
  weights: { w0: number; w1: number; w2: number } | null;
  trial_index: number | null;
  /** v0.33b —— 该记录被写入归档文件的时间。
   * 如果 sidecar 当时离线、记录是稍后写入的，
   * 可能与 `promoted_at_ms` 不同。 */
  archived_at_ms: number;
}

/** v0.33b —— `listPromoteHistoryArchive` 的响应。 */
export interface PromoteHistoryArchiveResult {
  /** 归档读取成功时为 `true`。 */
  ok: boolean;
  /** 经过滤（并分页后）匹配的条目。 */
  entries: PromoteHistoryArchiveEntry[];
  /** 文件中（分页前）的总条目数。 */
  total: number;
  /** 可选消息（错误或「尚无归档」）。 */
  message: string | null;
}

/** v0.33b —— 查询归档的晋升历史。
 *
 * Python sidecar 的 `archive.jsonl` 文件包含
 * 所有超出 20 条内存 `promotion_history[]`
 * 上限的晋升。L1 可以使用此 IPC 在 ModelLab 页面
 * 显示「查看归档」面板，使用户能够审计
 * 最近 20 条之外的晋升。
 *
 * 文件位于 `~/.polyrocket/sidecar/models/archive.jsonl`
 * （可通过 `POLYROCKET_SIDECAR_MODEL_DIR` 覆盖）。
 */
export const listPromoteHistoryArchive = (
  args: ListPromoteHistoryArchiveArgs = {},
) =>
  invoke<PromoteHistoryArchiveResult>(
    'list_promote_history_archive',
    { args },
  );

// =================================================================
// ==================== v0.20b —— rollback_model =====================
// =================================================================

/** `rollback_model` IPC 的参数。v0.20b。
 *
 * `model_version` 是要回滚到的版本，
 * 例如 "logistic-train-441c352b"。L1 应
 * 将从历史面板获得的值原样回传
 * （精确匹配）。版本必须是带有 `weights`
 * 存储的历史记录（v0.20a+）；v0.19 中没有
 * 权重的条目会被拒绝并给出明确的错误。
 */
export interface RollbackModelArgs {
  model_version: string;
}

/** Rust 端 `RollbackResult` 的线格式镜像
 * （由 `rollback_model` IPC 返回）。
 * v0.20b —— `rolled_back: bool` 是主要成功指标。
 *
 * 成功时，active.json 已用目标版本的权重
 * 重写。失败时，激活模型保持不变，`message`
 * 字段包含人类可读的诊断信息。
 */
export interface RollbackResult {
  rolled_back: boolean;
  /** "ok" | "failed" */
  status: 'ok' | 'failed' | string;
  /** 旧 active.json 的路径（始终为同一路径；
   * rollback 是对同一文件的写入）。 */
  previous_path: string | null;
  /** 新 active.json 的路径（与 previous_path 相同）。 */
  active_path: string | null;
  /** rollback 的挂钟时间（毫秒）。失败时为 `null`。 */
  rolled_back_at_ms: number | null;
  /** 回滚到的版本。失败时为空。 */
  model_version: string;
  /** 人类可读的错误消息。成功时为 `null`。 */
  message: string | null;
}

/** 将激活模型回滚到之前的版本。
 * v0.20b。按 `model_version` 在
 * `promotion_history` 中查找条目，并将其
 * 权重恢复为新的激活模型。
 *
 * 失败模式（均返回 `rolled_back: false`）：
 * - active.json 缺失 → 「no active model」
 * - model_version 不在历史中 → 「not found」
 * - 条目来自 v0.19（无权重）→ 「no weights」
 */
export const rollbackModel = (args: RollbackModelArgs) =>
  invoke<RollbackResult>('rollback_model', { args });

// =================================================================
// ================== v0.23b —— auto_promote_if_better ===============
// =================================================================

/** `auto_promote_if_better` IPC 的参数。v0.23b。
 *
 * `brier_margin` 表示候选模型必须比
 * 当前模型好多少（Brier 越低越好），
 * 才能触发 auto-promote。默认 0.005。
 *
 * `trial_index` 是要使用的 trial（None 表示最佳）。
 */
export interface AutoPromoteIfBetterArgs {
  /** 候选必须改进多少。默认 0.005。 */
  brier_margin?: number;
  /** 使用哪个 trial（None = 最佳，0..n-1 表示特定 trial）。 */
  trial_index?: number;
}

/** Rust 端 `AutoPromoteIfBetterResult` 的线格式镜像
 * （由 `auto_promote_if_better` IPC 返回）。v0.23b。
 *
 * `promoted` 和 `skipped` 互斥：
 *   - promoted=true, skipped=false：候选明显更好，
 *     已被晋升
 *   - promoted=false, skipped=true：候选**没有**
 *     明显更好，不采取任何操作
 *   - promoted=false, skipped=false：错误情况
 *     （例如无候选、margin 无效）
 */
export interface AutoPromoteIfBetterResult {
  promoted: boolean;
  skipped: boolean;
  /** 人类可读的原因。 */
  reason: string;
  candidate_brier: number | null;
  active_brier: number | null;
  /** 用于比较的 brier_margin。 */
  margin: number;
  model_version: string | null;
  promoted_at_ms: number | null;
  message: string | null;
}

/** 仅当候选显著优于激活模型时才晋升。
 * v0.23b。
 *
 * 一键操作：每次训练后点击，sidecar 比较
 * Brier 分数，要么晋升候选
 * （至少比 `brier_margin` 更好），要么
 * no-op（并给出明确的「skipped」原因）。
 *
 * 如果还没有激活模型，候选会被自动晋升
 * （无法比较，按定义就是最佳）。
 */
export const autoPromoteIfBetter = (args: AutoPromoteIfBetterArgs = {}) =>
  invoke<AutoPromoteIfBetterResult>('auto_promote_if_better', { args });

// =================================================================
// =================== v0.25b —— promote_all_trials ==================
// =================================================================

/** `results` 列表中单个逐 trial 结果的
 * 线格式镜像。v0.25b。 */
export interface PromoteAllTrialResult {
  /** 0-indexed 的 trial 编号。 */
  trial_index: number;
  /** 若该 trial 已被成功晋升则为 `true`。 */
  promoted: boolean;
  /** "ok" | "failed" —— 对应 Python 端每次调用的状态。 */
  status: 'ok' | 'failed' | string;
  /** 新的模型版本（例如 "logistic-train-XYZ-t2"）。 */
  model_version: string;
  /** 晋升时的挂钟时间（毫秒）。失败时为 `null`。 */
  promoted_at_ms: number | null;
  /** 人类可读的错误消息。成功时为 `null`。 */
  message: string | null;
}

/** Rust 端 `PromoteAllTrialsResult` 的线格式镜像
 * （由 `promote_all_trials` IPC 返回）。v0.25b。 */
export interface PromoteAllTrialsResult {
  /** 所有 trial 的晋升都成功时为 `true`。 */
  ok: boolean;
  /** 逐 trial 结果，按 trial_index 排序。 */
  results: PromoteAllTrialResult[];
  /** `len(results)`。 */
  count: number;
  /** 整体错误消息（例如「no candidate」）。
   * 所有晋升成功时为 `null`。 */
  message: string | null;
}

/** 批量晋升当前候选中的每个 trial。v0.25b。
 *
 * 用于 A/B 比较：用户可以看到所有 4 个 trial
 * 在真实市场上的表现，然后通过 v0.20c 的
 * Rollback 按钮回滚到胜出者。如果没有这个
 * 功能，用户需要点击「Promote」4 次。
 *
 * 调用后所有 4 个 trial 都会出现在历史面板中，
 * 每个都有自己 `-tN` 后缀的模型版本以及
 * `trial_index` 字段。当前激活的模型是
 * 最后一个被晋升的（trial 3），但用户可以
 * 回滚到其中任何一个。
 */
export const promoteAllTrials = () =>
  invoke<PromoteAllTrialsResult>('promote_all_trials', { args: {} });

// =================================================================
// ==================== v0.43 —— backtest_model ====================
// =================================================================

/** v0.43 —— 单个回测样本。L1 从 markets DB
 * （仅限已结算市场）构建此列表并传递。 */
export interface BacktestSample {
  /** 预测时的市场价格（0..1）。 */
  price: number;
  /** 预测时距离市场开盘的时长（小时）。
   * 模型的 `w2` 权重即 age 权重。 */
  market_age_hours: number;
  /** 结算结果（0 = NO，1 = YES）。 */
  outcome: number;
  /** 可选的人类可读标签（例如市场问题）。
   * 在 top winners/losers 列表中展示。空字符串也可。 */
  label?: string;
}

/** v0.43 —— calibration 直方图中的一条记录。
 * L1 将其渲染为小型条形图：「在此预测桶中，
 * 实际结算率为 X（与预测的 Y 对比）」。 */
export interface BacktestCalibrationBucket {
  /** 人类可读的桶标签，例如 "[0.4, 0.6)"。 */
  bucket: string;
  /** 此桶中的平均预测概率。空桶时为 `null`。 */
  predicted_avg: number | null;
  /** 此桶中的实际结算率。空桶时为 `null`。 */
  actual_rate: number | null;
  /** 此桶中的样本数。 */
  count: number;
}

/** v0.43 —— top winners / top losers 列表
 * 中的一条记录。包含足够上下文以渲染
 * 悬浮提示（hover tooltip）。 */
export interface BacktestTopSample {
  /** 从输入 `label` 回显。 */
  label: string;
  /** 单样本 Brier。 */
  brier: number;
  /** 模型的预测。 */
  predicted: number;
  /** 实际结果。 */
  outcome: number;
}

/** v0.43 —— Python sidecar 的 `backtest_model`
 * 响应的线格式镜像。 */
export interface BacktestResult {
  /** 成功时为 `true`；未知模型 / 空样本 /
   * 全部格式错误 / 权重缺失时为 `false`。 */
  ok: boolean;
  /** 从请求回显。 */
  model_version: string;
  /** 通过校验并对 Brier 有贡献的样本数。 */
  sample_count: number;
  /** 预测与结果之间的均方误差。
   * `ok=false` 时为 `null`。 */
  brier_mean: number | null;
  /** 逐样本 Brier 分数，按输入顺序排列。
   * 用于客户端直方图。 */
  brier_breakdown: number[];
  /** [0, 1] 中的 5 个 calibration 桶。 */
  calibration: BacktestCalibrationBucket[];
  /** 3 个 Brier 最低的样本（最佳预测）。 */
  top_winners: BacktestTopSample[];
  /** 3 个 Brier 最高的样本（最差预测），
   * 已反转（最差在前）。 */
  top_losers: BacktestTopSample[];
  /** 人类可读的状态 / 错误消息。 */
  message: string | null;
}

/** v0.43 —— `backtestModel` IPC 的参数。 */
export interface BacktestModelArgs {
  /** 要回测的模型，例如
   * "logistic-train-441c352b"。
   * 先在 `archive.jsonl` 中查找，
   * 然后在 `active.json` 中查找。 */
  model_version: string;
  /** 用于重放模型的 (price, market_age_hours,
   * outcome) 样本列表。 */
  samples: BacktestSample[];
}

/** v0.43 —— 重放一个已保存的模型，针对一系列
 * (price, market_age_hours, outcome) 样本，
 * 并返回 Brier + calibration + 逐样本
 * 预测。补上 v0.17–v0.41 的模型生命周期
 * 空白：在没有这个 IPC 的情况下无法
 * 回答「该模型在真实结算上表现会如何」。
 *
 * sidecar 是纯函数（除读取模型文件外
 * 无 IO），因此每次调用的成本是 O(samples)
 * —— 数百个样本很快，数百万个样本
 * 很慢。L1 应预先过滤到一个合理的
 * 时间窗口内。
 */
export const backtestModel = (args: BacktestModelArgs) =>
  invoke<BacktestResult>('backtest_model', { args });

// =================================================================
// ==================== v0.28a —— auto_promote_config =================
// =================================================================

/** v0.28a —— `setAutoPromoteConfig` 的参数。
 * 两个字段都是可选的：`undefined` 表示
 * 「保持不变」，以便 L1 仅更新用户在 UI 中
 * 更改的字段（例如仅 toggle，不含 margin）。 */
export interface SetAutoPromoteConfigArgs {
  enabled?: boolean;
  brier_margin?: number;
}

/** v0.28a —— Rust 端 `AutoPromoteConfigDto`
 * 的线格式镜像（由 `get` 和 `set` IPC
 * 返回）。Rust 的 `AppState` 保存当前值；
 * L1 在 Settings 页面挂载时通过
 * `setAutoPromoteConfig` 推送它们。 */
export interface AutoPromoteConfigDto {
  enabled: boolean;
  brier_margin: number;
}

/** v0.28a —— 将用户的 auto-promote 设置
 * 推送到 Rust。调用此 IPC 后，`train_job`
 * 的 Rust 处理函数将读取这些值，并在
 * `enabled === true` 且训练成功时启动
 * auto-promote worker。
 *
 * 返回新的合并后配置（以便 L1 确认
 * Rust 当前的设置）。
 *
 * 在 `Settings.tsx` 挂载时调用此 IPC 以使
 * 这些值在重载后保持一致。L1 的 zustand
 * store（prefs-store）是 UI 的真相源；
 * Rust 是 `train_job` 的消费者。
 */
export const setAutoPromoteConfig = (args: SetAutoPromoteConfigArgs = {}) =>
  invoke<AutoPromoteConfigDto>('set_auto_promote_config', { args });

/** v0.28a —— 从 Rust 读取当前的 auto-promote
 * 配置。如果 L1 从未推送过任何配置，
 * 则返回默认值。 */
export const getAutoPromoteConfig = () =>
  invoke<AutoPromoteConfigDto>('get_auto_promote_config');

// =================================================================
// ================== v0.42c —— telemetry 开关 =================
// =================================================================

/** v0.42c —— `setTelemetryEnabled` 的参数。
 * L1 从 Settings 中的 telemetry 开关推送此值。 */
export interface SetTelemetryEnabledArgs {
  enabled: boolean;
}

/** v0.42c —— telemetry gate 的运行时覆盖。
 *
 * 默认为进程启动时环境变量
 * `POLYROCKET_TELEMETRY` 的值（通常为
 * `false`）。此调用之后，在进程生命
 * 周期内环境变量被忽略 —— 用户的
 * 选择生效。
 *
 * 开启时，每个 Rust 生命周期事件
 * （train started / completed / failed、
 * promote completed、scheduler tick 等）
 * 会向 stderr 写入一行 NDJSON。可通过
 * 以下方式捕获：
 *
 *     polyrocket 2> telemetry.log
 *
 * 关闭时，`emit()` 是 no-op（原子加载，
 * 零开销）。
 */
export const setTelemetryEnabled = (args: SetTelemetryEnabledArgs) =>
  invoke<boolean>('set_telemetry_enabled', { args });

/** v0.42c —— 读取当前 telemetry 状态。
 * 返回当前有效值（启动时的环境变量，
 * 除非 L1 已覆盖）。 */
export const getTelemetryEnabled = () =>
  invoke<boolean>('get_telemetry_enabled');

// =================================================================
// ================== v0.49a —— telemetry 日志文件保留 =============
// =================================================================

/** v0.49a —— 磁盘上 telemetry 日志清单中的一行。
 * 由 `listTelemetryLogs` 返回。
 * 当前进程的活跃会话文件标记为
 * `isCurrent = true`。L1 TelemetryCard
 * 使用它来显示「本会话」/「较旧会话」
 * 及其大小。 */
export interface TelemetryLogInfo {
  /** 仅文件名，例如 `session-1740000000.jsonl`。 */
  name: string;
  /** 磁盘上的绝对路径。L1 不直接打开
   * 此文件（Rust 是 IO 所有者）；路径
   * 仅供 UI 展示参考。 */
  path: string;
  /** 文件大小（字节）。 */
  sizeBytes: number;
  /** 文件修改时间（unix 秒）。 */
  modifiedUnix: number;
  /** 若这是当前进程正在追加的活跃
   * 会话文件则为 true。 */
  isCurrent: boolean;
}

/** v0.49a —— 列出当前日志目录中磁盘上的
 * telemetry 会话文件。当前进程的文件
 * 标记为 `isCurrent = true`。尚未设置
 * 日志目录时（例如启动初始化未完成）
 * 返回空列表。 */
export const listTelemetryLogs = () =>
  invoke<TelemetryLogInfo[]>('list_telemetry_logs');

/** v0.49a —— 手动触发一次保留扫描。
 * 删除早于 `POLYROCKET_TELEMETRY_RETENTION_DAYS`
 * （默认 14）的会话文件。
 * 返回被删除的文件数。相同的扫描
 * 会在启动时自动运行。 */
export const purgeTelemetryLogs = () =>
  invoke<number>('purge_telemetry_logs');

// =================================================================
// ================== v0.49b —— active model ========================
// =================================================================

/** v0.49b —— 「当前哪个模型处于激活状态
 * 以及它的训练指标是什么」的单一真相源。
 * L1 始终通过此 IPC 读取激活模型，
 * 而不是复制 active.json 文件路径逻辑。
 *
 * 当还没有任何模型被晋升时
 * （典型的首次运行状态）返回 `null`。
 *
 * `weights` 字段保留给 v0.50+，
 * 届时 sidecar 将开始把训练后的权重
 * 写入 active.json。目前它始终为 `null`。 */
export interface ActiveModel {
  /** 例如 "logistic-train-441c352b" */
  modelVersion: string;
  /** 训练时的 Brier 分数。缺失时为 `null`。 */
  bestBrier: number | null;
  /** 最佳 trial 的超参数。 */
  bestParams: Record<string, unknown> | null;
  /** 上一次晋升的挂钟时间（毫秒）。 */
  promotedAtMs: number | null;
  /** v0.50+ —— 目前始终为 `null`。 */
  weights: number[] | null;
  /** active.json 的绝对路径（供 UI
   * 向高级用户展示「数据位于 ...」）。 */
  sourcePath: string;
}

/** v0.49b —— 从磁盘读取激活模型。
 * 当还没有任何模型被晋升时返回 `null`
 * （active.json 缺失）。当 active.json
 * 存在但格式错误时返回错误。 */
export const getActiveModel = () =>
  invoke<ActiveModel | null>('get_active_model');

// =================================================================
// ================== v0.49c —— scheduler self-test =================
// =================================================================

/** v0.49c —— 来自 scheduler self-test 的
 * 单个 loop 状态。Rust 端将每个 loop
 * 最近一次 tick 的 unix-ms 捕获到进程
 * 全局原子中；L1 轮询此快照以渲染
 * 一行绿/红点。 */
export interface SchedulerLoopStatus {
  /** 稳定的 loop 名称，例如 "health_probe"。
   * 对应 `infra::scheduler::record_tick`
   * 中的变体名。 */
  name: string;
  /** 最近一次 tick 的 Unix-ms。0 表示
   * 从未 tick（仍处于初始错峰休眠）。 */
  lastTickUnixMs: number;
  /** 距上次 tick 的毫秒数。
   * 若该 loop 从未 tick 则为 `null`。 */
  ageMs: number | null;
  /** 当 `ageMs <= 3 * expected_interval_ms` 时为 true。 */
  healthy: boolean;
}

/** v0.49c —— scheduler self-test 快照。
 * 由 `schedulerSelfTestNow` 返回。
 * 调用成本低（仅读取原子计数器，无 IO）。 */
export interface SchedulerSelfTest {
  /** 进程启动时的 Unix 秒。 */
  processStartedAtUnix: number;
  /** self-test 运行的 Unix-ms。 */
  checkedAtUnixMs: number;
  /** 当所有 loop 都健康时为 true。 */
  allHealthy: boolean;
  /** 逐 loop 状态，按字母顺序排列。 */
  loops: SchedulerLoopStatus[];
}

/** v0.49c —— 返回 8 个后台 scheduler loop
 * 的活跃性快照。L1 Settings 卡片将
 * 其渲染为一行绿/红点。 */
export const schedulerSelfTestNow = () =>
  invoke<SchedulerSelfTest>('scheduler_self_test_now');

// =================================================================
// ================== v0.50c —— fill analytics ======================
// =================================================================

/** v0.50c —— fill_analytics 的 order-type
 * 细分中的一个桶。 */
export interface OrderTypeBucket {
  orderType: 'market' | 'limit' | 'stop_loss';
  count: number;
  settled: number;
  won: number;
  /** 该桶中已结算行的已实现 PnL
   * （USDC）。正值表示盈利。 */
  realizedPnlUsdc: number;
}

/** v0.50c + v0.51b —— fill analytics 汇总，
 * 由 `fillAnalytics` 返回。聚合 `bets` 表。
 *
 * v0.51b 增加滑点、成交流转时间、部分
 * 成交流转等指标。对于 v0.5d 的确定性
 * stub 来说，这些是 0 / 0ms / 0
 * （因为 fill_price = price 且
 * filled_at = placed_at 天然相等）。
 * v0.51+ 接入了真实的 CLOB，这些将
 * 反映实际执行情况。 */
export interface FillAnalytics {
  totalFills: number;
  openCount: number;
  wonCount: number;
  lostCount: number;
  cancelledCount: number;
  /** 平均 (settled_at - placed_at)，毫秒。
   * 当没有已结算的 bet 时为 `null`。 */
  avgTimeToSettlementMs: number | null;
  /** won / (won + lost + cancelled)。当
   * 没有已结算时为 0.0。 */
  winRate: number;
  /** 已结算行的已实现 PnL（USDC）。 */
  realizedPnlUsdc: string;
  /** 逐 order-type 的细分（始终存在
   * 3 个桶，即使为空）。 */
  byOrderType: OrderTypeBucket[];
  /** post_only 的成交数。 */
  postOnlyCount: number;
  /** postOnlyCount / totalFills。当没有成交时为 0.0。 */
  postOnlyRate: number;
  /** v0.51b —— 已成交行的平均
   * |fill_price - price|。当没有行带有
   * fill_price 时为 `null`（v0.51b 之前的 DB）。 */
  avgSlippage: number | null;
  /** v0.51b —— 平均 (filled_at - placed_at)，
   * 毫秒。当没有行带有 filled_at 时为 `null`。 */
  avgTimeToFillMs: number | null;
  /** v0.51b —— 部分成交数。 */
  partialFillCount: number;
  /** v0.51b —— partialFillCount / totalFills。当为 0 时为 0.0。 */
  partialFillRate: number;
}

/** v0.50c —— 返回 fill analytics 汇总。 */
export const fillAnalytics = () =>
  invoke<FillAnalytics>('fill_analytics');

// =================================================================
// ================== v0.51a —— CLOB snapshots =====================
// =================================================================

/** v0.51a —— 一个市场的完整订单簿快照。
 * `bids` 按价格降序排列
 * （最优买价在前）；`asks` 按价格升序
 * （最优卖价在前）。每个元组是
 * `[price, size]`。 */
export interface ClobSnapshot {
  marketId: string;
  capturedAt: number;
  /** `[price, size]` 元组，按价格降序。 */
  bids: Array<[number, number]>;
  /** `[price, size]` 元组，按价格升序。 */
  asks: Array<[number, number]>;
}

/** v0.51a —— 当前 CLOB feed 状态。
 *
 * 当环境变量 `POLYROCKET_CLOB_API_KEY` /
 * `POLYROCKET_CLOB_API_SECRET` /
 * `POLYROCKET_CLOB_API_PASSPHRASE` 没有全部
 * 设置时返回 "not_configured"。
 *
 * 当凭据已设置但实际的 WebSocket 监听器
 * 尚未接入时返回 "configured"（v0.51+）。
 *
 * 当状态为 "not_configured" 时，L1 回退到
 * v0.47a 的 `price_snapshots`。 */
export interface ClobFeedStatus {
  state: 'not_configured' | 'configured' | 'connected';
  /** 表中不同的 (market_id, captured_at)
   * 对数。每个「snapshot」只计一次，
   * 即使它包含多行（每个价位一行）。 */
  totalSnapshots: number;
  /** 至少有一个 snapshot 的不同市场数。 */
  marketsWithSnapshots: number;
}

/** v0.51a —— return the current CLOB feed status. */
export const clobFeedStatus = () =>
  invoke<ClobFeedStatus>('clob_feed_status');

/** v0.51a —— `recordClobSnapshotNow` 的参数。
 * Bids 和 asks 是 `[price, size]` 元组。 */
export interface RecordClobSnapshotArgs {
  marketId: string;
  capturedAt: number;
  bids: Array<[number, number]>;
  asks: Array<[number, number]>;
}

/** v0.51a —— 一次性手动插入。记录一个
 * snapshot 并返回行数
 * （bids.length + asks.length）。 */
export const recordClobSnapshotNow = (args: RecordClobSnapshotArgs) =>
  invoke<number>('record_clob_snapshot_now', { args });

/** v0.51a —— 返回某个市场最近的 snapshot。
 * 当该市场没有任何 snapshot 时返回 `null`。 */
export const latestClobSnapshot = (marketId: string) =>
  invoke<ClobSnapshot | null>('latest_clob_snapshot', { marketId });

// =================================================================
// ================== v0.53a —— storage path ========================
// =================================================================

/** v0.53a —— 当前存储路径信息。
 * 由 `getStorageInfo` 返回。L1 用它来渲染
 * 「Welcome > Storage path」步骤以及
 * Settings → Storage 卡片。 */
export interface StorageInfo {
  /** OS 推荐的路径（Tauri 的 app_data_dir）。 */
  defaultPath: string;
  /** polyrocket 在下次启动时将使用的路径。
   * 用户没有自定义时等于 `defaultPath`。 */
  currentPath: string;
  /** 当 currentPath !== defaultPath 时为 true。 */
  isCustom: boolean;
  /** 当 currentPath 在磁盘上存在时为 true。 */
  exists: boolean;
  /** 当前用户对 currentPath 可写时为 true。 */
  writable: boolean;
  /** 剩余空间字节数。当 OS 查询失败时
   * （沙箱 / 特殊文件系统）为 `null`。
   * L1 在此字段为 null 时隐藏该指标。 */
  freeBytes: number | null;
  /** 当用户已选择自定义路径但当前进程仍在
   * 使用默认路径（即用户需要重启）时为 true。
   * L1 在此字段为 true 时显示「立即重启」按钮。 */
  restartRequired: boolean;
}

/** v0.53a —— 返回当前存储路径信息。
 * 开销很小（只是 std::fs metadata + 设置查询）。 */
export const getStorageInfo = () =>
  invoke<StorageInfo>('get_storage_info');

/** v0.53a —— 写入自定义存储路径。更改
 * 在下次启动时生效（当前进程的 DB 已经打开）。
 *
 * 在以下情况抛 AppError::Invalid：路径不是
 * 绝对路径、不存在、不是目录、或不可写。
 * L1 必须在调用此 IPC 后再次调用 `getStorageInfo`
 * 以刷新 UI；`restartRequired` 标志将为 true。 */
export const setStoragePath = (path: string) =>
  invoke<void>('set_storage_path', { args: { path } });

/** v0.53a —— 清除自定义存储路径。
 * 下次启动回退到 OS 默认路径。 */
export const resetStoragePath = () =>
  invoke<void>('reset_storage_path');

// =================================================================
// ================== v0.56 —— 网络代理 / Tor =================
// =================================================================

/** v0.56 —— 用户视角的代理配置。
 * `enabled` 是一个独立的布尔值，这样
 * 用户可以不清空 URL 就禁用代理（更快的开关）。 */
export interface ProxyConfig {
  enabled: boolean;
  url: string | null;
  scheme: 'http' | 'socks5' | null;
  restartRequired: boolean;
}

/** v0.56 —— 返回当前代理配置。
 * L1 用它来渲染 Settings → Network 卡片。 */
export const getProxyConfig = () =>
  invoke<ProxyConfig>('get_proxy_config');

/** v0.56 —— 写入代理配置。更改在下次启动
 * 生效（当前会话的 HTTP 客户端已构建完毕）。
 * L1 在此调用返回后显示「需要重启」的横幅。 */
export const setProxyConfig = (
  enabled: boolean,
  url: string | null,
): Promise<ProxyConfig> =>
  invoke<ProxyConfig>('set_proxy_config', {
    args: { enabled, url: url ?? null },
  });

/** v0.56 —— 完全清除代理（URL + enabled 标志）。
 * 下次启动使用直接的出站 HTTP。 */
export const clearProxyConfig = () =>
  invoke<void>('clear_proxy_config');

/** v0.56 —— 从 `network_proxy.json` 文件
 * 读取代理 URL（启动时的真相源）。供 L1
 * 的「需要重启」提示和健全性检查使用。 */
export const readProxyConfigFile = () =>
  invoke<string | null>('read_proxy_config_file');

// =================================================================
// ================== v0.55 —— 模型可解释性 =====================
// =================================================================

/** v0.55 —— 可解释性查询的输入样本。
 * 两个字段都是可选的；都省略时使用默认
 * 样本（price=0.5, age=24h）。 */
export interface ExplainSample {
  /** 市场价格，0..1。 */
  price?: number;
  /** 市场年龄（小时），>=0。 */
  market_age_hours?: number;
}

/** v0.55 —— 单个特征对单次预测的贡献。
 * L1 将其渲染为水平条形图。 */
export interface ExplainFeature {
  feature: string;
  value: number;
  weight: number;
  /** 对 (p - 0.5) 的贡献。正值表示
   * 「使预测更高」，负值表示「使预测更低」。 */
  contribution: number;
  abs_contribution: number;
}

/** v0.55 —— `explainModel` 的结果。
 * `features` 数组按 `abs_contribution`
 * 降序排列。 */
export interface ExplainResult {
  ok: boolean;
  model_version: string;
  features: ExplainFeature[];
  prediction: number | null;
  sample: { price: number; market_age_hours: number } | null;
  message: string;
}

/** v0.55 —— `explainModel` 的参数。
 * `sample` 是可选的；省略时 sidecar 使用
 * 默认样本，使用户能看到「对于典型市场
 * 模型会怎么说」的视图。 */
export const explainModel = (
  model_version: string,
  sample?: ExplainSample,
): Promise<ExplainResult> =>
  invoke<ExplainResult>('explain_model', {
    args: { model_version, sample: sample ?? null },
  });

// =================================================================
// ================== v0.59 —— KernelSHAP ===========================
// =================================================================

/** v0.59 —— 一个特征的 SHAP 值。形状与 ExplainFeature
 * （v0.55）相同，但使用 `shap_value` / `abs_shap`
 * 而不是 `contribution` / `abs_contribution`，
 * 以便在 L1 中显式表达 SHAP 的数学含义。 */
export interface ShapFeature {
  feature: string;
  value: number;
  weight: number;
  /** SHAP 值 φ_i。正值表示「使预测更高」，
   * 负值表示「使预测更低」。满足：
   *   Σφ_i = f(x) - E[f(x)]
   * （SHAP 的效率公理）。 */
  shap_value: number;
  /** |shap_value|。用于排序和图表条长度。 */
  abs_shap: number;
}

/** v0.59 —— `shapExplain` 的结果。包含
 * `baseline_prediction`（空联盟值）和
 * `target_prediction`（全联盟值），以便 L1
 * 显示「SHAP 值之和 = f(x) - E[f(x)]」的提示。
 * `efficiency_diff` 是拟合后的残差；
 * 应在浮点容差范围内近似为 0。 */
export interface ShapResult {
  ok: boolean;
  model_version: string;
  /** 目前固定为 "kernel_shap"。未来的
   * 变体（例如基于树模型的 "tree_shap"）
   * 可以设置为不同的值。 */
  method: string;
  features: ShapFeature[];
  baseline_prediction: number | null;
  target_prediction: number | null;
  efficiency_diff: number | null;
  sample: { price: number; market_age_hours: number } | null;
  message: string;
}

/** v0.59 —— 通过 sidecar 的 KernelExplainer
 * 计算真正的 SHAP 值。对于 3 特征的 polyrocket
 * 模型，开销是 8 次联盟评估（约 100µs）；
 * 对于 M > 5 的树模型则需要 TreeSHAP。
 * v0.59 候选方案。 */
export const shapExplain = (
  model_version: string,
  sample?: ExplainSample,
): Promise<ShapResult> =>
  invoke<ShapResult>('shap_explain', {
    args: { model_version, sample: sample ?? null },
  });

// =================================================================
// ================== v0.54b —— storage migration ===================
// =================================================================

/** v0.54b —— `migrateStoragePath` 的结果。
 * L1 使用 `noop` 来跳过「数据已迁移」的
 * 提示（例如源目录没有可拷贝内容，
 * 比如在新路径上的全新安装）。 */
export interface MigrateStoragePathResult {
  from: string;
  to: string;
  filesCopied: number;
  bytesCopied: number;
  overwritten: boolean;
  noop: boolean;
}

/** v0.54b —— 从源路径（当前激活路径）拷贝
 * `polyrocket.db` + `logs/` 到 `dest`。
 * `dest` 必须是用户刚刚通过 `setStoragePath`
 * 设置的路径（或者在用户调用
 * `resetStoragePath` 时的 OS 默认路径）。
 *
 * 幂等性：以相同参数重复调用是 no-op
 * （返回 `noop: true`）。
 *
 * 在以下情况抛 AppError::Invalid：
 *   - dest 不是绝对路径 / 不存在 / 不是
 *     目录 / 不可写
 *   - dest 非空且 overwrite=false
 *
 * 对于较大的 DB，这可能需要数秒时间。
 * L1 应当在后台 mutation 中调用，以免
 * UI 卡顿。 */
export const migrateStoragePath = (
  dest: string,
  overwrite = false,
): Promise<MigrateStoragePathResult> =>
  invoke<MigrateStoragePathResult>('migrate_storage_path', {
    args: { dest, overwrite },
  });

// =================================================================
// ================== v0.54a —— tauri-plugin-dialog =================
// =================================================================

/** v0.54a —— 打开原生目录选择器。返回用户
 * 选择的绝对路径，如果用户取消则返回 `null`。
 * 封装了 `dialog:open` 插件命令 —— 由
 * Welcome → Storage 步骤中的「浏览...」按钮
 * 使用，避免用户手动输入完整路径。 */
export const pickDirectory = (): Promise<string | null> =>
  invoke<string | null>('pick_directory');

/** v0.54a —— 打开原生文件选择器。供 LLM 管理页
 * 从 `.env` 格式的文件导入 API key，以及钱包
 * 管理器导入钱包 JSON 时使用。`filters` 可选；
 * 省略时用户能看到所有文件。返回用户选择的
 * 绝对路径，如果用户取消则返回 `null`。 */
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
// ================== v0.51c —— CLOB submit ========================
// =================================================================

/** v0.51c —— CLOB 下单尝试的结果。
 * 对应 `domain::polymarket::ClobOrderResult`。
 * L1 从 `placeSignedOrder` 响应的 audit-log
 * 负载中读取此结果（audit_log 中按 v0.51c 记录
 * 了 via_http / fill_price / fill_size / partial
 * 字段）。对于 L1 的下单表单（v0.52+），完整的
 * 下单响应通过 BetDto 的 filled_at / fill_price /
 * fill_size / partial 字段体现。 */
export interface ClobOrderResult {
  ok: boolean;
  txHash: string;
  filledAtMs: number;
  fillPrice: number;
  fillSize: string;
  partial: boolean;
  error: string;
  /** 当调用通过 HTTP 路径完成（凭据已配置
   * 且可达）时为 true。使用确定性 stub 时为 false。 */
  viaHttp: boolean;
}

// =================================================================
// ================== v0.28a —— auto_promote:finished =================
// =================================================================

/** v0.28a —— `auto_promote:finished` 事件的负载，
 * 由 `train_job` 派生的后台 worker 发出
 * （仅在 auto-promote 启用时）。
 *
 * L1 在 ModelLab 页面上监听此事件以自动
 * 刷新页面（并可选择性地显示 toast）。
 */
export interface AutoPromoteFinishedEvent {
  /** 触发 auto-promote 的 train 的 `job_id`。
   * L1 可借此将事件与刚刚启动的训练关联。 */
  job_id: string;
  /** 若候选模型实际被晋升则为 `true`。 */
  promoted: boolean;
  /** 来自 sidecar 的可读状态。例如
   * 「auto-promoted: improvement 0.012 > margin 0.005」，
   * 或「sidecar not running」，
   * 或「auto_promote parse: …」。 */
  message: string;
  /** 晋升后的新模型版本（如有）。 */
  model_version: string | null;
  /** auto-promote 完成的时间（unix 毫秒）。 */
  finished_at: number;
}

/** v0.28a —— 监听 `auto_promote:finished` 事件。
 * ModelLab 页面借此在后台 auto-promote 完成时
 * 自动刷新 PromoteHistory 面板和 Brier 图表。 */
export const onAutoPromoteFinished = (
  cb: (e: AutoPromoteFinishedEvent) => void,
): Promise<UnlistenFn> =>
  safeListen<AutoPromoteFinishedEvent>('auto_promote:finished', (msg) => cb(msg.payload));

// ---------------------------------------------------------------- v0.126 —— 竞争分析

// P0-1：Smart Money Score
export interface SmartMoneyScore {
  market_id: string;
  yes_score: number;
  no_score: number;
  yes_breakdown: SideBreakdown;
  no_breakdown: SideBreakdown;
  computed_at: number;
}
export interface SideBreakdown {
  wallet_count: number;
  avg_pnl: number;
  win_rate: number;
  median_position: number;
  top_wallets: TopWallet[];
}
export interface TopWallet {
  address: string;
  pnl: number;
  win_rate: number;
}
export const smartMoneyScore = (marketId: string) =>
  safeInvoke<SmartMoneyScore>('smart_money_score', { marketId });

// P0-2：Market Calendar
export interface CalendarDay {
  date: string;
  fixtures: CalendarFixture[];
}
export interface CalendarFixture {
  market_id: string;
  home: string;
  away: string;
  time: string;
  edge: number | null;
  competition: string | null;
}
export const marketCalendar = (year: number, month: number) =>
  safeInvoke<CalendarDay[]>('market_calendar', { year, month });

// P0-3：Spike Detection
export interface SpikeAlert {
  id: number;
  market_id: string;
  old_price: number;
  new_price: number;
  change_pct: number;
  detected_at: number;
  market_question: string | null;
}
export const listSpikeAlerts = (limit?: number) =>
  safeInvoke<SpikeAlert[]>('list_spike_alerts', { limit: limit ?? 50 });
export const runSpikeScan = () =>
  safeInvoke<number>('run_spike_scan');

// P1-1：News
export interface NewsItem {
  id: number;
  title: string;
  source: string;
  url: string;
  published_at: number;
  market_id: string | null;
  relevance_score: number | null;
  impact_direction: string | null;
  summary: string | null;
}
export const marketNews = (marketId: string) =>
  safeInvoke<NewsItem[]>('market_news', { marketId });
export const listAllNews = (limit?: number) =>
  safeInvoke<NewsItem[]>('list_all_news', { limit: limit ?? 50 });

// P1-2：NL Query
export interface NlQueryResult {
  sql: string;
  results: NlQueryRow[];
  explanation: string;
}
export interface NlQueryRow {
  market_id: string;
  question: string;
  yes_price: number | null;
  model_prob: number | null;
  edge: number | null;
  category: string;
}
export const nlQuery = (query: string) =>
  safeInvoke<NlQueryResult>('nl_query', { query });

// P1-3：Arbitrage Scanner
export interface ArbOpportunity {
  market_id: string;
  question: string;
  yes_cost: number;
  no_cost: number;
  total_cost: number;
  profit_margin: number;
  category: string;
}
export const arbScan = () =>
  safeInvoke<ArbOpportunity[]>('arb_scan');
export const listArbOpportunities = (limit?: number) =>
  safeInvoke<ArbOpportunity[]>('list_arb_opportunities', { limit: limit ?? 50 });

// P1-4：Cross-Platform Arbitrage
export interface CrossPlatformArb {
  match_name: string;
  market_question: string;
  pm_price: number;
  kalshi_price: number;
  spread: number;
  direction: string;
  est_profit_per_1000: number;
}
export const crossPlatformArbScan = () =>
  safeInvoke<CrossPlatformArb[]>('cross_platform_arb_scan');

// P2-1：Poisson Score Matrix
export interface ScoreMatrixResult {
  lambda_h: number;
  lambda_a: number;
  matrix: number[][];
  most_likely: [string, number][];
}
export const poissonScoreMatrix = (marketId: string) =>
  safeInvoke<ScoreMatrixResult>('poisson_score_matrix', { marketId });

// P2-2：Football Context
export interface FootballContext {
  home_team: string;
  away_team: string;
  home_rest_days: number | null;
  away_rest_days: number | null;
  home_matches_7d: number;
  away_matches_7d: number;
  home_fatigue: string;
  away_fatigue: string;
}
export const getFootballContext = (marketId: string) =>
  safeInvoke<FootballContext>('get_football_context', { marketId });

// P2-3：UMA Dispute Status
export interface UmaDisputeStatus {
  market_id: string;
  status: string;
  detail: string | null;
  raised_at: number | null;
  raised_by: string | null;
}
export const umaDisputeStatus = (marketId: string) =>
  safeInvoke<UmaDisputeStatus>('uma_dispute_status', { marketId });

// Phase 1.1：Crowd Wisdom —— 资金加权观点
export interface CrowdOpinion {
  market_id: string;
  yes_capital: number;
  no_capital: number;
  yes_weighted_pct: number;
  no_weighted_pct: number;
  hhi: number;
  top3_share: number;
  n_holders: number;
  smart_yes_pct: number;
  smart_divergence: number;
  computed_at: number;
}
export const crowdOpinion = (marketId: string) =>
  safeInvoke<CrowdOpinion>('crowd_opinion', { marketId });

// Phase 1.2：Mean Reversion
export interface ReversionSignal {
  market_id: string;
  current: number;
  mean: number;
  std_dev: number;
  z_score: number;
  bollinger_upper: number;
  bollinger_lower: number;
  is_overextended: boolean;
  direction: string;
  window_size: number;
  fade_signal: number;
  confidence: number;
  computed_at: number;
}
export const reversionSignal = (
  marketId: string,
  window?: number,
  zThreshold?: number,
) =>
  safeInvoke<ReversionSignal>('reversion_signal', {
    marketId,
    window: window ?? 20,
    zThreshold: zThreshold ?? 2.0,
  });
