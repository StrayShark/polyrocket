/**
 * L1 domain mirror — M9 Consensus.
 * Mirrors src-tauri/src/domain/consensus/mod.rs.
 */

export type ConsensusSide = 'YES' | 'NO' | 'MAYBE';

export function consensusSideAsString(s: ConsensusSide): string {
  return s;
}

export function parseConsensusSide(s: string): ConsensusSide | null {
  if (s === 'YES' || s === 'NO' || s === 'MAYBE') return s;
  return null;
}

export interface Consensus {
  side: ConsensusSide;
  /** 0..1, fraction of LLMs agreeing. */
  strength: number;
  nModels: number;
  avgConfidence: number;
}

/** Pick the majority side; strength = agreement fraction. */
export function consensusFrom(
  votes: Array<{ side: ConsensusSide; confidence: number }>,
): Consensus {
  if (votes.length === 0) {
    return { side: 'MAYBE', strength: 0, nModels: 0, avgConfidence: 0 };
  }
  const counts: Record<ConsensusSide, number> = { YES: 0, NO: 0, MAYBE: 0 };
  let totalConf = 0;
  for (const v of votes) {
    counts[v.side] += 1;
    totalConf += v.confidence;
  }
  const side = (Object.keys(counts) as ConsensusSide[]).reduce((a, b) =>
    counts[a] >= counts[b] ? a : b,
  );
  return {
    side,
    strength: counts[side] / votes.length,
    nModels: votes.length,
    avgConfidence: totalConf / votes.length,
  };
}
