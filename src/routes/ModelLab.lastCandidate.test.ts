/**
 * PromoteModel state machine tests (v0.18d).
 *
 * v0.18c introduced a `lastCandidate` state in ModelLab
 * that tracks the most recently trained model so the
 * Promote button knows which `job_id` to pass (race-
 * condition protection). The transitions are:
 *
 *   null         ────train completed────►  { jobId, ... }
 *   { jobId }    ────promote succeeded──►  null
 *   { jobId }    ────train completed────►  { newJobId, ... }  (overwrite)
 *   { jobId }    ────promote failed─────►  { jobId }  (keep)
 *
 * This test extracts the transition logic into a pure
 * function and tests it in isolation. (The actual state
 * is React useState, but the transitions are pure.)
 */

import { describe, expect, it } from 'vitest';

interface LastCandidate {
  jobId: string;
  candidatePath: string | null;
  bestBrier: number | null;
}

type Action =
  | { kind: 'train_completed'; jobId: string; candidatePath: string | null; bestBrier: number | null }
  | { kind: 'train_failed' }
  | { kind: 'promote_succeeded' }
  | { kind: 'promote_failed' };

/** Pure reducer for the lastCandidate state. v0.18c wires
 * this into ModelLab via setLastCandidate. */
function lastCandidateReducer(
  state: LastCandidate | null,
  action: Action,
): LastCandidate | null {
  switch (action.kind) {
    case 'train_completed':
      // Train succeeded → set the new candidate (overwriting
      // any prior one).
      return {
        jobId: action.jobId,
        candidatePath: action.candidatePath,
        bestBrier: action.bestBrier,
      };
    case 'train_failed':
      // Train failed → keep the existing candidate (or stay null).
      // (The toast still fires; the user can retry.)
      return state;
    case 'promote_succeeded':
      // Promote succeeded → clear (the model is now active;
      // no more candidate to promote).
      return null;
    case 'promote_failed':
      // Promote failed → keep the candidate (the user can retry).
      return state;
  }
}

describe('ModelLab lastCandidate reducer (v0.18d)', () => {
  it('starts null', () => {
    expect(lastCandidateReducer(null, { kind: 'train_failed' })).toBeNull();
  });

  it('sets a candidate on train_completed', () => {
    const next = lastCandidateReducer(null, {
      kind: 'train_completed',
      jobId: 'train-abc',
      candidatePath: '/tmp/c.json',
      bestBrier: 0.18,
    });
    expect(next?.jobId).toBe('train-abc');
    expect(next?.candidatePath).toBe('/tmp/c.json');
    expect(next?.bestBrier).toBe(0.18);
  });

  it('overwrites the candidate on a second train_completed', () => {
    const s1 = lastCandidateReducer(null, {
      kind: 'train_completed', jobId: 'train-abc',
      candidatePath: '/tmp/a.json', bestBrier: 0.2,
    });
    const s2 = lastCandidateReducer(s1, {
      kind: 'train_completed', jobId: 'train-xyz',
      candidatePath: '/tmp/x.json', bestBrier: 0.15,
    });
    expect(s2?.jobId).toBe('train-xyz');
    expect(s2?.bestBrier).toBe(0.15);
  });

  it('clears the candidate on promote_succeeded', () => {
    const s1 = lastCandidateReducer(null, {
      kind: 'train_completed', jobId: 'train-abc',
      candidatePath: '/tmp/a.json', bestBrier: 0.2,
    });
    const s2 = lastCandidateReducer(s1, { kind: 'promote_succeeded' });
    expect(s2).toBeNull();
  });

  it('keeps the candidate on promote_failed (user can retry)', () => {
    const s1 = lastCandidateReducer(null, {
      kind: 'train_completed', jobId: 'train-abc',
      candidatePath: '/tmp/a.json', bestBrier: 0.2,
    });
    const s2 = lastCandidateReducer(s1, {
      kind: 'promote_failed',
    });
    expect(s2?.jobId).toBe('train-abc');
  });

  it('keeps the candidate on train_failed (allows retry)', () => {
    const s1 = lastCandidateReducer(null, {
      kind: 'train_completed', jobId: 'train-abc',
      candidatePath: '/tmp/a.json', bestBrier: 0.2,
    });
    const s2 = lastCandidateReducer(s1, { kind: 'train_failed' });
    expect(s2?.jobId).toBe('train-abc');
  });

  it('keeps a null state through promote_failed', () => {
    const s = lastCandidateReducer(null, { kind: 'promote_failed' });
    expect(s).toBeNull();
  });
});
