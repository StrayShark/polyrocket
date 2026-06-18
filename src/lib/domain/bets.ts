/**
 * L1 domain mirror — M3 Bets.
 * Mirrors src-tauri/src/domain/bet/mod.rs.
 */

export type BetMode = 'A_jump' | 'B_signed';
export type BetStatus = 'open' | 'won' | 'lost' | 'cancelled';
export type BetSide = 'YES' | 'NO';

export function parseSide(s: string): BetSide {
  const up = s.toUpperCase();
  if (up === 'YES' || up === 'NO') return up;
  throw new Error(`unknown side: ${s}`);
}

export function parseStatus(s: string): BetStatus {
  if (s === 'open' || s === 'won' || s === 'lost' || s === 'cancelled') return s;
  throw new Error(`unknown bet status: ${s}`);
}

export function sharesForSize(sizeUsdc: string, price: number): string {
  const n = Number(sizeUsdc);
  if (n > 0 && price > 0) return String(n / price);
  return '0';
}

export function pnlOnYes(side: BetSide, shares: number, price: number, sizeUsdc: number): number {
  return side === 'YES' ? shares * (1 - price) : -sizeUsdc;
}

export function pnlOnNo(side: BetSide, shares: number, price: number, sizeUsdc: number): number {
  return side === 'YES' ? -sizeUsdc : shares * price;
}

/** 算 PnL（USDC）。**L1 镜像 Rust 端 `domain::bet::pnl`**。
 *
 * **公式**：
 *   - outcome=YES, side=YES: `shares * (1 - price)`
 *   - outcome=YES, side=NO: `-size_usdc`
 *   - outcome=NO,  side=YES: `-size_usdc`
 *   - outcome=NO,  side=NO:  `shares * price`
 *
 * @param outcome — 市场 resolve 方向
 * @param side — 下单方向
 * @param shares — 持仓股数
 * @param price — 下单价
 * @param sizeUsdc — 下单金额（USDC）
 */
export function pnl(
  outcome: BetSide,
  side: BetSide,
  shares: number,
  price: number,
  sizeUsdc: number,
): number {
  return outcome === 'YES' ? pnlOnYes(side, shares, price, sizeUsdc) : pnlOnNo(side, shares, price, sizeUsdc);
}

export function isOpen(s: BetStatus): boolean {
  return s === 'open';
}
