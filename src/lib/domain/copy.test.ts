import { describe, it, expect } from 'vitest';
import { shouldMirror, isDuplicateTx, validateTargetArgs, CopyValidationError, type CopyTargetInput, type CopyEventInput } from './copy';

function target(enabled: boolean, minEdge: number, cap: string | null): CopyTargetInput {
  return {
    id: 't1',
    address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
    label: null,
    enabled,
    allocationCap: cap,
    minEdge,
    createdAt: 0,
  };
}

function ev(txHash: string): CopyEventInput {
  return {
    id: 1,
    targetId: 't1',
    marketId: 'm1',
    detectedAt: 0,
    side: 'YES',
    size: '100',
    price: 0.5,
    txHash,
    matchedBetId: null,
  };
}

describe('validateTargetArgs', () => {
  it('rejects bad address', () => {
    expect(() => validateTargetArgs('not-an-address')).toThrow(CopyValidationError);
  });
  it('rejects edge out of range', () => {
    const addr = '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045';
    expect(() => validateTargetArgs(addr, 1.5)).toThrow();
    expect(() => validateTargetArgs(addr, -0.1)).toThrow();
  });
  it('rejects negative cap', () => {
    const addr = '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045';
    expect(() => validateTargetArgs(addr, undefined, '-5')).toThrow();
  });
  it('accepts valid', () => {
    const addr = '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045';
    expect(() => validateTargetArgs(addr, 0.05, '100')).not.toThrow();
    expect(() => validateTargetArgs(addr, undefined, undefined)).not.toThrow();
  });
});

describe('shouldMirror', () => {
  it('returns null when target disabled', () => {
    expect(shouldMirror(target(false, 0.05, null), 'YES', '100', 0.10)).toBeNull();
  });
  it('returns null when edge too small', () => {
    expect(shouldMirror(target(true, 0.05, null), 'YES', '100', 0.02)).toBeNull();
  });
  it('returns null for zero size', () => {
    expect(shouldMirror(target(true, 0, null), 'YES', '0', 0.10)).toBeNull();
  });
  it('caps the size', () => {
    const r = shouldMirror(target(true, 0.05, '50'), 'YES', '100', 0.10);
    expect(r?.size).toBe('50');
    expect(r?.flip).toBe(false);
  });
  it('detects direction disagreement', () => {
    const r = shouldMirror(target(true, 0.05, null), 'YES', '100', -0.10);
    expect(r?.side).toBe('NO');
    expect(r?.flip).toBe(true);
  });
});

describe('isDuplicateTx', () => {
  it('detects a duplicate', () => {
    expect(isDuplicateTx([ev('0xabc')], '0xabc')).toBe(true);
    expect(isDuplicateTx([ev('0xabc')], '0xdef')).toBe(false);
    expect(isDuplicateTx([], '0xabc')).toBe(false);
  });
});
