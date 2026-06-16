import { describe, it, expect } from 'vitest';
import { winRate, brierScore, realizedPnl, categorize, summarize } from './pnl';

describe('winRate', () => {
  it('computes fraction', () => {
    expect(winRate(7, 3)).toBeCloseTo(0.7, 9);
  });
  it('returns 0 for no data', () => {
    expect(winRate(0, 0)).toBe(0);
  });
});

describe('brierScore', () => {
  it('perfect predictions → 0', () => {
    expect(brierScore([1, 0, 1, 0], [1, 0, 1, 0])).toBe(0);
  });
  it('worst case → 1', () => {
    expect(brierScore([1, 0], [0, 1])).toBe(1);
  });
  it('returns null on empty or mismatched', () => {
    expect(brierScore([], [])).toBeNull();
    expect(brierScore([0.5], [0.5, 1])).toBeNull();
  });
});

describe('realizedPnl', () => {
  it('sums parsed numbers, skips invalid', () => {
    expect(realizedPnl(['100', '-50', null, undefined, 'abc', '25'])).toBeCloseTo(75, 9);
  });
});

describe('categorize', () => {
  it('open stays open regardless of pnl', () => {
    expect(categorize('100', 'open')).toBe('open');
    expect(categorize(null, 'open')).toBe('open');
  });
  it('cancelled is other', () => {
    expect(categorize(null, 'cancelled')).toBe('other');
  });
  it('positive pnl is win, negative is loss', () => {
    expect(categorize('100', 'won')).toBe('win');
    expect(categorize('-50', 'lost')).toBe('loss');
  });
});

describe('summarize', () => {
  it('aggregates per category', () => {
    const r = summarize([
      ['100', 'won'],
      ['-40', 'lost'],
      ['60', 'won'],
      [null, 'open'],
    ]);
    expect(r.nTotal).toBe(4);
    expect(r.nWon).toBe(2);
    expect(r.nLost).toBe(1);
    expect(r.nOpen).toBe(1);
    expect(r.winRate).toBeCloseTo(2 / 3, 9);
    expect(r.totalPnl).toBeCloseTo(120, 9);
    expect(r.avgWin).toBeCloseTo(80, 9);
    expect(r.avgLoss).toBeCloseTo(-40, 9);
  });
  it('handles empty input', () => {
    const r = summarize([]);
    expect(r.nTotal).toBe(0);
    expect(r.winRate).toBe(0);
    expect(r.totalPnl).toBe(0);
  });
});
