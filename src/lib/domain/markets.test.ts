import { describe, it, expect } from 'vitest';
import {
  classifyCategory,
  parseLiquidity,
  parseVolume24h,
  hoursUntilClose,
  closesWithin,
  closingBucket,
  CATEGORY_LABELS,
} from './markets';

describe('classifyCategory', () => {
  it('classifies football via multiple triggers', () => {
    expect(classifyCategory('Will FC Barcelona win La Liga?')).toBe('football');
    expect(classifyCategory('Man City vs Liverpool — who wins?')).toBe('football');
    expect(classifyCategory('NFL season MVP?')).toBe('football');
  });
  it('classifies cs2', () => {
    expect(classifyCategory('Will NaVi win the BLAST Spring Final?')).toBe('cs2');
  });
  it('classifies politics', () => {
    expect(classifyCategory('Will Trump win the 2024 election?')).toBe('politics');
  });
  it('classifies crypto', () => {
    expect(classifyCategory('BTC > 100k by EOY?')).toBe('crypto');
  });
  it('classifies tech', () => {
    expect(classifyCategory('Will OpenAI release GPT-5?')).toBe('tech');
  });
  it('returns other for unrecognized', () => {
    expect(classifyCategory('Mystery box')).toBe('other');
  });
});

describe('parseLiquidity / parseVolume24h', () => {
  it('handles null and empty', () => {
    expect(parseLiquidity(null)).toBe(0);
    expect(parseLiquidity('')).toBe(0);
  });
  it('parses valid numbers', () => {
    expect(parseLiquidity('1234.5')).toBe(1234.5);
    expect(parseVolume24h('0')).toBe(0);
  });
  it('returns 0 for invalid', () => {
    expect(parseLiquidity('not-a-number')).toBe(0);
  });
});

describe('hoursUntilClose', () => {
  const now = 1_000_000_000_000;
  it('computes positive and negative hours', () => {
    expect(hoursUntilClose(now + 3_600_000, now)).toBe(1);
    expect(hoursUntilClose(now - 3_600_000, now)).toBe(-1);
  });
});

describe('closesWithin', () => {
  const now = 1_000_000_000_000;
  it('true when within horizon', () => {
    expect(closesWithin(now + 2 * 3_600_000, now, 24)).toBe(true);
  });
  it('false when outside horizon', () => {
    expect(closesWithin(now + 100 * 3_600_000, now, 24)).toBe(false);
  });
  it('false when already closed', () => {
    expect(closesWithin(now - 3_600_000, now, 24)).toBe(false);
  });
});

describe('closingBucket', () => {
  const now = 1_000_000_000_000;
  const h = (hours: number) => now + hours * 3_600_000;
  it('returns human labels', () => {
    expect(closingBucket(h(-2), now)).toBe('closed');
    expect(closingBucket(h(0), now)).toBe('< 1h');
    expect(closingBucket(h(5), now)).toBe('today');
    expect(closingBucket(h(72), now)).toBe('this week');
    expect(closingBucket(h(24 * 14), now)).toBe('this month');
    expect(closingBucket(h(24 * 90), now)).toBe('later');
  });
});

describe('CATEGORY_LABELS', () => {
  it('has labels for all 9 categories', () => {
    expect(Object.keys(CATEGORY_LABELS).length).toBe(9);
  });
});
