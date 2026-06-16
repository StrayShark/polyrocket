import { describe, it, expect } from 'vitest';
import { score, isActionable, filterActive, sortByEdgeAbs, bestForMarket, stats, expireForClosedMarkets, type Signal } from './signals';

function sig(marketId: string, edge: number, conf: number): Signal {
  return {
    marketId,
    computedAt: 0,
    modelVersion: 'v0',
    predictedProb: 0.5 + edge,
    marketProb: 0.5,
    edge,
    confidence: conf,
    horizonHours: 24,
  };
}

describe('score', () => {
  it('computes edge and confidence', () => {
    const r = score(0.7, 0.5);
    expect(r.edge).toBeCloseTo(0.2, 9);
    expect(r.confidence).toBeCloseTo(0.4, 9);
  });
  it('caps confidence at 0.95', () => {
    expect(score(1.0, 0.0).confidence).toBeLessThanOrEqual(0.95);
  });
});

describe('isActionable', () => {
  it('needs both edge and confidence', () => {
    expect(isActionable(sig('m', 0.04, 0.9), 0.05)).toBe(false);
    expect(isActionable(sig('m', 0.10, 0.5), 0.05)).toBe(false);
    expect(isActionable(sig('m', 0.10, 0.6), 0.05)).toBe(true);
  });
});

describe('filterActive', () => {
  it('keeps actionable only', () => {
    const s = [sig('a', 0.03, 0.9), sig('b', 0.10, 0.6), sig('c', -0.20, 0.7)];
    const r = filterActive(s, 0.05);
    expect(r.length).toBe(2);
  });
});

describe('sortByEdgeAbs', () => {
  it('sorts descending by |edge|', () => {
    const r = sortByEdgeAbs([sig('a', 0.05, 0.7), sig('b', -0.20, 0.8), sig('c', 0.10, 0.7)]);
    expect(r[0].marketId).toBe('b');
    expect(r[1].marketId).toBe('c');
  });
  it('does not mutate the input', () => {
    const input = [sig('a', 0.05, 0.7), sig('b', 0.20, 0.8)];
    const inputCopy = [...input];
    sortByEdgeAbs(input);
    expect(input).toEqual(inputCopy);
  });
});

describe('bestForMarket', () => {
  it('picks the signal with largest |edge|', () => {
    const s = [sig('a', 0.05, 0.7), sig('a', 0.20, 0.7), sig('a', 0.10, 0.7)];
    const r = bestForMarket(s, 'a');
    expect(r?.edge).toBeCloseTo(0.20, 9);
  });
  it('returns undefined when no match', () => {
    expect(bestForMarket([sig('a', 0.1, 0.7)], 'b')).toBeUndefined();
  });
});

describe('stats', () => {
  it('aggregates bullish / bearish / edges', () => {
    const s = [sig('a', 0.10, 0.7), sig('b', -0.20, 0.7), sig('c', 0.30, 0.7)];
    const r = stats(s);
    expect(r.n).toBe(3);
    expect(r.bullish).toBe(2);
    expect(r.bearish).toBe(1);
    expect(r.avgAbsEdge).toBeCloseTo(0.20, 9);
    expect(r.maxAbsEdge).toBeCloseTo(0.30, 9);
  });
  it('empty input', () => {
    expect(stats([])).toEqual({ n: 0, avgAbsEdge: 0, maxAbsEdge: 0, bullish: 0, bearish: 0 });
  });
});

describe('expireForClosedMarkets', () => {
  it('returns market ids whose computed+horizon is in the past', () => {
    const r = expireForClosedMarkets([sig('a', 0.05, 0.7)], 100 * 3_600_000);
    expect(r).toEqual(['a']);
  });
});
