import { describe, it, expect } from 'vitest';
import { consensusFrom, parseConsensusSide } from './consensus';

describe('parseConsensusSide', () => {
  it('parses valid sides', () => {
    expect(parseConsensusSide('YES')).toBe('YES');
    expect(parseConsensusSide('NO')).toBe('NO');
    expect(parseConsensusSide('MAYBE')).toBe('MAYBE');
  });
  it('returns null for invalid', () => {
    expect(parseConsensusSide('foo')).toBeNull();
  });
});

describe('consensusFrom', () => {
  it('empty input returns MAYBE with zero stats', () => {
    const r = consensusFrom([]);
    expect(r.side).toBe('MAYBE');
    expect(r.strength).toBe(0);
    expect(r.nModels).toBe(0);
    expect(r.avgConfidence).toBe(0);
  });
  it('picks majority side', () => {
    const r = consensusFrom([
      { side: 'YES', confidence: 0.8 },
      { side: 'YES', confidence: 0.7 },
      { side: 'NO', confidence: 0.6 },
    ]);
    expect(r.side).toBe('YES');
    expect(r.strength).toBeCloseTo(2 / 3, 9);
    expect(r.nModels).toBe(3);
    expect(r.avgConfidence).toBeCloseTo((0.8 + 0.7 + 0.6) / 3, 9);
  });
  it('tie goes to first seen in iteration order', () => {
    const r = consensusFrom([
      { side: 'YES', confidence: 0.5 },
      { side: 'NO', confidence: 0.5 },
    ]);
    expect(r.side).toBe('YES');
    expect(r.strength).toBe(0.5);
  });
});
