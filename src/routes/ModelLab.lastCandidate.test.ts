/**
 * PromoteModel 状态机测试（v0.18d）。
 *
 * v0.18c 在 ModelLab 中引入了 `lastCandidate` 状态，用于
 * 跟踪最近一次训练完成的模型，以便 Promote 按钮知道要传
 * 哪个 `job_id`（竞态保护）。状态转移为：
 *
 *   null         ────train completed────►  { jobId, ... }
 *   { jobId }    ────promote succeeded──►  null
 *   { jobId }    ────train completed────►  { newJobId, ... }  (overwrite)
 *   { jobId }    ────promote failed─────►  { jobId }  (keep)
 *
 * 本测试将状态转移逻辑提取为纯函数并隔离测试。
 * （实际状态是 React useState，但转移本身是纯函数。）
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

/** lastCandidate 状态的纯 reducer。v0.18c 通过 setLastCandidate
 * 将其接入 ModelLab。 */
function lastCandidateReducer(
  state: LastCandidate | null,
  action: Action,
): LastCandidate | null {
  switch (action.kind) {
    case 'train_completed':
      // 训练成功 → 设置新的 candidate（覆盖之前任一 candidate）。
      return {
        jobId: action.jobId,
        candidatePath: action.candidatePath,
        bestBrier: action.bestBrier,
      };
    case 'train_failed':
      // 训练失败 → 保留已有 candidate（或维持 null）。
      // （toast 仍会触发；用户可重试。）
      return state;
    case 'promote_succeeded':
      // Promote 成功 → 清空（模型现已激活；
      // 没有 candidate 可再 promote）。
      return null;
    case 'promote_failed':
      // Promote 失败 → 保留 candidate（用户可重试）。
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
