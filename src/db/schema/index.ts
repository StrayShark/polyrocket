import { sqliteTable, text, integer, real, index } from 'drizzle-orm/sqlite-core';
import { sql } from 'drizzle-orm';

/**
 * polyrocket SQLite schema (10 tables)
 * Spec: polyradar-blueprint-v2-client.md §2
 *
 * All monetary values stored as TEXT (decimal string) to preserve precision.
 * All timestamps stored as INTEGER (Unix epoch milliseconds).
 */

// 1. wallets — connected EOA / smart wallets
export const wallets = sqliteTable(
  'wallets',
  {
    id: text('id').primaryKey(), // uuid
    address: text('address').notNull().unique(), // 0x...
    label: text('label'),
    chainId: integer('chain_id').notNull().default(137), // Polygon mainnet
    walletType: text('wallet_type').notNull(), // 'eoa' | 'smart'
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
    lastSyncedAt: integer('last_synced_at'),
  },
  (t) => ({ addrIdx: index('wallets_addr_idx').on(t.address) }),
);

// 2. markets — Polymarket markets metadata (cached)
export const markets = sqliteTable(
  'markets',
  {
    id: text('id').primaryKey(), // polymarket condition id
    slug: text('slug').notNull().unique(),
    question: text('question').notNull(),
    description: text('description'),
    category: text('category').notNull(), // 'football' | 'cs2' | 'politics'
    tags: text('tags'), // JSON array
    endDate: integer('end_date').notNull(), // close time
    active: integer('active', { mode: 'boolean' }).notNull().default(true),
    resolved: integer('resolved', { mode: 'boolean' }).notNull().default(false),
    outcome: text('outcome'), // 'YES' | 'NO' | null when unresolved
    liquidity: text('liquidity'), // decimal string, USDC
    volume24h: text('volume_24h'),
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
    updatedAt: integer('updated_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
  (t) => ({
    catIdx: index('markets_cat_idx').on(t.category, t.active),
    endIdx: index('markets_end_idx').on(t.endDate),
  }),
);

// 3. orderbook_snapshots — periodic CLOB snapshots (aggregated)
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
    bidLiquidity: text('bid_liquidity'), // depth up to 5%
    askLiquidity: text('ask_liquidity'),
  },
  (t) => ({
    marketTimeIdx: index('snapshots_market_time_idx').on(t.marketId, t.capturedAt),
  }),
);

// 4. ticks — raw price updates (downsampled after 24h)
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

// 5. signals — model output, recomputed periodically
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
    horizonHours: integer('horizon_hours').notNull(), // forecast horizon
    rationale: text('rationale'), // JSON, model explanation
    active: integer('active', { mode: 'boolean' }).notNull().default(true),
  },
  (t) => ({
    marketActiveIdx: index('signals_market_active_idx').on(t.marketId, t.active),
    edgeIdx: index('signals_edge_idx').on(t.edge),
  }),
);

// 6. model_performance — rolling model metrics (Brier, calibration, win rate)
export const modelPerformance = sqliteTable(
  'model_performance',
  {
    id: integer('id').primaryKey({ autoIncrement: true }),
    modelVersion: text('model_version').notNull(),
    category: text('category'), // null = aggregate
    windowStart: integer('window_start').notNull(),
    windowEnd: integer('window_end').notNull(),
    nPredictions: integer('n_predictions').notNull(),
    brierScore: real('brier_score').notNull(),
    logLoss: real('log_loss'),
    winRate: real('win_rate'),
    avgEdge: real('avg_edge'),
    calibration: text('calibration'), // JSON: array of {predicted_bucket, actual_freq, n}
  },
  (t) => ({
    modelWindowIdx: index('perf_model_window_idx').on(t.modelVersion, t.windowEnd),
  }),
);

// 7. bets — user-entered bet records
export const bets = sqliteTable(
  'bets',
  {
    id: text('id').primaryKey(), // uuid
    walletId: text('wallet_id').notNull().references(() => wallets.id),
    marketId: text('market_id').notNull().references(() => markets.id),
    signalId: integer('signal_id').references(() => signals.id), // null = manual
    mode: text('mode').notNull(), // 'A_jump' | 'B_signed' | 'manual'
    side: text('side').notNull(), // 'YES' | 'NO'
    size: text('size').notNull(), // decimal string, USDC
    price: real('price').notNull(),
    shares: text('shares').notNull(), // decimal string
    placedAt: integer('placed_at').notNull(),
    settledAt: integer('settled_at'),
    pnl: text('pnl'), // decimal string, signed
    status: text('status').notNull(), // 'open' | 'won' | 'lost' | 'cancelled'
    txHash: text('tx_hash'), // for mode B
    notes: text('notes'),
  },
  (t) => ({
    walletIdx: index('bets_wallet_idx').on(t.walletId, t.placedAt),
    marketIdx: index('bets_market_idx').on(t.marketId, t.placedAt),
    statusIdx: index('bets_status_idx').on(t.status),
  }),
);

// 8. copy_targets — Polymarket addresses to mirror
export const copyTargets = sqliteTable(
  'copy_targets',
  {
    id: text('id').primaryKey(), // uuid
    address: text('address').notNull().unique(),
    label: text('label'),
    enabled: integer('enabled', { mode: 'boolean' }).notNull().default(true),
    allocationCap: text('allocation_cap'), // decimal string, USDC max position
    minEdge: real('min_edge').notNull().default(0.05),
    createdAt: integer('created_at').notNull().default(sql`(unixepoch() * 1000)`),
  },
  (t) => ({ addrIdx: index('copy_addr_idx').on(t.address) }),
);

// 9. copy_events — every fill detected from a copy target
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
    matchedBetId: text('matched_bet_id').references(() => bets.id), // if user copied
  },
  (t) => ({
    targetTimeIdx: index('copy_events_target_time_idx').on(t.targetId, t.detectedAt),
  }),
);

// 10. audit_log — mode B signed orders, key access, config changes
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

export type Wallet = typeof wallets.$inferSelect;
export type Market = typeof markets.$inferSelect;
export type Signal = typeof signals.$inferSelect;
export type Bet = typeof bets.$inferSelect;
export type CopyTarget = typeof copyTargets.$inferSelect;
export type CopyEvent = typeof copyEvents.$inferSelect;
export type ModelPerformance = typeof modelPerformance.$inferSelect;