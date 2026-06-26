// v0.55 —— mirror executor DTO。
//
// 镜像 `src-tauri/src/commands/mirror_executor.rs`
// (MirrorRow, ListMirrorsArgs, EnqueueArgs, 等)
// 以及 `src-tauri/src/domain/mirror/mod.rs`
// (ExecutorPassResult, MirrorQueueStats)。
//
// L1 在 `enqueue_mirror` / `list_mirrors` /
// `run_mirror_executor_pass` / `mirror_queue_stats`
// 这几个 IPC 中使用这些类型。

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
  /** 单次循环曝光上限（USDC）的可选
   * 覆盖值。省略时执行器
   * 使用已配置的 cap。 */
  exposure_cap_usdc?: number;
  /** 当为 true 时为 dry-run：会计算
   * 入队与仓位决策，但实际不会
   * 提交订单。 */
  dry_run?: boolean;
}

export interface ExecutorPassResult {
  /** 当 pass 执行时为 true（即使没有
   * 提交订单 —— 队列可能为空）。 */
  ok: boolean;
  /** 本次 pass 开始时的 ISO 时间戳。 */
  started_at: string;
  /** 本次 pass 结束时的 ISO 时间戳。 */
  finished_at: string;
  /** 本次 pass 提交的订单数。 */
  submitted: number;
  /** 被拒绝的订单数（cap / flipped /
   * size / wallet）。 */
  rejected: number;
  /** 按原因分类的拒绝明细。 */
  reject_breakdown: {
    cap?: number;
    flipped?: number;
    size?: number;
    wallet?: number;
    other?: number;
  };
  /** 本次 pass 后的 USDC 总敞口。 */
  total_exposure_usdc: number;
  /** USDC 剩余空间（cap - exposure）。 */
  headroom_usdc: number;
  /** 供 L1 toast 使用的可选消息。 */
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
