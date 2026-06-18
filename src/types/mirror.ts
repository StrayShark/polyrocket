// v0.55 — mirror executor DTOs.
//
// Mirrors `src-tauri/src/commands/mirror_executor.rs`
// (MirrorRow, ListMirrorsArgs, EnqueueArgs, etc.)
// and `src-tauri/src/domain/mirror/mod.rs`
// (ExecutorPassResult, MirrorQueueStats).
//
// The L1 uses these in `enqueue_mirror` /
// `list_mirrors` / `run_mirror_executor_pass` /
// `mirror_queue_stats` IPCs.

export interface EnqueueMirrorArgs {
  event_id: number;
  target_id: string;
  market_id: string;
  side: string;
  size: string;
  flipped: boolean;
}

export interface MirrorRow {
  id: string;
  event_id: number;
  target_id: string;
  market_id: string;
  side: string;
  size: string;
  flipped: boolean;
  status: string;
  created_at: number;
}

export interface ListMirrorsArgs {
  status?: string;
  limit?: number;
}

export interface RunMirrorPassArgs {
  /** Optional override of the per-cycle
   * exposure cap (USDC). When omitted, the
   * executor uses the configured cap. */
  exposure_cap_usdc?: number;
  /** When true, dry-run: enqueue + size
   * decisions are computed but no orders are
   * actually submitted. */
  dry_run?: boolean;
}

export interface ExecutorPassResult {
  /** True when the pass ran (even if no orders
   * were submitted — the queue may be empty). */
  ok: boolean;
  /** ISO timestamp at the start of the pass. */
  started_at: string;
  /** ISO timestamp at the end of the pass. */
  finished_at: string;
  /** Number of orders submitted this pass. */
  submitted: number;
  /** Number of orders rejected (cap / flipped
   * / size / wallet). */
  rejected: number;
  /** Per-reject breakdown (by reason). */
  reject_breakdown: {
    cap?: number;
    flipped?: number;
    size?: number;
    wallet?: number;
    other?: number;
  };
  /** Total exposure in USDC after the pass. */
  total_exposure_usdc: number;
  /** Headroom in USDC (cap - exposure). */
  headroom_usdc: number;
  /** Optional message for the L1 toast. */
  message?: string;
}

export interface MirrorQueueStats {
  n_pending: number;
  n_submitted: number;
  n_filled: number;
  n_rejected: number;
  n_expired: number;
  total_exposure_usdc: number;
  headroom_usdc: number;
}
