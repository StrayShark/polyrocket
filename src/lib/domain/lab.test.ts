import { describe, it, expect } from 'vitest';
import {
  parseRunStatus,
  canTransitionTo,
  validateVersion,
  isOlder,
  isBetter,
  VersionValidationError,
  type ModelPerf,
} from './lab';

describe('parseRunStatus', () => {
  it('round-trips valid statuses', () => {
    for (const s of ['queued', 'running', 'done', 'error'] as const) {
      expect(parseRunStatus(s)).toBe(s);
    }
  });
  it('returns null for garbage', () => {
    expect(parseRunStatus('garbage')).toBeNull();
  });
});

describe('canTransitionTo', () => {
  it('allows legal transitions', () => {
    expect(canTransitionTo('queued', 'running')).toBe(true);
    expect(canTransitionTo('running', 'done')).toBe(true);
    expect(canTransitionTo('running', 'error')).toBe(true);
  });
  it('blocks illegal', () => {
    expect(canTransitionTo('done', 'running')).toBe(false);
    expect(canTransitionTo('error', 'done')).toBe(false);
    expect(canTransitionTo('queued', 'done')).toBe(false);
  });
});

describe('validateVersion', () => {
  it('accepts canonical forms', () => {
    expect(() => validateVersion('v0.1.0')).not.toThrow();
    expect(() => validateVersion('v1.2')).not.toThrow();
    expect(() => validateVersion('v0.1.0-beta')).not.toThrow();
    expect(() => validateVersion('v10.20.30-rc1')).not.toThrow();
  });
  it('rejects malformed', () => {
    expect(() => validateVersion('0.1.0')).toThrow(VersionValidationError);
    expect(() => validateVersion('v1')).toThrow(VersionValidationError);
    expect(() => validateVersion('vX.Y.Z')).toThrow(VersionValidationError);
    expect(() => validateVersion('v1.0.0-')).toThrow(VersionValidationError);
  });
});

describe('isOlder', () => {
  it('compares correctly', () => {
    expect(isOlder('v0.1.0', 'v0.1.1')).toBe(true);
    expect(isOlder('v0.1.0', 'v0.2.0')).toBe(true);
    expect(isOlder('v0.9.9', 'v1.0.0')).toBe(true);
    expect(isOlder('v1.0.0', 'v0.9.9')).toBe(false);
    expect(isOlder('v0.1.0', 'v0.1.0')).toBe(false);
  });
});

describe('isBetter', () => {
  const base: ModelPerf = {
    modelVersion: 'v0',
    nPredictions: 100,
    winRate: 0.6,
    brierScore: 0.20,
    logLoss: 0.40,
    avgEdge: 0.05,
  };
  it('lower brier wins', () => {
    const a = { ...base, brierScore: 0.10 };
    expect(isBetter(a, base)).toBe(true);
    expect(isBetter(base, a)).toBe(false);
  });
  it('higher win rate is tiebreak', () => {
    const a = { ...base, brierScore: 0.20, winRate: 0.7 };
    expect(isBetter(a, base)).toBe(true);
  });
  it('more predictions is final tiebreak', () => {
    const a = { ...base, brierScore: 0.20, winRate: 0.6, nPredictions: 200 };
    expect(isBetter(a, base)).toBe(true);
  });
  it('identical returns false (no improvement)', () => {
    expect(isBetter({ ...base }, { ...base })).toBe(false);
  });
});
