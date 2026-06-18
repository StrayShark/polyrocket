// v0.57d — env-file parsing tests.
//
// Covers the helper that reads .env / .key /
// .txt files and extracts a secret. The picker
// is the dialog plugin (v0.54a); the parser is
// pure (no IO), so we test it directly here.

import { describe, it, expect } from 'vitest';
import { extractSecretFromEnv } from './env-file';

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
