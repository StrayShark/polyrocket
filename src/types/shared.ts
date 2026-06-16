// Shared DTOs used by multiple modules.

export interface CopyTarget {
  id: string;
  address: string;
  label: string | null;
  enabled: boolean;
  allocation_cap: string | null;
  min_edge: number;
  created_at: number;
}

export interface CopyEvent {
  id: number;
  target_id: string;
  market_id: string;
  detected_at: number;
  side: string;
  size: string;
  price: number;
  tx_hash: string;
  matched_bet_id: string | null;
}

export interface AddCopyTargetArgs {
  address: string;
  label?: string;
  allocation_cap?: string;
  min_edge?: number;
}

export interface DashboardKpis {
  total_equity_usdc: string;
  open_pnl_usdc: string;
  win_rate_30d: number;
  brier_score: number;
  active_signals: number;
  open_positions: number;
}

export interface DailyBriefEntry {
  market_id: string;
  market_question: string;
  market_category: string;
  market_end_date: number;
  market_liquidity: string | null;
  market_volume_24h: string | null;
  rank: number;
  match_score: number;
  score_breakdown: string | null;
  edge: number | null;
  confidence: number | null;
  consensus_side: string | null;
  consensus_strength: number | null;
  computed_at: number;
  expires_at: number;
  dismissed: boolean;
}

export interface BriefWeights {
  w1: number; // edge
  w2: number; // confidence
  w3: number; // consensus
  w4: number; // time
  w5: number; // user_interest
  w6: number; // cost penalty
}

export interface BriefRefreshResult {
  computed_at: number;
  n_items: number;
}

export interface SetBriefPrefsArgs {
  user_id: string;
  weights?: BriefWeights;
  max_items?: number;
  min_liquidity?: string;
  categories?: string[];
}

export interface SchedulerStatus {
  health_probe_interval_sec: number;
  daily_brief_hour_utc: number;
  daily_brief_tz_offset_min: number;
  anomaly_window_sec: number;
  next_brief_run_at_unix_ms: number;
}

export interface SchedulerTriggerResult {
  triggered_at_unix_ms: number;
  kind: 'health_probe' | 'daily_brief';
  ok: boolean;
  error: string | null;
}

export interface SecretsStatus {
  llm_keys: number;
  pm_api: boolean;
  pm_passphrase: boolean;
  pm_secret: boolean;
  wallet_pk: number;
}

export interface AuditEntry {
  id: number;
  at: number;
  actor: string;
  action: string;
  target: string | null;
  payload: string | null;
  result: string;
}
