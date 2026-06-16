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
