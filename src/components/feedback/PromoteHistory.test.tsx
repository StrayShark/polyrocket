/**
 * PromoteHistory component tests (v0.19c).
 *
 * v0.19c adds a read-only "Promotion history" panel
 * to the ModelLab page. It calls the `list_promote_history`
 * IPC, shows the entries in reverse-chronological order
 * (newest first), and renders a Brier badge with a
 * color hint (green/yellow/red).
 *
 * The panel has 4 render states:
 *  1. loading     → 3 skeleton rows
 *  2. error       → ErrorState
 *  3. empty       → "No promotions yet" empty state
 *  4. populated   → N rows, newest first
 *
 * Tests here mock `@/ipc` (via vi.mock) and react-query
 * (via QueryClient wrapper). The pattern matches the
 * existing AnalyzeProgress.test.tsx + TrainProgress.test.tsx.
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listPromoteHistory: vi.fn(),
  rollbackModel: vi.fn(),
}));

import { listPromoteHistory } from '@/ipc';
import { PromoteHistory } from '@/components/feedback/PromoteHistory';

// Minimal i18n shim — the real useT pulls from a Zustand
// store. The test only checks that strings render; the
// keys themselves are asserted in i18n.test.ts.
vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

describe('PromoteHistory (v0.19c)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistory).mockReset();
  });

  it('renders the empty state when history is empty', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 0,
      entries: [],
      message: 'no active model yet; train + promote to start history',
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-loading')).toBeInTheDocument();
    });
    // Wait for the empty state to replace the skeleton
    await waitFor(() => {
      expect(screen.getByText('promote.history.empty')).toBeInTheDocument();
    });
    expect(screen.getByText('promote.history.empty_desc')).toBeInTheDocument();
  });

  it('renders rows in reverse-chronological order (newest first)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        // Oldest first (Python's natural order)
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.16, best_params: null },
        { job_id: 'train-ccc', model_version: 'logistic-train-ccc', promoted_at_ms: 1_700_002_000_000, best_brier: 0.14, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const rows = screen.getAllByTestId('promote-history-row');
    expect(rows).toHaveLength(3);
    // Newest (ccc) should be FIRST, oldest (aaa) should be LAST
    expect(rows[0]).toHaveAttribute('data-job-id', 'train-ccc');
    expect(rows[1]).toHaveAttribute('data-job-id', 'train-bbb');
    expect(rows[2]).toHaveAttribute('data-job-id', 'train-aaa');
  });

  it('renders the Brier score with proper formatting', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.184, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-brier')).toBeInTheDocument();
    });
    const brier = screen.getByTestId('promote-history-brier');
    expect(brier).toHaveAttribute('data-brier', '0.184');
    expect(brier.textContent).toBe('0.184');
  });

  it('shows em-dash for null Brier (v0.18 active.json back-compat)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: null, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-brier')).toBeInTheDocument();
    });
    const brier = screen.getByTestId('promote-history-brier');
    expect(brier).toHaveAttribute('data-brier', '');
    expect(brier.textContent).toBe('—');
  });

  it('exposes the count via data-count attribute', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 5,
      entries: Array.from({ length: 5 }, (_, i) => ({
        job_id: `train-${i}`,
        model_version: `logistic-train-${i}`,
        promoted_at_ms: 1_700_000_000_000 + i * 1000,
        best_brier: 0.15,
        best_params: null,
      })),
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history')).toHaveAttribute('data-count', '5');
    expect(screen.getAllByTestId('promote-history-row')).toHaveLength(5);
  });

  it('hides the Rollback button on the active row (v0.20c)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.16, best_params: null },
      ],
      message: null,
    });
    // The active model is train-bbb (the newer one, but
    // we display newest first so it's the first row).
    render(wrap(<PromoteHistory activeModelVersion="logistic-train-bbb" />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    // The active row is marked
    const rows = screen.getAllByTestId('promote-history-row');
    expect(rows[0]).toHaveAttribute('data-active', 'true');
    expect(rows[1]).toHaveAttribute('data-active', 'false');
    // Only the non-active row has a Rollback button
    const rollbackButtons = screen.getAllByTestId('promote-history-rollback');
    expect(rollbackButtons).toHaveLength(1);
    // The active badge is on the active row
    expect(screen.getByTestId('promote-history-active-badge')).toBeInTheDocument();
  });

  it('shows Rollback button on all rows when no active version is set', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.16, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory activeModelVersion={null} />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const rollbackButtons = screen.getAllByTestId('promote-history-rollback');
    expect(rollbackButtons).toHaveLength(2);
    // No active badge
    expect(screen.queryByTestId('promote-history-active-badge')).toBeNull();
  });

  it('opens confirmation modal when Rollback is clicked (v0.20c)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-rollback')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('promote-history-rollback'));
    // Confirmation modal opens
    await waitFor(() => {
      expect(screen.getByTestId('rollback-confirm-btn')).toBeInTheDocument();
    });
    expect(screen.getByText('rollback.confirm.title')).toBeInTheDocument();
  });

  it('shows "best trial" badge when trial_index is null (v0.24a)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null, trial_index: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const badge = screen.getByTestId('promote-history-trial-badge');
    expect(badge).toHaveAttribute('data-trial-index', 'best');
    expect(badge.textContent).toContain('promote.history.trial_best');
  });

  it('shows "best trial" badge when trial_index is missing (v0.18 back-compat, v0.24a)', async () => {
    // Old history entries from before v0.21a don't have
    // trial_index at all. The L1 type marks it as optional,
    // so undefined should be treated like null (best).
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-old', model_version: 'logistic-train-old', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const badge = screen.getByTestId('promote-history-trial-badge');
    expect(badge).toHaveAttribute('data-trial-index', 'best');
  });

  it('shows "trial #N" badge for bulk-promoted trials (v0.24a)', async () => {
    // trial_index=2 → 0-indexed 2, displayed as "trial #3"
    // (1-indexed for human display)
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb-t2', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null, trial_index: 2 },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const badge = screen.getByTestId('promote-history-trial-badge');
    expect(badge).toHaveAttribute('data-trial-index', '2');
    expect(badge.textContent).toContain('promote.history.trial_n');
  });
});
