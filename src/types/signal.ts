// Signal DTOs (mirror src-tauri/src/commands/signal.rs + domain::signal)

/** L1 Signal DTO。**镜像** `src-tauri/src/commands/signal.rs::SignalDto`。
 *
 * **JOIN `markets`**：IPC 返回的 `SignalDto` 已经 JOIN `markets` 表，
 * 所以 `market_question` / `market_slug` 是 nullable field（L1 用
 * `?? market_id` 兜底显示）。
 *
 * **`id` 是 number**（不是 UUID）—— `signals` 表用 `INTEGER PRIMARY KEY AUTOINCREMENT`。
 */
export interface Signal {
  id: number;
  market_id: string;
  computed_at: number;
  model_version: string;
  predicted_prob: number;
  market_prob: number;
  edge: number;
  confidence: number;
  horizon_hours: number;
  rationale: string | null;
  market_question?: string | null;
  market_slug?: string | null;
}

export interface ListSignalsArgs {
  min_edge?: number;
  category?: string;
  limit?: number;
}
