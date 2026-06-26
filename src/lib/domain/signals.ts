/**
 * L1 领域镜像 —— M2 信号。
 * 镜像 src-tauri/src/domain/signal/mod.rs。
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

/** 算 (edge, confidence) 对。**L1 镜像 Rust 端 `domain::signal::score`**。
 *
 * **为什么 confidence 用 `min(|edge| * 2, 0.95)`**：`confidence` 应该跟 `edge` 强相关
 * （大 edge = 高确信）。`* 2` 是个启发式，cap 在 0.95 避免「100% 确信」假象。
 * **真正的不确定性** 应该由 model 直接输出（v0.55+ 替换）。
 */
export function score(predictedProb: number, marketProb: number): { edge: number; confidence: number } {
  const edge = predictedProb - marketProb;
  const confidence = Math.min(0.95, Math.abs(edge) * 2);
  return { edge, confidence };
}

/** 判断 signal 是否可执行。**L1 镜像 Rust 端 `Signal::is_actionable`**。
 *
 * **两条条件**：`|edge| >= minEdge` AND `confidence >= 0.6`。
 * L1 「Signals」页面过滤「actionable」信号用。
 */
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

/** 算一批 signal 的统计：n / avg|edge| / max|edge| / bullish / bearish。
 *
 * **调用方**：L1 「Signals」页面顶部 summary 卡。
 * **空数组**：返回所有 0。
 */
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
