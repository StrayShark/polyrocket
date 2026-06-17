/**
 * PromoteHistoryChart component tests (v0.22a).
 *
 * The chart is a self-contained SVG sparkline. It reuses
 * the `useQuery(['promote-history'], listPromoteHistory)`
 * pattern from PromoteHistory, but renders differently
 * (SVG dots + connecting line, no per-row detail).
 *
 * The chart has 3 render states:
 *  1. loading     → skeleton
 *  2. error       → ErrorState
 *  3. empty       → "No brier data yet" placeholder
 *  4. populated   → SVG with dots + connecting line
 *
 * Tests here mock `@/ipc` and react-query (via QueryClient
 * wrapper). The pattern matches PromoteHistory.test.tsx.
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listPromoteHistory: vi.fn(),
  rollbackModel: vi.fn(),
}));

import { listPromoteHistory } from '@/ipc';
import { PromoteHistoryChart } from '@/components/feedback/PromoteHistoryChart';

// Minimal i18n shim — the real useT pulls from a Zustand store.
vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

describe('PromoteHistoryChart (v0.22a)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistory).mockReset();
  });

  it('renders the empty state when no entries have brier data', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 0,
      entries: [],
      message: 'no active model yet',
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-empty')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-empty').textContent).toContain(
      'promote.chart.empty'
    );
  });

  it('skips entries with null brier (v0.18 back-compat)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        // First entry: null brier (should be skipped)
        { job_id: 'train-old', model_version: 'logistic-train-old', promoted_at_ms: 1_700_000_000_000, best_brier: null, best_params: null },
        // Second entry: real brier
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    // Only 1 dot (the null-brier entry was filtered)
    const dots = screen.getAllByTestId('promote-history-chart-dot');
    expect(dots).toHaveLength(1);
    expect(dots[0]).toHaveAttribute('data-job-id', 'train-aaa');
  });

  it('renders one dot per entry with brier data', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.20, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-ccc', model_version: 'logistic-train-ccc', promoted_at_ms: 1_700_002_000_000, best_brier: 0.14, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    const dots = screen.getAllByTestId('promote-history-chart-dot');
    expect(dots).toHaveLength(3);
    expect(dots[0]).toHaveAttribute('data-brier', '0.2');
    expect(dots[1]).toHaveAttribute('data-brier', '0.18');
    expect(dots[2]).toHaveAttribute('data-brier', '0.14');
  });

  it('shows "improving" trend when latest brier is lower than first', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1_700_000_000_000, best_brier: 0.20, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'c', model_version: 'c', promoted_at_ms: 1_700_002_000_000, best_brier: 0.14, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-trend')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-trend')).toHaveAttribute('data-trend', 'up');
  });

  it('shows "worsening" trend when latest brier is higher than first', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1_700_000_000_000, best_brier: 0.14, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'c', model_version: 'c', promoted_at_ms: 1_700_002_000_000, best_brier: 0.22, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-trend')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-trend')).toHaveAttribute('data-trend', 'down');
  });

  it('shows "flat" trend when all briers are equal', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'c', model_version: 'c', promoted_at_ms: 1_700_002_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-trend')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-trend')).toHaveAttribute('data-trend', 'flat');
  });

  it('exposes the count via data-points attribute', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 5,
      entries: Array.from({ length: 5 }, (_, i) => ({
        job_id: `train-${i}`,
        model_version: `logistic-train-${i}`,
        promoted_at_ms: 1_700_000_000_000 + i * 1000,
        best_brier: 0.15 + i * 0.01,
        best_params: null,
      })),
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart')).toHaveAttribute('data-points', '5');
  });
});
