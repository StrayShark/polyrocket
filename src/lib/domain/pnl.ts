/**
 * L1 domain mirror — M6 PnL.
 * Mirrors src-tauri/src/domain/pnl/mod.rs.
 */

export interface DashboardKpis {
  total_equity_usdc: string;
  open_pnl_usdc: string;
  win_rate_30d: number;
  brier_score: number;
  active_signals: number;
  open_positions: number;
}

export function winRate(won: number, lost: number): number {
  const total = won + lost;
  return total === 0 ? 0 : won / total;
}

export function brierScore(predicted: number[], actuals: number[]): number | null {
  if (predicted.length === 0 || predicted.length !== actuals.length) return null;
  let sum = 0;
  for (let i = 0; i < predicted.length; i++) {
    const d = predicted[i] - actuals[i];
    sum += d * d;
  }
  return sum / predicted.length;
}

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

export function categorize(pnl: string | null | undefined, status: string): BetCategory {
  if (status === 'open') return 'open';
  if (status === 'cancelled') return 'other';
  if (pnl == null) return 'other';
  const n = Number(pnl);
  if (!Number.isFinite(n)) return 'other';
  return n > 0 ? 'win' : n < 0 ? 'loss' : 'other';
}

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
