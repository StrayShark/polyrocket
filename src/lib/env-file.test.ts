// v0.57d — env-file parsing tests.
//
// Covers the helper that reads .env / .key /
// .txt files and extracts a secret. The picker
// is the dialog plugin (v0.54a); the parser is
// pure (no IO), so we test it directly here.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { extractSecretFromEnv, readFileText } from './env-file';

// v0.97 — mock @tauri-apps/plugin-fs so the dynamic import
// inside readFileText resolves to a known impl in tests.
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
    // Common shape: a `sk-...` line on its own
    // with no key=value. The whole line is the
    // secret.
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
});
