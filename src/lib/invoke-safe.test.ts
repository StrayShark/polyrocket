import { describe, expect, it, vi, beforeEach } from 'vitest';
import { classifyError, canRetry, formatError, safeInvoke, type AppErrorShape } from './invoke-safe';

// v0.97 — mock 掉 @tauri-apps/api/core 以便测试 safeInvoke
const { mockTauriInvoke } = vi.hoisted(() => ({
  mockTauriInvoke: vi.fn(),
}));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockTauriInvoke(...args),
}));

describe('classifyError', () => {
  it('classifies db errors', () => {
    expect(classifyError('database error: connection refused').kind).toBe('db');
    expect(classifyError('sqlite: no such table').kind).toBe('db');
  });

  it('classifies http errors', () => {
    expect(classifyError('http error: timeout').kind).toBe('http');
    expect(classifyError('reqwest error: connection refused').kind).toBe('http');
  });

  it('classifies serde errors', () => {
    expect(classifyError('serde error: missing field').kind).toBe('serde');
  });

  it('classifies keyring errors', () => {
    expect(classifyError('keyring error: entry not found').kind).toBe('keyring');
  });

  it('classifies io errors', () => {
    expect(classifyError('io error: permission denied').kind).toBe('io');
  });

  it('classifies invalid errors', () => {
    expect(classifyError('invalid input: market_id is required').kind).toBe('invalid');
  });

  it('classifies not_found errors', () => {
    expect(classifyError('not found: wallet wlt-xxx').kind).toBe('not_found');
  });

  it('classifies internal errors', () => {
    expect(classifyError('internal: something broke').kind).toBe('internal');
  });

  it('classifies unknown errors', () => {
    expect(classifyError('weird random message').kind).toBe('unknown');
  });

  it('preserves the raw message', () => {
    const e = classifyError('invalid input: foo');
    expect(e.raw).toBe('invalid input: foo');
    expect(e.message).toBe('invalid input: foo');
  });

  it('attaches a non-empty hint', () => {
    const e = classifyError('db error: x');
    expect(e.hint.length).toBeGreaterThan(10);
  });
});

describe('canRetry', () => {
  it.each<[AppErrorShape['kind'], boolean]>([
    ['db', true],
    ['http', true],
    ['serde', false],
    ['keyring', false],
    ['io', true],
    ['invalid', false],
    ['not_found', false],
    ['internal', false],
    ['network', true],
    ['unknown', true],
  ])('kind=%s retryable=%s', (kind, expected) => {
    const e: AppErrorShape = { kind, message: 'x', hint: 'y', retryable: expected, raw: 'x' };
    expect(canRetry(e)).toBe(expected);
  });
});

describe('formatError', () => {
  it('prefixes kind to message', () => {
    const e = classifyError('invalid input: foo');
    const formatted = formatError(e);
    expect(formatted).toBe('invalid: invalid input: foo');
  });
});

describe('network error classification (v0.97)', () => {
  it('classifies "window not found" as network', () => {
    expect(classifyError('window not found: w-123').kind).toBe('network');
  });

  it('classifies "ipc failed" as network', () => {
    expect(classifyError('ipc failed: connection closed').kind).toBe('network');
  });
});

describe('safeInvoke (v0.97)', () => {
  beforeEach(() => {
    mockTauriInvoke.mockReset();
  });

  it('returns the result on success', async () => {
    mockTauriInvoke.mockResolvedValue({ ok: true, items: [1, 2, 3] });
    const out = await safeInvoke<{ ok: boolean; items: number[] }>('list_things', { x: 1 });
    expect(out).toEqual({ ok: true, items: [1, 2, 3] });
    expect(mockTauriInvoke).toHaveBeenCalledWith('list_things', { x: 1 });
  });

  it('throws an AppErrorShape on failure', async () => {
    mockTauriInvoke.mockRejectedValue(new Error('invalid input: missing field'));
    await expect(safeInvoke('cmd', {})).rejects.toMatchObject({
      kind: 'invalid',
      message: 'invalid input: missing field',
      retryable: false,
    });
  });

  it('uses empty args object when args is undefined', async () => {
    mockTauriInvoke.mockResolvedValue('ok');
    await safeInvoke('cmd');
    expect(mockTauriInvoke).toHaveBeenCalledWith('cmd', {});
  });

  it('classifies non-Error throws as their String() value', async () => {
    mockTauriInvoke.mockRejectedValue('database error: x');
    await expect(safeInvoke('cmd', {})).rejects.toMatchObject({
      kind: 'db',
      message: 'database error: x',
    });
  });
});
