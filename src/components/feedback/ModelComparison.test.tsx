/**
 * v0.40b — ModelComparison component tests.
 *
 * The multi-model comparison modal shows 2-3 model
 * versions side-by-side. The "lowest Brier" entry
 * is highlighted as the winner.
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import { ModelComparison } from '@/components/feedback/ModelComparison';
import type { PromoteHistoryEntry } from '@/ipc';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

const E1: PromoteHistoryEntry = {
  job_id: 'a',
  model_version: 'logistic-a',
  promoted_at_ms: 1_700_000_000_000,
  best_brier: 0.20,
  best_params: { lr: 0.01, reg: 0.001 },
  trial_index: null,
};
const E2: PromoteHistoryEntry = {
  job_id: 'b',
  model_version: 'logistic-b-t0',
  promoted_at_ms: 1_700_001_000_000,
  best_brier: 0.18,
  best_params: { lr: 0.01, reg: 0.001 },
  trial_index: 0,
};
const E3: PromoteHistoryEntry = {
  job_id: 'c',
  model_version: 'logistic-c-t2',
  promoted_at_ms: 1_700_002_000_000,
  best_brier: 0.16,
  best_params: { lr: 0.005, reg: 0.0001 },
  trial_index: 2,
};

describe('ModelComparison (v0.40b)', () => {
  beforeEach(() => {
    // No-op; just for symmetry with other tests
  });
  afterEach(() => {
    cleanup();
  });

  it('renders 2 columns when given 2 entries', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2]} />);
    const cols = screen.getAllByTestId('model-comparison-col');
    expect(cols).toHaveLength(2);
    expect(screen.getByTestId('model-comparison')).toHaveAttribute('data-count', '2');
  });

  it('renders 3 columns when given 3 entries', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2, E3]} />);
    const cols = screen.getAllByTestId('model-comparison-col');
    expect(cols).toHaveLength(3);
    expect(screen.getByTestId('model-comparison')).toHaveAttribute('data-count', '3');
  });

  it('marks the entry with the lowest Brier as the best', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2, E3]} />);
    // The "★ best" badge is shown on the best col
    // (E3 has Brier 0.16, the lowest)
    const best = screen.getByTestId('model-comparison-best');
    expect(best).toBeInTheDocument();
    // The best col is E3
    const cols = screen.getAllByTestId('model-comparison-col');
    const bestCol = cols.find((c) => c.getAttribute('data-best') === 'true');
    expect(bestCol).not.toBeNull();
    expect(bestCol).toHaveAttribute('data-job-id', 'c');
  });

  it('shows the lowest Brier summary at the bottom', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2]} />);
    const summary = screen.getByTestId('model-comparison-summary');
    expect(summary).toBeInTheDocument();
    // The summary contains the best model_version
    expect(summary.textContent).toContain('logistic-b-t0');
    expect(summary.textContent).toContain('0.1800');
  });

  it('handles entries with null brier (pre-v0.18)', () => {
    const E_NO_BRIER: PromoteHistoryEntry = {
      ...E1,
      job_id: 'no-brier',
      model_version: 'logistic-no-brier',
      best_brier: null,
    };
    render(<ModelComparison open onClose={() => {}} entries={[E_NO_BRIER, E2]} />);
    // E2 (0.18) is the best
    const cols = screen.getAllByTestId('model-comparison-col');
    expect(cols[0]).toHaveAttribute('data-best', 'false'); // null brier
    expect(cols[1]).toHaveAttribute('data-best', 'true'); // 0.18
  });
});
