/**
 * L1 domain mirror — M6 PnL.
 * Mirrors src-tauri/src/domain/pnl/mod.rs.
 */

/** L1 dashboard KPI 聚合 DTO。**镜像** `src-tauri/src/domain/pnl::DashboardKpis`（Rust 端的 DTO）。L1 直接消费 IPC 返回的 JSON 解析到这个 type。
 *
 * **字段语义**：
 *   - `total_equity_usdc` — 钱包余额 + 未结算 PnL
 *   - `open_pnl_usdc` — 当前 open bet 的 mark-to-market PnL
 *   - `win_rate_30d` — 最近 30 天 win 比例（0..1）
 *   - `brier_score` — 最近 30 天 model Brier（越低越好）
 *   - `active_signals` — 当前 active signals 数
 *   - `open_positions` — 当前 open bets 数
 *
 * **USDC 都用 string**：避免浮点精度问题（`1.0000001 USDC`），parseFloat 渲染。
 */
export interface DashboardKpis {
  total_equity_usdc: string;
  open_pnl_usdc: string;
  win_rate_30d: number;
  brier_score: number;
  active_signals: number;
  open_positions: number;
}

/** 算 win rate = won / (won + lost)。`won + lost = 0` 时返回 0。
 *
 * @param won — 赢的 bet 数
 * @param lost — 输的 bet 数
 * @returns win rate ∈ [0, 1]
 */
export function winRate(won: number, lost: number): number {
  const total = won + lost;
  return total === 0 ? 0 : won / total;
}

/** 算 Brier score：mean (predicted - actual)²。
 *
 * **为什么返回 null 而不是 0**：空 list / 长度不匹配 → 无效输入。
 * 返回 null 让调用方决定怎么处理（静默 / toast 错误）。
 *
 * @param predicted — model 预测概率 ∈ [0, 1]
 * @param actuals — 实际结果 0.0 或 1.0
 * @returns Brier score ∈ [0, 1]，或 null（输入无效）
 */
export function brierScore(predicted: number[], actuals: number[]): number | null {
  if (predicted.length === 0 || predicted.length !== actuals.length) return null;
  let sum = 0;
  for (let i = 0; i < predicted.length; i++) {
    const d = predicted[i] - actuals[i];
    sum += d * d;
  }
  return sum / predicted.length;
}

/** 累加已实现的 PnL（USDC）。`null` / `undefined` / 非有限数都跳过。
 *
 * @param pnls — 每条 bet 的 PnL 字符串（DB 存为 decimal string）
 * @returns 总 PnL（USDC）
 */
export function realizedPnl(pnls: Array<string | null | undefined>): number {
  let total = 0;
  for (const p of pnls) {
    if (p == null) continue;
    const n = Number(p);
    if (Number.isFinite(n)) total += n;
  }
  return total;
}

export type BetCategory = 'win' | 'loss' | 'open' | 'other';

/** 把单条 bet 分到 4 类之一：`'win'` / `'loss'` / `'open'` / `'other'`。
 *
 * **优先级**：`status` > `pnl` —— `status = 'open'` 一定归 `'open'`，不看 pnl。
 *
 * @param pnl — bet PnL（USDC decimal string）
 * @param status — bet status 字符串（`'open'` / `'won'` / `'lost'` / `'cancelled'`）
 * @returns 分类
 */
export function categorize(pnl: string | null | undefined, status: string): BetCategory {
  if (status === 'open') return 'open';
  if (status === 'cancelled') return 'other';
  if (pnl == null) return 'other';
  const n = Number(pnl);
  if (!Number.isFinite(n)) return 'other';
  return n > 0 ? 'win' : n < 0 ? 'loss' : 'other';
}

/** PnL 聚合统计。L1 「History → Summary」面板用。
 *
 * **字段**：
 *   - `nTotal / nWon / nLost / nOpen` — 各状态 bet 数
 *   - `winRate` — win 比例
 *   - `totalPnl` — 总 PnL（USDC，含 win 正 + loss 负）
 *   - `avgWin / avgLoss` — win / loss 各自平均 PnL
 */
export interface PnLSummary {
  nTotal: number;
  nWon: number;
  nLost: number;
  nOpen: number;
  winRate: number;
  totalPnl: number;
  avgWin: number;
  avgLoss: number;
}

/** 算一批 bet 的 PnL summary。每行 `(pnl_str, status_str)`。
 *
 * **调用方**：L1 「History」页面 mount 时调，预算整页的统计。
 *
 * @param rows — 每条 bet 的 `(pnl, status)`
 * @returns PnL summary
 */
export function summarize(rows: Array<[string | null | undefined, string]>): PnLSummary {
  let nWon = 0;
  let nLost = 0;
  let nOpen = 0;
  let sumWin = 0;
  let sumLoss = 0;
  let total = 0;
  for (const [pnl, status] of rows) {
    const cat = categorize(pnl, status);
    const n = pnl == null ? null : Number(pnl);
    if (cat === 'win' && n != null && Number.isFinite(n)) {
      nWon += 1;
      sumWin += n;
      total += n;
    } else if (cat === 'loss' && n != null && Number.isFinite(n)) {
      nLost += 1;
      sumLoss += n;
      total += n;
    } else if (cat === 'open') {
      nOpen += 1;
    }
  }
  return {
    nTotal: rows.length,
    nWon,
    nLost,
    nOpen,
    winRate: winRate(nWon, nLost),
    totalPnl: total,
    avgWin: nWon > 0 ? sumWin / nWon : 0,
    avgLoss: nLost > 0 ? sumLoss / nLost : 0,
  };
}
