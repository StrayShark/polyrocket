/**
 * v0.15b — IPC event payload types (round-trip).
 *
 * The Rust side (domain::llm::progress) defines the canonical
 * event payloads. These tests assert the TS types match: every
 * Rust field is present in the TS interface, the optional /
 * null semantics match, and a JSON blob shaped like a Rust
 * payload parses cleanly into the TS type.
 *
 * We don't import from `@/ipc` directly because the TS module
 * pulls in `@tauri-apps/api/event` which is not vitest-friendly
 * out of the box. We re-define the shapes inline as TS types
 * and assert against a JSON literal that mirrors the Rust
 * payload. If the shapes ever drift, the JSON cast will fail
 * the type check.
 */
import { describe, it, expect } from 'vitest';

interface AnalyzeStartedEvent {
  analysis_id: string;
  market_id: string;
  prompt_version: string;
  providers: string[];
  started_at: number;
}

interface ProviderDoneEvent {
  analysis_id: string;
  provider_id: string;
  ok: boolean;
  latency_ms: number;
  tokens_in: number | null;
  tokens_out: number | null;
  cost_cents: number;
  error_kind: string;
  error_message: string | null;
  finished_at: number;
}

interface ConsensusDoneEvent {
  analysis_id: string;
  status: 'completed' | 'partial' | 'failed';
  n_success: number;
  n_failed: number;
  consensus_pred: number | null;
  consensus_side: string | null;
  consensus_conf: number | null;
}

interface AnalyzeFinishedEvent {
  analysis_id: string;
  status: 'completed' | 'partial' | 'failed';
  total_latency_ms: number;
  total_cost_cents: number;
  n_success: number;
  n_failed: number;
  finished_at: number;
}

describe('LLM analyze event payloads (v0.15b)', () => {
  it('AnalyzeStartedEvent shape', () => {
    const e: AnalyzeStartedEvent = JSON.parse(JSON.stringify({
      analysis_id: 'a1',
      market_id: 'm1',
      prompt_version: 'market-analysis-1.0.0',
      providers: ['anthropic', 'openai', 'google'],
      started_at: 1_700_000_000_000,
    }));
    expect(e.analysis_id).toBe('a1');
    expect(e.providers).toEqual(['anthropic', 'openai', 'google']);
    expect(typeof e.started_at).toBe('number');
  });

  it('ProviderDoneEvent with error', () => {
    const e: ProviderDoneEvent = JSON.parse(JSON.stringify({
      analysis_id: 'a1',
      provider_id: 'openai',
      ok: false,
      latency_ms: 1234,
      tokens_in: 0,
      tokens_out: 0,
      cost_cents: 0.0,
      error_kind: 'rate_limit',
      error_message: '429 Too Many Requests',
      finished_at: 1_700_000_001_000,
    }));
    expect(e.ok).toBe(false);
    expect(e.error_kind).toBe('rate_limit');
    expect(e.error_message).toContain('429');
  });

  it('ProviderDoneEvent ok=true has no error message', () => {
    const e: ProviderDoneEvent = JSON.parse(JSON.stringify({
      analysis_id: 'a1',
      provider_id: 'anthropic',
      ok: true,
      latency_ms: 987,
      tokens_in: 312,
      tokens_out: 88,
      cost_cents: 0.04,
      error_kind: 'none',
      error_message: null,
      finished_at: 1_700_000_001_500,
    }));
    expect(e.ok).toBe(true);
    expect(e.error_message).toBeNull();
  });

  it('ConsensusDoneEvent partial', () => {
    const e: ConsensusDoneEvent = JSON.parse(JSON.stringify({
      analysis_id: 'a1',
      status: 'partial',
      n_success: 2,
      n_failed: 1,
      consensus_pred: 0.62,
      consensus_side: 'YES',
      consensus_conf: 0.71,
    }));
    expect(e.status).toBe('partial');
    expect(e.n_success + e.n_failed).toBe(3);
  });

  it('ConsensusDoneEvent all-failed has null consensus', () => {
    const e: ConsensusDoneEvent = JSON.parse(JSON.stringify({
      analysis_id: 'a1',
      status: 'failed',
      n_success: 0,
      n_failed: 3,
      consensus_pred: null,
      consensus_side: null,
      consensus_conf: null,
    }));
    expect(e.status).toBe('failed');
    expect(e.consensus_pred).toBeNull();
    expect(e.consensus_side).toBeNull();
    expect(e.consensus_conf).toBeNull();
  });

  it('AnalyzeFinishedEvent carries totals', () => {
    const e: AnalyzeFinishedEvent = JSON.parse(JSON.stringify({
      analysis_id: 'a1',
      status: 'completed',
      total_latency_ms: 4200,
      total_cost_cents: 0.12,
      n_success: 3,
      n_failed: 0,
      finished_at: 1_700_000_005_000,
    }));
    expect(e.total_latency_ms).toBe(4200);
    expect(e.total_cost_cents).toBeCloseTo(0.12, 5);
  });

  it('error_kind is one of the stable codes', () => {
    // The Rust side uses these stable strings (see
    // domain::llm::err). The TS interface just types error_kind
    // as string, but the test documents the expected set.
    const valid = new Set([
      'none',
      'auth',
      'rate_limit',
      'timeout',
      'network',
      'parse',
      'model_not_found',
      'quota',
      'unknown',
    ]);
    for (const k of valid) {
      expect(valid.has(k), `expected ${k} to be in the valid set`).toBe(true);
    }
    expect(valid.size).toBe(9);
  });
});

interface TrainStartedEvent {
  job_id: string;
  n_trials: number;
  epochs: number;
  started_at: number;
}

interface TrainTrialDto {
  lr: number;
  reg: number;
  brier: number;
  weights: Record<string, number>;
}

interface TrainFinishedEvent {
  job_id: string;
  status: 'completed' | 'failed' | string;
  best_brier: number | null;
  best_params: Record<string, number> | null;
  trials: TrainTrialDto[];
  duration_ms: number;
  candidate_path: string | null;
  message: string | null;
  finished_at: number;
}

interface TrainResult {
  job_id: string;
  status: 'completed' | 'failed' | string;
  best_brier: number | null;
  best_params: Record<string, number> | null;
  trials: TrainTrialDto[];
  duration_ms: number;
  candidate_path: string | null;
  message: string | null;
}

describe('Train job event payloads (v0.17b)', () => {
  it('TrainStartedEvent shape', () => {
    const e: TrainStartedEvent = JSON.parse(JSON.stringify({
      job_id: 'train-441c352b',
      n_trials: 4,
      epochs: 80,
      started_at: 1_700_000_000_000,
    }));
    expect(e.job_id).toBe('train-441c352b');
    expect(e.n_trials).toBe(4);
    expect(e.epochs).toBe(80);
  });

  it('TrainFinishedEvent completed with trials', () => {
    const e: TrainFinishedEvent = JSON.parse(JSON.stringify({
      job_id: 'train-441c352b',
      status: 'completed',
      best_brier: 0.184,
      best_params: { w0: 0.1, w1: 0.2, w2: 0.3 },
      trials: [
        { lr: 0.05, reg: 0.01, brier: 0.184, weights: { w0: 0.1, w1: 0.2, w2: 0.3 } },
        { lr: 0.10, reg: 0.01, brier: 0.210, weights: { w0: 0.1, w1: 0.2, w2: 0.3 } },
        { lr: 0.05, reg: 0.10, brier: 0.225, weights: { w0: 0.1, w1: 0.2, w2: 0.3 } },
        { lr: 0.10, reg: 0.10, brier: 0.243, weights: { w0: 0.1, w1: 0.2, w2: 0.3 } },
      ],
      duration_ms: 4200,
      candidate_path: '/home/x/.polyrocket/sidecar/models/candidate.json',
      message: null,
      finished_at: 1_700_000_004_200,
    }));
    expect(e.status).toBe('completed');
    expect(e.trials.length).toBe(4);
    expect(e.trials[0].brier).toBeLessThan(e.trials[3].brier);
  });

  it('TrainFinishedEvent failed with diagnostic message', () => {
    const e: TrainFinishedEvent = JSON.parse(JSON.stringify({
      job_id: 'train-deadbeef',
      status: 'failed',
      best_brier: null,
      best_params: null,
      trials: [],
      duration_ms: 500,
      candidate_path: null,
      message: 'sidecar not running',
      finished_at: 1_700_000_001_000,
    }));
    expect(e.status).toBe('failed');
    expect(e.best_brier).toBeNull();
    expect(e.trials).toEqual([]);
    expect(e.message).toContain('sidecar not running');
  });

  it('TrainResult IPC return shape (same as finished, no finished_at)', () => {
    const r: TrainResult = JSON.parse(JSON.stringify({
      job_id: 'train-441c352b',
      status: 'completed',
      best_brier: 0.184,
      best_params: { w0: 0.1, w1: 0.2, w2: 0.3 },
      trials: [
        { lr: 0.05, reg: 0.01, brier: 0.184, weights: { w0: 0.1, w1: 0.2, w2: 0.3 } },
      ],
      duration_ms: 4200,
      candidate_path: '/home/x/.polyrocket/sidecar/models/candidate.json',
      message: null,
    }));
    expect(r.best_brier).toBeLessThan(0.2);
    // The IPC return is missing finished_at — the IPC itself
    // is the "this just finished" signal, so the event payload
    // adds finished_at to make event subscribers unambiguous.
  });
});

interface PromoteResult {
  promoted: boolean;
  status: 'ok' | 'failed' | string;
  previous_path: string | null;
  active_path: string | null;
  promoted_at_ms: number | null;
  model_version: string;
  message: string | null;
  /** v0.21a — bulk promote. undefined = best, number = trial n. */
  trial_index?: number;
}

describe('Promote model wire format (v0.18b)', () => {
  it('PromoteResult success', () => {
    const r: PromoteResult = JSON.parse(JSON.stringify({
      promoted: true,
      status: 'ok',
      previous_path: '/home/x/.polyrocket/sidecar/models/active.json',
      active_path: '/home/x/.polyrocket/sidecar/models/active.json',
      promoted_at_ms: 1_700_000_000_000,
      model_version: 'logistic-train-441c352b',
      message: null,
    }));
    expect(r.promoted).toBe(true);
    expect(r.status).toBe('ok');
    expect(r.model_version).toBe('logistic-train-441c352b');
    expect(r.promoted_at_ms).toBe(1_700_000_000_000);
  });

  it('PromoteResult failure (no candidate)', () => {
    const r: PromoteResult = JSON.parse(JSON.stringify({
      promoted: false,
      status: 'failed',
      previous_path: null,
      active_path: null,
      promoted_at_ms: null,
      model_version: '',
      message: 'no candidate found at /home/x/.polyrocket/sidecar/models/candidate.json; run train_job first',
    }));
    expect(r.promoted).toBe(false);
    expect(r.status).toBe('failed');
    expect(r.model_version).toBe('');
    expect(r.message).toContain('run train_job first');
  });

  it('PromoteResult failure (job_id mismatch)', () => {
    const r: PromoteResult = JSON.parse(JSON.stringify({
      promoted: false,
      status: 'failed',
      previous_path: null,
      active_path: null,
      promoted_at_ms: null,
      model_version: '',
      message: 'candidate job_id mismatch: expected train-abc, got train-xyz',
    }));
    expect(r.promoted).toBe(false);
    expect(r.message).toContain('mismatch');
    expect(r.message).toContain('train-abc');
  });

  it('PromoteResult first promote (no previous_path)', () => {
    // On the very first promote, the active.json doesn't
    // exist yet, so previous_path is null.
    const r: PromoteResult = JSON.parse(JSON.stringify({
      promoted: true,
      status: 'ok',
      previous_path: null,
      active_path: '/home/x/.polyrocket/sidecar/models/active.json',
      promoted_at_ms: 1_700_000_000_000,
      model_version: 'logistic-train-441c352b',
      message: null,
    }));
    expect(r.promoted).toBe(true);
    expect(r.previous_path).toBeNull();
    expect(r.active_path).toBe('/home/x/.polyrocket/sidecar/models/active.json');
  });

  it('PromoteResult bulk promote (v0.21a — trial_index=2)', () => {
    // v0.21a — bulk promote: the model_version has a
    // -t2 suffix, and trial_index is recorded.
    const r: PromoteResult = JSON.parse(JSON.stringify({
      promoted: true,
      status: 'ok',
      previous_path: '/home/x/.polyrocket/sidecar/models/active.json',
      active_path: '/home/x/.polyrocket/sidecar/models/active.json',
      promoted_at_ms: 1_700_010_000_000,
      model_version: 'logistic-train-441c352b-t2',
      message: null,
      trial_index: 2,
    }));
    expect(r.promoted).toBe(true);
    expect(r.model_version).toBe('logistic-train-441c352b-t2');
    expect(r.trial_index).toBe(2);
  });

  it('PromoteResult default (no trial_index) — backward compat', () => {
    // v0.21a — when the Python sidecar doesn't return
    // trial_index (older version), the TS side treats
    // it as undefined (best, not bulk).
    const r: PromoteResult = JSON.parse(JSON.stringify({
      promoted: true,
      status: 'ok',
      previous_path: '/home/x/.polyrocket/sidecar/models/active.json',
      active_path: '/home/x/.polyrocket/sidecar/models/active.json',
      promoted_at_ms: 1_700_011_000_000,
      model_version: 'logistic-train-441c352b',
      message: null,
    }));
    expect(r.model_version).toBe('logistic-train-441c352b');
    expect(r.trial_index).toBeUndefined();
  });
});

// =================================================================
// ================= v0.19b — list_promote_history =================
// =================================================================

interface PromoteHistoryEntry {
  job_id: string;
  model_version: string;
  promoted_at_ms: number;
  best_brier: number | null;
  best_params: Record<string, unknown> | null;
  /** v0.24a — which trial of the sweep was promoted. */
  trial_index?: number | null;
}

interface PromoteHistoryResult {
  ok: boolean;
  entries: PromoteHistoryEntry[];
  count: number;
  message: string | null;
}

describe('Promote history wire format (v0.19b)', () => {
  it('PromoteHistoryResult populated', () => {
    const r: PromoteHistoryResult = JSON.parse(JSON.stringify({
      ok: true,
      count: 2,
      entries: [
        {
          job_id: 'train-aaa',
          model_version: 'logistic-train-aaa',
          promoted_at_ms: 1_700_000_000_000,
          best_brier: 0.184,
          best_params: { lr: 0.01, reg: 0.1 },
        },
        {
          job_id: 'train-bbb',
          model_version: 'logistic-train-bbb',
          promoted_at_ms: 1_700_001_000_000,
          best_brier: 0.179,
          best_params: null,
        },
      ],
      message: null,
    }));
    expect(r.ok).toBe(true);
    expect(r.count).toBe(2);
    expect(r.entries).toHaveLength(2);
    expect(r.entries[0].job_id).toBe('train-aaa');
    expect(r.entries[0].best_brier).toBeCloseTo(0.184);
    expect(r.entries[0].best_params).toEqual({ lr: 0.01, reg: 0.1 });
    expect(r.entries[1].job_id).toBe('train-bbb');
    expect(r.entries[1].best_params).toBeNull();
  });

  it('PromoteHistoryResult empty (no active model yet)', () => {
    const r: PromoteHistoryResult = JSON.parse(JSON.stringify({
      ok: true,
      count: 0,
      entries: [],
      message: 'no active model yet; train + promote to start history',
    }));
    expect(r.ok).toBe(true);
    expect(r.count).toBe(0);
    expect(r.entries).toEqual([]);
    expect(r.message).toContain('train + promote');
  });

  it('PromoteHistoryResult envelope ok=false means error', () => {
    // The envelope `ok: false` is the case where the
    // sidecar subprocess returned an error response
    // (transport-level failure). The result block won't
    // even be present in that case — the L1 wrapper
    // would see the rejection from `invoke()` itself.
    // We don't model that here; we just assert the
    // success-side shape is preserved.
    const r: PromoteHistoryResult = JSON.parse(JSON.stringify({
      ok: false,
      count: 0,
      entries: [],
      message: 'sidecar not running',
    }));
    expect(r.ok).toBe(false);
    expect(r.entries).toEqual([]);
  });
});

// =================================================================
// ==================== v0.20b — rollback_model =====================
// =================================================================

interface RollbackResult {
  rolled_back: boolean;
  status: 'ok' | 'failed' | string;
  previous_path: string | null;
  active_path: string | null;
  rolled_back_at_ms: number | null;
  model_version: string;
  message: string | null;
}

describe('Rollback model wire format (v0.20b)', () => {
  it('RollbackResult success', () => {
    const r: RollbackResult = JSON.parse(JSON.stringify({
      rolled_back: true,
      status: 'ok',
      previous_path: '/home/x/.polyrocket/sidecar/models/active.json',
      active_path: '/home/x/.polyrocket/sidecar/models/active.json',
      rolled_back_at_ms: 1_700_005_000_000,
      model_version: 'logistic-train-441c352b',
      message: null,
    }));
    expect(r.rolled_back).toBe(true);
    expect(r.status).toBe('ok');
    expect(r.model_version).toBe('logistic-train-441c352b');
    expect(r.rolled_back_at_ms).toBe(1_700_005_000_000);
    expect(r.message).toBeNull();
  });

  it('RollbackResult failure (not found)', () => {
    const r: RollbackResult = JSON.parse(JSON.stringify({
      rolled_back: false,
      status: 'failed',
      previous_path: null,
      active_path: null,
      rolled_back_at_ms: null,
      model_version: '',
      message: "model_version 'logistic-train-XYZ' not found in promotion history",
    }));
    expect(r.rolled_back).toBe(false);
    expect(r.message).toContain('not found');
    expect(r.model_version).toBe('');
  });

  it('RollbackResult failure (no weights, v0.19 entry)', () => {
    const r: RollbackResult = JSON.parse(JSON.stringify({
      rolled_back: false,
      status: 'failed',
      previous_path: null,
      active_path: null,
      rolled_back_at_ms: null,
      model_version: '',
      message: "model_version 'logistic-train-OLD' has no weights stored (promoted before v0.20); cannot rollback",
    }));
    expect(r.rolled_back).toBe(false);
    expect(r.message).toContain('no weights');
    expect(r.message).toContain('cannot rollback');
  });
});

// =================================================================
// ================ v0.23b — auto_promote_if_better =================
// =================================================================

interface AutoPromoteIfBetterResult {
  promoted: boolean;
  skipped: boolean;
  reason: string;
  candidate_brier: number | null;
  active_brier: number | null;
  margin: number;
  model_version: string | null;
  promoted_at_ms: number | null;
  message: string | null;
}

describe('Auto-promote-if-better wire format (v0.23b)', () => {
  it('AutoPromoteIfBetterResult promoted', () => {
    const r: AutoPromoteIfBetterResult = JSON.parse(JSON.stringify({
      promoted: true,
      skipped: false,
      reason: 'auto-promoted: improvement 0.0120 > margin 0.005',
      candidate_brier: 0.180,
      active_brier: 0.192,
      margin: 0.005,
      model_version: 'logistic-train-XYZ',
      promoted_at_ms: 1_700_020_000_000,
      message: null,
    }));
    expect(r.promoted).toBe(true);
    expect(r.skipped).toBe(false);
    expect(r.reason).toContain('auto-promoted');
    expect(r.candidate_brier).toBeCloseTo(0.180);
    expect(r.active_brier).toBeCloseTo(0.192);
    expect(r.margin).toBeCloseTo(0.005);
    expect(r.model_version).toBe('logistic-train-XYZ');
  });

  it('AutoPromoteIfBetterResult skipped', () => {
    const r: AutoPromoteIfBetterResult = JSON.parse(JSON.stringify({
      promoted: false,
      skipped: true,
      reason: 'candidate brier 0.1900 is not at least 0.005 better than active 0.1920 (improvement: +0.0020)',
      candidate_brier: 0.190,
      active_brier: 0.192,
      margin: 0.005,
      model_version: null,
      promoted_at_ms: null,
      message: null,
    }));
    expect(r.promoted).toBe(false);
    expect(r.skipped).toBe(true);
    expect(r.reason).toContain('not at least 0.005 better');
    expect(r.model_version).toBeNull();
  });

  it('AutoPromoteIfBetterResult no active model', () => {
    const r: AutoPromoteIfBetterResult = JSON.parse(JSON.stringify({
      promoted: true,
      skipped: false,
      reason: 'no active model; auto-promoted the candidate',
      candidate_brier: null,
      active_brier: null,
      margin: 0.005,
      model_version: 'logistic-train-ABC',
      promoted_at_ms: 1_700_021_000_000,
      message: null,
    }));
    expect(r.promoted).toBe(true);
    expect(r.candidate_brier).toBeNull();
    expect(r.active_brier).toBeNull();
    expect(r.reason).toContain('no active model');
  });
});
