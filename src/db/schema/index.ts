import { sqliteTable, text, integer, real, index } from 'drizzle-orm/sqlite-core';
import { sql } from 'drizzle-orm';

/**
 * polyrocket SQLite schema (10 张表)
 * 规范: polyradar-blueprint-v2-client.md §2
 *
 * 所有金额字段存为 TEXT (decimal string) 以保留精度。
 * 所有时间戳存为 INTEGER (Unix 纪元毫秒)。
 */

// 1. wallets — 已连接的 EOA / smart 钱包
export const wallets = sqliteTable(
  'wallets',
  {
    id: text('id').primaryKey(), // uuid
    address: text('address').notNull().unique(), // 0x...
    label: text('label'),
    chainId: integer('chain_id').notNull().default(137), // Polygon 主网
    walletType: text('wallet_type').notNull(), // 'eoa' | 'smart'
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
    lastSyncedAt: integer('last_synced_at'),
  },
  (t) => ({ addrIdx: index('wallets_addr_idx').on(t.address) }),
);

// 2. markets —— Polymarket 市场元数据（缓存）
export const markets = sqliteTable(
  'markets',
  {
    id: text('id').primaryKey(), // polymarket condition id
    slug: text('slug').notNull().unique(),
    question: text('question').notNull(),
    description: text('description'),
    category: text('category').notNull(), // 'football' | 'cs2' | 'politics'
    tags: text('tags'), // JSON 数组
    endDate: integer('end_date').notNull(), // 关闭时间
    active: integer('active', { mode: 'boolean' }).notNull().default(true),
    resolved: integer('resolved', { mode: 'boolean' }).notNull().default(false),
    outcome: text('outcome'), // 'YES' | 'NO' | null 表示未结算
    liquidity: text('liquidity'), // decimal string, USDC
    volume24h: text('volume_24h'),
    userInterested: integer('user_interested', { mode: 'boolean' }).notNull().default(false), // v0.2: 用户关注列表
    briefDismissedAt: integer('brief_dismissed_at'), // v0.2: 用户最近一次在每日简报中忽略该市场的时间
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
    updatedAt: integer('updated_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
  (t) => ({
    catIdx: index('markets_cat_idx').on(t.category, t.active),
    endIdx: index('markets_end_idx').on(t.endDate),
  }),
);

// 3. orderbook_snapshots — 周期性的 CLOB 快照（聚合后）
export const orderbookSnapshots = sqliteTable(
  'orderbook_snapshots',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    marketId: text('market_id').notNull().references(() => markets.id),
    capturedAt: integer('captured_at').notNull(),
    bestBid: real('best_bid').notNull(),
    bestAsk: real('best_ask').notNull(),
    midPrice: real('mid_price').notNull(),
    spread: real('spread').notNull(),
    bidLiquidity: text('bid_liquidity'), // 5% 以内的深度
    askLiquidity: text('ask_liquidity'),
  },
  (t) => ({
    marketTimeIdx: index('snapshots_market_time_idx').on(t.marketId, t.capturedAt),
  }),
);

// 4. ticks — 原始价格更新（24h 之后降采样）
export const ticks = sqliteTable(
  'ticks',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    marketId: text('market_id').notNull().references(() => markets.id),
    capturedAt: integer('captured_at').notNull(),
    price: real('price').notNull(),
    side: text('side').notNull(), // 'YES' | 'NO'
    size: text('size'), // decimal string
  },
  (t) => ({
    marketTimeIdx: index('ticks_market_time_idx').on(t.marketId, t.capturedAt),
  }),
);

// 5. signals — 模型输出，周期重算
export const signals = sqliteTable(
  'signals',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    marketId: text('market_id').notNull().references(() => markets.id),
    computedAt: integer('computed_at').notNull(),
    modelVersion: text('model_version').notNull(),
    predictedProb: real('predicted_prob').notNull(), // 0..1
    marketProb: real('market_prob').notNull(),
    edge: real('edge').notNull(), // predicted - market
    confidence: real('confidence').notNull(), // 0..1
    horizonHours: integer('horizon_hours').notNull(), // 预测时间窗
    rationale: text('rationale'), // JSON, 模型解释
    active: integer('active', { mode: 'boolean' }).notNull().default(true),
  },
  (t) => ({
    marketActiveIdx: index('signals_market_active_idx').on(t.marketId, t.active),
    edgeIdx: index('signals_edge_idx').on(t.edge),
  }),
);

// 6. model_performance — 滚动模型指标（Brier、校准、胜率）
export const modelPerformance = sqliteTable(
  'model_performance',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    modelVersion: text('model_version').notNull(),
    category: text('category'), // null = 聚合
    windowStart: integer('window_start').notNull(),
    windowEnd: integer('window_end').notNull(),
    nPredictions: integer('n_predictions').notNull(),
    brierScore: real('brier_score').notNull(),
    logLoss: real('log_loss'),
    winRate: real('win_rate'),
    avgEdge: real('avg_edge'),
    calibration: text('calibration'), // JSON: {predicted_bucket, actual_freq, n} 数组
  },
  (t) => ({
    modelWindowIdx: index('perf_model_window_idx').on(t.modelVersion, t.windowEnd),
  }),
);

// 7. bets — 用户录入的投注记录
export const bets = sqliteTable(
  'bets',
  {
    id: text('id').primaryKey(), // uuid
    walletId: text('wallet_id').notNull().references(() => wallets.id),
    marketId: text('market_id').notNull().references(() => markets.id),
    signalId: integer('signal_id').references(() => signals.id), // null = 手动
    // 注意: decisionId 故意不引用 llmDecisions.id —
    // 那会产生循环类型推导。外键在应用层
    // 强制（参见 src-tauri/src/commands/llm.rs）。
    decisionId: integer('decision_id'), // v0.2: 关联 LLM decision
    wasLlmAssisted: integer('was_llm_assisted', { mode: 'boolean' }).notNull().default(false), // v0.2
    mode: text('mode').notNull(), // 'A_jump' | 'B_signed' | 'manual'
    side: text('side').notNull(), // 'YES' | 'NO'
    size: text('size').notNull(), // decimal string, USDC
    price: real('price').notNull(),
    shares: text('shares').notNull(), // decimal string
    placedAt: integer('placed_at').notNull(),
    settledAt: integer('settled_at'),
    pnl: text('pnl'), // decimal string, 带符号
    status: text('status').notNull(), // 'open' | 'won' | 'lost' | 'cancelled'
    txHash: text('tx_hash'), // mode B 用
    notes: text('notes'),
  },
  (t) => ({
    walletIdx: index('bets_wallet_idx').on(t.walletId, t.placedAt),
    marketIdx: index('bets_market_idx').on(t.marketId, t.placedAt),
    statusIdx: index('bets_status_idx').on(t.status),
  }),
);

// 8. copy_targets — 要镜像的 Polymarket 地址
export const copyTargets = sqliteTable(
  'copy_targets',
  {
    id: text('id').primaryKey(), // uuid
    address: text('address').notNull().unique(),
    label: text('label'),
    enabled: integer('enabled', { mode: 'boolean' }).notNull().default(true),
    allocationCap: text('allocation_cap'), // decimal string, USDC 最大持仓
    minEdge: real('min_edge').notNull().default(0.05),
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
  (t) => ({ addrIdx: index('copy_addr_idx').on(t.address) }),
);

// 9. copy_events —— 从某个 copy target 检测到的每次成交
export const copyEvents = sqliteTable(
  'copy_events',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    targetId: text('target_id').notNull().references(() => copyTargets.id),
    marketId: text('market_id').notNull().references(() => markets.id),
    detectedAt: integer('detected_at').notNull(),
    side: text('side').notNull(),
    size: text('size').notNull(),
    price: real('price').notNull(),
    txHash: text('tx_hash').notNull().unique(),
    matchedBetId: text('matched_bet_id').references(() => bets.id), // 若用户跟单则非空
  },
  (t) => ({
    targetTimeIdx: index('copy_events_target_time_idx').on(t.targetId, t.detectedAt),
  }),
);

// 10. audit_log — mode B 签名订单、密钥访问、配置变更
export const auditLog = sqliteTable(
  'audit_log',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    at: integer('at').notNull().default(sql`(unixepoch() * 1000)`),
    actor: text('actor').notNull(), // 'user' | 'system' | 'tauri:<cmd>'
    action: text('action').notNull(), // 'bet.place' | 'key.access' | 'config.change' | ...
    target: text('target'),
    payload: text('payload'), // JSON
    result: text('result'), // 'ok' | 'error:...'
  },
  (t) => ({ atIdx: index('audit_at_idx').on(t.at) }),
);

// === M10 LLM Analysis (v0.2) ===

// 11. llm_providers — provider 配置（API key 存在 OS keyring）
export const llmProviders = sqliteTable(
  'llm_providers',
  {
    id: text('id').primaryKey(), // 'openai' | 'anthropic' | 'google' | 'deepseek' | 'xai'
    displayName: text('display_name').notNull(), // 'GPT-4o' | 'Claude Sonnet 4' | ...
    providerKind: text('provider_kind').notNull().default('openai'), // 'openai' | 'anthropic' | 'google' | 'deepseek' | 'openai_compat' | 'anthropic_compat'
    requestFormat: text('request_format').notNull().default('chat_completions'), // 'chat_completions' | 'messages' | 'generate_content'
    supportsStreaming: integer('supports_streaming', { mode: 'boolean' }).notNull().default(false),
    enabled: integer('enabled', { mode: 'boolean' }).notNull().default(true),
    apiBase: text('api_base'), // 自建代理/第三方转发
    keyAlias: text('key_alias').notNull(), // OS keyring 中的主 key alias（遗留字段）
    defaultModel: text('default_model').notNull(), // 'gpt-4o-2024-08-06' / etc
    timeoutMs: integer('timeout_ms').notNull().default(30000),
    requestTimeoutMs: integer('request_timeout_ms').notNull().default(30000),
    maxRetries: integer('max_retries').notNull().default(2),
    costPer1kIn: real('cost_per_1k_in'), // cents
    costPer1kOut: real('cost_per_1k_out'),
    rateLimitRpm: integer('rate_limit_rpm'), // 每分钟请求数（provider 文档）
    rateLimitTpm: integer('rate_limit_tpm'), // 每分钟 token 数
    quotaDailyCents: real('quota_daily_cents'), // 每日硬性上限
    quotaMonthlyCents: real('quota_monthly_cents'), // 每月硬性上限
    keyRotationStrategy: text('key_rotation_strategy').notNull().default('failover'), // 'failover' | 'round_robin' | 'manual'
    healthStatus: text('health_status').notNull().default('unknown'), // 'ok' | 'slow' | 'failing' | 'unreachable' | 'unknown'
    healthLatencyP50Ms: integer('health_latency_p50_ms'),
    healthLatencyP95Ms: integer('health_latency_p95_ms'),
    lastHealthCheckAt: integer('last_health_check_at'),
    lastHealthError: text('last_health_error'),
    notes: text('notes'),
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
    updatedAt: integer('updated_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
);

// 11b. llm_provider_keys — 每个 provider 多个 API key（M11 v0.2）
export const llmProviderKeys = sqliteTable(
  'llm_provider_keys',
  {
    id: text('id').primaryKey(), // uuid
    providerId: text('provider_id').notNull().references(() => llmProviders.id),
    alias: text('alias').notNull(), // 'prod-1' | 'backup-azure' | 'dev'
    keyringAlias: text('keyring_alias').notNull(), // OS keyring 别名
    enabled: integer('enabled', { mode: 'boolean' }).notNull().default(true),
    priority: integer('priority').notNull().default(0), // 越小优先级越高
    weight: integer('weight').notNull().default(1), // round_robin 权重
    lastUsedAt: integer('last_used_at'),
    lastError: text('last_error'),
    lastErrorAt: integer('last_error_at'),
    totalCalls: integer('total_calls').notNull().default(0),
    totalErrors: integer('total_errors').notNull().default(0),
    notes: text('notes'),
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
    updatedAt: integer('updated_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
  (t) => ({
    providerIdx: index('llm_keys_provider_idx').on(t.providerId, t.priority),
  }),
);

// 11c. llm_call_logs —— 用于流量监控的每次请求日志（M11 v0.2）
export const llmCallLogs = sqliteTable(
  'llm_call_logs',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    analysisId: text('analysis_id'), // 可选，来自 llm_analyze 时填写
    providerId: text('provider_id').notNull().references(() => llmProviders.id),
    keyId: text('key_id').references(() => llmProviderKeys.id),
    calledAt: integer('called_at').notNull(),
    latencyMs: integer('latency_ms').notNull(),
    tokensIn: integer('tokens_in').notNull().default(0),
    tokensOut: integer('tokens_out').notNull().default(0),
    costCents: real('cost_cents').notNull().default(0),
    httpStatus: integer('http_status').notNull(),
    success: integer('success', { mode: 'boolean' }).notNull(),
    errorCode: text('error_code'),
    errorMessage: text('error_message'),
    promptVersion: text('prompt_version'),
    predictedProb: real('predicted_prob'),
    recommendedSide: text('recommended_side'),
    caller: text('caller').notNull(), // 'm10.llm_analyze' | 'm12.brief' | 'm7.model' | 'user.test' | 'health.probe'
    retryCount: integer('retry_count').notNull().default(0),
  },
  (t) => ({
    providerTimeIdx: index('call_logs_provider_time_idx').on(t.providerId, t.calledAt),
    analysisIdx: index('call_logs_analysis_idx').on(t.analysisId),
    successTimeIdx: index('call_logs_success_time_idx').on(t.success, t.calledAt),
  }),
);

// 11d. llm_health_checks — 连通性探测历史（M11 v0.2）
export const llmHealthChecks = sqliteTable(
  'llm_health_checks',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    providerId: text('provider_id').notNull().references(() => llmProviders.id),
    keyId: text('key_id').references(() => llmProviderKeys.id),
    checkedAt: integer('checked_at').notNull(),
    trigger: text('trigger').notNull(), // 'user' | 'probe' | 'auto_after_fail'
    success: integer('success', { mode: 'boolean' }).notNull(),
    latencyMs: integer('latency_ms'),
    httpStatus: integer('http_status'),
    errorCode: text('error_code'), // 'auth' | 'rate_limit' | 'timeout' | 'network' | 'parse'
    errorMessage: text('error_message'),
    modelUsed: text('model_used'),
    testRequestId: text('test_request_id'),
  },
  (t) => ({
    providerTimeIdx: index('health_provider_time_idx').on(t.providerId, t.checkedAt),
    successTimeIdx: index('health_success_time_idx').on(t.success, t.checkedAt),
  }),
);

// 12. llm_analyses —— 一次多 LLM 分析请求
export const llmAnalyses = sqliteTable(
  'llm_analyses',
  {
    id: text('id').primaryKey(), // uuid
    marketId: text('market_id').notNull().references(() => markets.id),
    signalId: integer('signal_id').references(() => signals.id),
    promptVersion: text('prompt_version').notNull(), // 'v3.2'
    requestedAt: integer('requested_at').notNull(),
    completedAt: integer('completed_at'),
    status: text('status').notNull(), // 'pending' | 'completed' | 'partial' | 'failed'
    consensusPredicted: real('consensus_predicted'), // 加权中位数 0..1
    consensusSide: text('consensus_side'), // 'YES' | 'NO' | 'skip'
    consensusConf: real('consensus_conf'),
    totalLatencyMs: integer('total_latency_ms'),
    costCents: real('cost_cents'),
    triggeredBy: text('triggered_by').notNull(), // 'user:<id>' | 'auto:signal_refresh'
  },
  (t) => ({
    marketTimeIdx: index('analyses_market_time_idx').on(t.marketId, t.requestedAt),
    statusIdx: index('analyses_status_idx').on(t.status),
  }),
);

// 13. llm_recommendations — 一次分析中每个 LLM 的输出
export const llmRecommendations = sqliteTable(
  'llm_recommendations',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    analysisId: text('analysis_id').notNull().references(() => llmAnalyses.id),
    providerId: text('provider_id').notNull().references(() => llmProviders.id),
    predictedProb: real('predicted_prob'), // 0..1
    side: text('side'), // 'YES' | 'NO' | 'skip'
    confidence: real('confidence'), // 0..1
    reasoning: text('reasoning'), // LLM 自然语言
    latencyMs: integer('latency_ms'),
    tokensIn: integer('tokens_in'),
    tokensOut: integer('tokens_out'),
    costCents: real('cost_cents'),
    rawResponse: text('raw_response'), // 完整 JSON，仅调试用
    parseOk: integer('parse_ok', { mode: 'boolean' }).notNull(),
    parseError: text('parse_error'),
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
  (t) => ({
    analysisIdx: index('recs_analysis_idx').on(t.analysisId),
    providerIdx: index('recs_provider_idx').on(t.providerId),
  }),
);

// 14. llm_decisions — 用户的最终决策（是否跟了 LLM）
export const llmDecisions = sqliteTable(
  'llm_decisions',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    analysisId: text('analysis_id').notNull().references(() => llmAnalyses.id),
    // 注意: betId 故意不引用 bets.id 以避免
    // 循环类型推导。外键在应用层强制。
    betId: text('bet_id'), // skip 时为 null
    userDecision: text('user_decision').notNull(), // 'follow_top' | 'manual_yes' | 'manual_no' | 'skip' | 're_analyze'
    userDecidedSide: text('user_decided_side'), // YES / NO / NULL
    followedLlmId: integer('followed_llm_id').references(() => llmRecommendations.id),
    decidedAt: integer('decided_at').notNull(),
    contextSnapshot: text('context_snapshot'), // 用于回放的 UI state JSON
  },
  (t) => ({
    analysisIdx: index('decisions_analysis_idx').on(t.analysisId),
    betIdx: index('decisions_bet_idx').on(t.betId),
  }),
);

// === v0.2 — Daily Brief ===

// 15. daily_briefs — 今日简报的 top-N 市场缓存
export const dailyBriefs = sqliteTable(
  'daily_briefs',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    marketId: text('market_id').notNull().references(() => markets.id),
    rank: integer('rank').notNull(), // 1..N（按 match_score DESC）
    matchScore: real('match_score').notNull(),
    scoreBreakdown: text('score_breakdown'), // JSON: {edge: 0.85, confidence: 0.7, ...}
    computedAt: integer('computed_at').notNull(),
    expiresAt: integer('expires_at').notNull(), // = 今日结束
  },
  (t) => ({
    rankIdx: index('daily_briefs_rank_idx').on(t.rank, t.computedAt),
    marketIdx: index('daily_briefs_market_idx').on(t.marketId, t.computedAt),
  }),
);

// 16. user_brief_prefs — 每日简报的用户偏好（每用户 1 行）
export const userBriefPrefs = sqliteTable(
  'user_brief_prefs',
  {
    userId: text('user_id').primaryKey(), // 钱包地址或 'default'
    weightsJson: text('weights_json').notNull(), // {w1: 0.35, w2: 0.2, ...}
    maxItems: integer('max_items').notNull().default(5),
    minLiquidity: text('min_liquidity'), // decimal string, USDC
    categories: text('categories'), // JSON 数组: ['football', 'cs2']
    updatedAt: integer('updated_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
);

export type Wallet = typeof wallets.$inferSelect;
export type Market = typeof markets.$inferSelect;
export type Signal = typeof signals.$inferSelect;
export type Bet = typeof bets.$inferSelect;
export type CopyTarget = typeof copyTargets.$inferSelect;
export type CopyEvent = typeof copyEvents.$inferSelect;
export type ModelPerformance = typeof modelPerformance.$inferSelect;
export type LlmProvider = typeof llmProviders.$inferSelect;
export type LlmAnalysis = typeof llmAnalyses.$inferSelect;
export type LlmRecommendation = typeof llmRecommendations.$inferSelect;
export type LlmDecision = typeof llmDecisions.$inferSelect;
export type LlmProviderKey = typeof llmProviderKeys.$inferSelect;
export type LlmCallLog = typeof llmCallLogs.$inferSelect;
export type LlmHealthCheck = typeof llmHealthChecks.$inferSelect;