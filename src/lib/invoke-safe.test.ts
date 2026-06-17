import { describe, expect, it } from 'vitest';
import { classifyError, canRetry, formatError, type AppErrorShape } from './invoke-safe';

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
