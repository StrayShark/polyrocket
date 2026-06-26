// v0.57d — env-file 解析测试。
//
// 覆盖读取 .env / .key / .txt 文件并提取密钥的助手。
// 选择器是 dialog 插件（v0.54a）；解析器是
// 纯函数（无 IO），因此我们在此直接测试。

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { extractSecretFromEnv, readFileText } from './env-file';

// v0.97 — mock 掉 @tauri-apps/plugin-fs,以便 readFileText 内部的动态
// import 在测试中解析为已知实现。
const { mockReadTextFile } = vi.hoisted(() => ({
  mockReadTextFile: vi.fn(),
}));
vi.mock('@tauri-apps/plugin-fs', () => ({
  readTextFile: (...args: unknown[]) => mockReadTextFile(...args),
}));

describe('extractSecretFromEnv (v0.57d)', () => {
  it('extracts the value of a simple KEY=VALUE pair', () => {
    expect(extractSecretFromEnv('OPENAI_API_KEY=sk-abc123')).toBe('sk-abc123');
  });

  it('skips comments and empty lines', () => {
    const content = `
# This is a comment

ANTHROPIC_API_KEY=sk-ant-456

# Another comment
`;
    expect(extractSecretFromEnv(content)).toBe('sk-ant-456');
  });

  it('returns the first non-empty value', () => {
    const content = `
EMPTY=
FIRST=first-value
SECOND=second-value
`;
    expect(extractSecretFromEnv(content)).toBe('first-value');
  });

  it('strips surrounding double quotes', () => {
    expect(
      extractSecretFromEnv('OPENAI_API_KEY="sk-quoted"'),
    ).toBe('sk-quoted');
  });

  it('strips surrounding single quotes', () => {
    expect(
      extractSecretFromEnv("OPENAI_API_KEY='sk-single'"),
    ).toBe('sk-single');
  });

  it('treats a line without `=` as a plain key file', () => {
    // 常见形式:一行独立的 `sk-...`,
    // 没有 key=value。整行就是 secret。
    expect(extractSecretFromEnv('sk-plain-789')).toBe('sk-plain-789');
  });

  it('returns null for an empty file', () => {
    expect(extractSecretFromEnv('')).toBe(null);
  });

  it('returns null when all KEY=VALUE lines have empty values', () => {
    expect(extractSecretFromEnv('FOO=\nBAR=\n')).toBe(null);
  });

  it('handles \\r\\n line endings (Windows)', () => {
    const content = 'OPENAI_API_KEY=sk-crlf\r\nFOO=bar\r\n';
    expect(extractSecretFromEnv(content)).toBe('sk-crlf');
  });

  it('skips lines with only whitespace around KEY=VALUE', () => {
    expect(
      extractSecretFromEnv('   OPENAI_API_KEY   =   sk-trim  '),
    ).toBe('sk-trim');
  });
});

describe('readFileText (v0.97)', () => {
  beforeEach(() => {
    mockReadTextFile.mockReset();
  });

  it('reads via Tauri plugin when available', async () => {
    mockReadTextFile.mockResolvedValue('OPENAI_API_KEY=sk-tauri');
    const out = await readFileText('/path/to/file');
    expect(out).toBe('OPENAI_API_KEY=sk-tauri');
    expect(mockReadTextFile).toHaveBeenCalledWith('/path/to/file');
  });

  it('returns the content as a string', async () => {
    mockReadTextFile.mockResolvedValue('hello world');
    expect(await readFileText('/x')).toBe('hello world');
  });

  // v0.106 —— 覆盖率提升。覆盖 web-fetch 兜底路径（第 51-55 行）,
  // 即 Tauri plugin 抛错时的情况。
  it('falls back to fetch when Tauri plugin throws (covers lines 51-55)', async () => {
    mockReadTextFile.mockRejectedValue(new Error('not in Tauri'));
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      text: async () => 'OPENAI_API_KEY=sk-fetched',
    });
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mockFetch as unknown as typeof fetch;
    try {
      const out = await readFileText('/path/to/file');
      expect(out).toBe('OPENAI_API_KEY=sk-fetched');
      expect(mockFetch).toHaveBeenCalledWith('/path/to/file');
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it('fetch fallback throws when response not ok (covers line 52-54 error branch)', async () => {
    mockReadTextFile.mockRejectedValue(new Error('not in Tauri'));
    const mockFetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      text: async () => '',
    });
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mockFetch as unknown as typeof fetch;
    try {
      await expect(readFileText('/missing')).rejects.toThrow('HTTP 404');
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
