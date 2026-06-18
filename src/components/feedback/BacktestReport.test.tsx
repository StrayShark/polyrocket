/**
 * BacktestReport component tests (v0.43d).
 *
 * Tests the modal that shows the result of a
 * `backtestModel` call:
 *   - loading state (target not yet resolved)
 *   - target header
 *   - samples textarea
 *   - run button
 *   - result rendering (Brier, calibration,
 *     top winners/losers)
 *   - app-level error (sidecar ok=false)
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  backtestModel: vi.fn(),
  listPromoteHistory: vi.fn().mockResolvedValue({
    ok: true,
    count: 1,
    entries: [
      {
        job_id: 'train-target',
        model_version: 'logistic-train-target',
        promoted_at_ms: 1_700_000_000_000,
        best_brier: 0.18,
        best_params: { lr: 0.01, reg: 0.001 },
        trial_index: null,
      },
    ],
  }),
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string, p?: Record<string, unknown>) => {
    if (!p) return k;
    return k + ':' + JSON.stringify(p);
  }, locale: 'en' as const }),
}));

import { BacktestReport } from './BacktestReport';
import { backtestModel } from '@/ipc';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

const MOCK_RESULT = {
  ok: true,
  model_version: 'logistic-train-target',
  sample_count: 3,
  brier_mean: 0.18,
  brier_breakdown: [0.25, 0.05, 0.25],
  calibration: [
    { bucket: '[0.0, 0.2)', predicted_avg: 0.1, actual_rate: 0.0, count: 1 },
    { bucket: '[0.4, 0.6)', predicted_avg: 0.5, actual_rate: 0.5, count: 1 },
    { bucket: '[0.8, 1.0)', predicted_avg: 0.9, actual_rate: 1.0, count: 1 },
  ],
  top_winners: [
    { label: 'good', brier: 0.05, predicted: 0.95, outcome: 1.0 },
  ],
  top_losers: [
    { label: 'bad', brier: 0.5, predicted: 0.0, outcome: 1.0 },
  ],
  message: null,
};

describe('BacktestReport (v0.43d)', () => {
  beforeEach(() => {
    vi.mocked(backtestModel).mockReset();
  });

  it('renders nothing visible when open=false', () => {
    render(wrap(<BacktestReport open={false} onClose={() => {}} targetJobId={null} />));
    expect(screen.queryByTestId('backtest-report')).not.toBeInTheDocument();
  });

  it('renders the target header after history loads', async () => {
    render(wrap(<BacktestReport open onClose={() => {}} targetJobId="train-target" />));
    await waitFor(() => {
      expect(screen.getByTestId('backtest-target')).toBeInTheDocument();
    });
    const t = screen.getByTestId('backtest-target');
    expect(t).toHaveAttribute('data-job-id', 'train-target');
    expect(t.textContent).toContain('logistic-train-target');
  });

  it('renders the samples textarea with a default', async () => {
    render(wrap(<BacktestReport open onClose={() => {}} targetJobId="train-target" />));
    const ta = await screen.findByTestId('backtest-samples-input');
    expect(ta).toBeInTheDocument();
    // Default samples should mention "Will X happen?" etc.
    expect((ta as HTMLTextAreaElement).value).toContain('Will X happen?');
  });

  it('clicking Run with valid JSON calls backtestModel and renders result', async () => {
    vi.mocked(backtestModel).mockResolvedValue(MOCK_RESULT);
    render(wrap(<BacktestReport open onClose={() => {}} targetJobId="train-target" />));
    await waitFor(() => {
      expect(screen.getByTestId('backtest-target')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('backtest-run-btn'));
    await waitFor(() => {
      expect(screen.getByTestId('backtest-result')).toBeInTheDocument();
    });
    // Headline Brier
    const brier = screen.getByTestId('backtest-brier-mean');
    expect(brier.textContent).toContain('0.1800');
    // Calibration buckets
    const buckets = screen.getAllByTestId('backtest-cal-bucket');
    expect(buckets).toHaveLength(3);
    // Winners / losers
    expect(screen.getByTestId('backtest-top-winners')).toBeInTheDocument();
    expect(screen.getByTestId('backtest-top-losers')).toBeInTheDocument();
  });

  it('shows app-level error from sidecar (ok=false)', async () => {
    vi.mocked(backtestModel).mockResolvedValue({
      ...MOCK_RESULT,
      ok: false,
      sample_count: 0,
      brier_mean: null,
      brier_breakdown: [],
      calibration: [],
      top_winners: [],
      top_losers: [],
      message: "model 'logistic-missing' not found in archive or active",
    });
    render(wrap(<BacktestReport open onClose={() => {}} targetJobId="train-target" />));
    await waitFor(() => {
      expect(screen.getByTestId('backtest-target')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('backtest-run-btn'));
    await waitFor(() => {
      expect(screen.getByTestId('backtest-app-error')).toBeInTheDocument();
    });
    expect(screen.getByTestId('backtest-app-error').textContent).toContain('not found');
  });

  it('shows parse error when samples JSON is invalid', async () => {
    render(wrap(<BacktestReport open onClose={() => {}} targetJobId="train-target" />));
    await waitFor(() => {
      expect(screen.getByTestId('backtest-target')).toBeInTheDocument();
    });
    fireEvent.change(screen.getByTestId('backtest-samples-input'), {
      target: { value: '{not valid json' },
    });
    fireEvent.click(screen.getByTestId('backtest-run-btn'));
    await waitFor(() => {
      expect(screen.getByTestId('backtest-parse-error')).toBeInTheDocument();
    });
    // backtestModel should NOT have been called
    expect(vi.mocked(backtestModel)).not.toHaveBeenCalled();
  });
});
