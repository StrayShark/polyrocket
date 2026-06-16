/**
 * L1 domain mirror — M2 Signals.
 * Mirrors src-tauri/src/domain/signal/mod.rs.
 */

export interface Signal {
  marketId: string;
  computedAt: number;
  modelVersion: string;
  predictedProb: number;
  marketProb: number;
  edge: number;
  confidence: number;
  horizonHours: number;
  rationale?: string | null;
}

export function score(predictedProb: number, marketProb: number): { edge: number; confidence: number } {
  const edge = predictedProb - marketProb;
  const confidence = Math.min(0.95, Math.abs(edge) * 2);
  return { edge, confidence };
}

export function isActionable(s: Signal, minEdge: number): boolean {
  return Math.abs(s.edge) >= minEdge && s.confidence >= 0.6;
}

export function filterActive(signals: Signal[], minEdge: number): Signal[] {
  return signals.filter((s) => isActionable(s, minEdge));
}

export function sortByEdgeAbs(signals: Signal[]): Signal[] {
  return [...signals].sort((a, b) => Math.abs(b.edge) - Math.abs(a.edge));
}

export function bestForMarket(signals: Signal[], marketId: string): Signal | undefined {
  return signals
    .filter((s) => s.marketId === marketId)
    .reduce<Signal | undefined>(
      (best, s) => (best === undefined || Math.abs(s.edge) > Math.abs(best.edge) ? s : best),
      undefined,
    );
}

export interface SignalStats {
  n: number;
  avgAbsEdge: number;
  maxAbsEdge: number;
  bullish: number;
  bearish: number;
}

export function stats(signals: Signal[]): SignalStats {
  if (signals.length === 0) {
    return { n: 0, avgAbsEdge: 0, maxAbsEdge: 0, bullish: 0, bearish: 0 };
  }
  let sum = 0;
  let max = 0;
  let bull = 0;
  let bear = 0;
  for (const s of signals) {
    const a = Math.abs(s.edge);
    sum += a;
    if (a > max) max = a;
    if (s.edge > 0) bull += 1;
    if (s.edge < 0) bear += 1;
  }
  return {
    n: signals.length,
    avgAbsEdge: sum / signals.length,
    maxAbsEdge: max,
    bullish: bull,
    bearish: bear,
  };
}

export function expireForClosedMarkets(signals: Signal[], nowMs: number): string[] {
  return signals
    .filter((s) => {
      const end = s.horizonHours * 3_600_000 + s.computedAt;
      return end - nowMs < 0;
    })
    .map((s) => s.marketId);
}
