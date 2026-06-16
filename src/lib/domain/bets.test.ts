import { describe, it, expect } from 'vitest';
import { sharesForSize, pnlOnYes, pnlOnNo, pnl, isOpen, parseSide, parseStatus } from './bets';

describe('parseSide / parseStatus', () => {
  it('parses valid sides', () => {
    expect(parseSide('YES')).toBe('YES');
    expect(parseSide('no')).toBe('NO');
  });
  it('rejects invalid', () => {
    expect(() => parseSide('maybe')).toThrow();
    expect(() => parseStatus('gone')).toThrow();
  });
});

describe('sharesForSize', () => {
  it('divides size by price', () => {
    expect(sharesForSize('100', 0.5)).toBe('200');
  });
  it('returns 0 for invalid inputs', () => {
    expect(sharesForSize('100', 0)).toBe('0');
    expect(sharesForSize('-5', 0.5)).toBe('0');
    expect(sharesForSize('abc', 0.5)).toBe('0');
  });
});

describe('pnlOnYes / pnlOnNo', () => {
  it('YES-bet wins on YES outcome', () => {
    // 100 USDC at price 0.40 → 250 shares → (1-0.40) * 250 = 150
    expect(pnlOnYes('YES', 250, 0.40, 100)).toBeCloseTo(150, 6);
  });
  it('YES-bet loses on NO outcome', () => {
    expect(pnlOnNo('YES', 250, 0.40, 100)).toBeCloseTo(-100, 6);
  });
  it('NO-bet wins on NO outcome', () => {
    // 100 USDC at price 0.60 → 100/0.60 shares → pnl = shares * 0.60 = 100
    const shares = 100 / 0.60;
    expect(pnlOnNo('NO', shares, 0.60, 100)).toBeCloseTo(100, 6);
  });
  it('NO-bet loses on YES outcome', () => {
    expect(pnlOnYes('NO', 200, 0.60, 100)).toBeCloseTo(-100, 6);
  });
});

describe('pnl dispatches on outcome', () => {
  it('YES outcome uses pnlOnYes', () => {
    expect(pnl('YES', 'YES', 250, 0.40, 100)).toBeCloseTo(150, 6);
  });
  it('NO outcome uses pnlOnNo', () => {
    expect(pnl('NO', 'NO', 100 / 0.6, 0.60, 100)).toBeCloseTo(100, 6);
  });
});

describe('isOpen', () => {
  it('true only for open status', () => {
    expect(isOpen('open')).toBe(true);
    expect(isOpen('won')).toBe(false);
    expect(isOpen('lost')).toBe(false);
    expect(isOpen('cancelled')).toBe(false);
  });
});
