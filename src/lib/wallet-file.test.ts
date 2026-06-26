// v0.57d — 钱包 JSON 解析测试。

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
    // regex 要求 0x 后正好 40 个十六进制字符。
    // 像 0xabc 这种短十六进制会被拒绝。
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

  it('handles plain JSON string (parses to a string, not object) — v0.96', () => {
    // 某些钱包仅将 address 导出为顶层字符串。
    // 该字符串是合法的 JSON 值,但解析器需要
    // 处理「obj 是字符串」这一分支。
    const addr = '0x' + 'f'.repeat(40);
    expect(extractAddressFromJson(JSON.stringify(addr))).toBe(addr);
  });

  it('handles JSON null (parses to null, falls through to return null) — v0.96', () => {
    expect(extractAddressFromJson('null')).toBe(null);
  });

  it('handles JSON array at top level (walks array for address) — v0.96', () => {
    const addr = '0x' + 'a'.repeat(40);
    const content = JSON.stringify([addr]);
    expect(extractAddressFromJson(content)).toBe(addr);
  });

  it('handles custom field name (not in standard list) — v0.96', () => {
    // 「fallback walk」在标准字段名都不匹配时迭代 Object.values。
    // 自定义字段「myCustomKey」也应能被找到。
    const addr = '0x' + 'b'.repeat(40);
    const content = JSON.stringify({ myCustomKey: addr });
    expect(extractAddressFromJson(content)).toBe(addr);
  });
});
