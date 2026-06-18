// v0.57d — wallet JSON parsing tests.

import { describe, it, expect } from 'vitest';
import { extractAddressFromJson } from './wallet-file';

describe('extractAddressFromJson (v0.57d)', () => {
  it('parses frame-style {"address": "0x..."}', () => {
    const content = JSON.stringify({ address: '0xabc1234567890def1234567890abcdef12345678' });
    expect(extractAddressFromJson(content)).toBe('0xabc1234567890def1234567890abcdef12345678');
  });

  it('parses Rabby/MetaMask v3+ {"addr": "0x..."}', () => {
    const content = JSON.stringify({ addr: '0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef' });
    expect(extractAddressFromJson(content)).toBe('0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef');
  });

  it('parses WalletConnect-style {"accounts": ["0x..."]}', () => {
    const content = JSON.stringify({
      accounts: ['0xfeedfacefeedfacefeedfacefeedfacefeedface'],
    });
    expect(extractAddressFromJson(content)).toBe('0xfeedfacefeedfacefeedfacefeedfacefeedface');
  });

  it('walks nested objects', () => {
    const content = JSON.stringify({
      wallet: {
        info: { name: 'primary' },
        keys: {
          address: '0x1234567890123456789012345678901234567890',
        },
      },
    });
    expect(extractAddressFromJson(content)).toBe('0x1234567890123456789012345678901234567890');
  });

  it('falls back to regex when content is not JSON', () => {
    const content = `
      Some random text with a 0x prefix
      0xc0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ff
      and more text.
    `;
    expect(extractAddressFromJson(content)).toBe('0xc0ffeec0ffeec0ffeec0ffeec0ffeec0ffeec0ff');
  });

  it('returns null when no address is present', () => {
    expect(extractAddressFromJson(JSON.stringify({ name: 'no-addr' }))).toBe(null);
    expect(extractAddressFromJson('just text with no 0x prefix')).toBe(null);
  });

  it('returns null for short hex strings (not 40 chars)', () => {
    // The regex requires exactly 40 hex chars
    // after 0x. A short hex like 0xabc is
    // rejected.
    const content = JSON.stringify({ address: '0xabc' });
    expect(extractAddressFromJson(content)).toBe(null);
  });

  it('handles addresses in arrays', () => {
    const content = JSON.stringify({
      keys: [
        { addr: '0x' + 'a'.repeat(40) },
        { addr: '0x' + 'b'.repeat(40) },
      ],
    });
    expect(extractAddressFromJson(content)).toBe('0x' + 'a'.repeat(40));
  });

  it('is case-insensitive (uppercase hex)', () => {
    const content = JSON.stringify({ address: '0xABCDEF' + '0'.repeat(34) });
    expect(extractAddressFromJson(content)).toBe('0xABCDEF' + '0'.repeat(34));
  });
});
