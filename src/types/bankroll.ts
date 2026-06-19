// v0.78 — bankroll allocation DTOs (L1 ↔ L2 boundary).
//
// Mirror of `src-tauri/src/commands/bankroll.rs` types. The
// Rust side derives `specta::Type` on these so the codegen
// pipeline (v0.76+) can pick them up in Phase 3.

export type BetSide = 'Yes' | 'No';

export type CappedReason =
  | 'PerSignalCap'
  | 'Liquidity'
  | 'TotalExposure';

export interface BankrollConfigDto {
  kelly_multiplier: number;
  max_per_signal_pct: number;
  reserve_pct: number;
  min_edge_pct: number;
  max_total_exposure_pct: number;
  min_confidence: number;
}

export interface AllocationItem {
  market_id: string;
  side: BetSide;
  size_usdc: string;
  kelly_pct: number;
  capped_reason: CappedReason | null;
  source_signal_ids: string[];
  model_version: string;
  confidence: number;
  expected_roi: number;
}

export interface AllocationResult {
  total_allocated_usdc: string;
  reserved_usdc: string;
  per_market: AllocationItem[];
  dropped_markets: string[];
}

export interface ComputeAllocationArgs {
  bankroll_usdc: string;
  config?: BankrollConfigDto | null;
  signals: import('@/types/signal').Signal[];
  market_liquidity?: Record<string, string> | null;
}
