import { describe, it, expect } from 'vitest';
import {
  validateAddress,
  validateChain,
  validateLabel,
  shortAddress,
  parseWalletType,
  POLYGON_MAINNET,
  WalletValidationError,
} from './wallets';

describe('POLYGON_MAINNET', () => {
  it('is 137', () => {
    expect(POLYGON_MAINNET).toBe(137);
  });
});

describe('parseWalletType', () => {
  it('round-trip', () => {
    expect(parseWalletType('eoa')).toBe('eoa');
    expect(parseWalletType('smart')).toBe('smart');
  });
  it('rejects unknown', () => {
    expect(() => parseWalletType('multisig')).toThrow();
  });
});

describe('validateAddress', () => {
  it('accepts Vitalik address', () => {
    expect(() => validateAddress('0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045')).not.toThrow();
  });
  it('rejects no 0x prefix', () => {
    expect(() => validateAddress('d8dA6BF26964aF9D7eEd9e03E53415D37aA96045')).toThrow(WalletValidationError);
  });
  it('rejects wrong length', () => {
    expect(() => validateAddress('0x1234')).toThrow(WalletValidationError);
  });
  it('rejects non-hex', () => {
    expect(() => validateAddress('0xZZZZ6BF26964aF9D7eEd9e03E53415D37aA96045')).toThrow(WalletValidationError);
  });
});

describe('validateChain', () => {
  it('accepts mainnet and amoy', () => {
    expect(() => validateChain(137)).not.toThrow();
    expect(() => validateChain(80002)).not.toThrow();
  });
  it('rejects ethereum mainnet', () => {
    expect(() => validateChain(1)).toThrow(WalletValidationError);
  });
});

describe('validateLabel', () => {
  it('null/undefined OK', () => {
    expect(() => validateLabel(null)).not.toThrow();
    expect(() => validateLabel(undefined)).not.toThrow();
  });
  it('rejects empty', () => {
    expect(() => validateLabel('')).toThrow(WalletValidationError);
  });
  it('rejects too long', () => {
    expect(() => validateLabel('x'.repeat(65))).toThrow(WalletValidationError);
  });
});

describe('shortAddress', () => {
  it('formats standard address', () => {
    expect(shortAddress('0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045')).toBe('0xd8dA…6045');
  });
  it('returns short input as-is', () => {
    expect(shortAddress('0x123')).toBe('0x123');
  });
});
