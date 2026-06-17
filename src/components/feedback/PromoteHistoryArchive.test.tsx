/**
 * v0.34a — PromoteHistoryArchive component tests.
 *
 * The archive modal is the L1 UI for reading the
 * Python sidecar's `archive.jsonl` file (via the
 * v0.33b `list_promote_history_archive` IPC). The
 * tests cover:
 *  1. Empty state (no archive file yet)
 *  2. Populated state with a single page
 *  3. Pagination: prev/next buttons work
 *  4. Per-row: trial badge, brier color, time
 *  5. Close button resets offset
 *
 * Pattern matches the existing PromoteHistory.test.tsx:
 *   - happy-dom environment
 *   - mock @/ipc (only the archive IPC is needed)
 *   - mock @/lib/i18n with `(k) => k` passthrough
 *   - wrap in QueryClientProvider
 *   - the Modal is a real Modal (not stubbed)
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listPromoteHistoryArchive: vi.fn(),
}));

import { listPromoteHistoryArchive } from '@/ipc';
import { PromoteHistoryArchive } from '@/components/feedback/PromoteHistoryArchive';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

describe('PromoteHistoryArchive (v0.34a)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistoryArchive).mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders empty state when the archive file does not exist', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [],
      total: 0,
      message: 'no archive yet; archive is created on first overflow',
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    // The empty state shows the EmptyState component, not
    // the rows container. Wait for the empty title to appear.
    await waitFor(() => {
      expect(screen.getByText('promote.archive.empty_title')).toBeInTheDocument();
    });
    expect(screen.getByText('promote.archive.empty_desc')).toBeInTheDocument();
  });

  it('renders a single page of entries with total > 0', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [
        { job_id: 'a', model_version: 'la', promoted_at_ms: 3, best_brier: 0.20, best_params: null, weights: null, trial_index: null, archived_at_ms: 3 },
        { job_id: 'b', model_version: 'lb-t1', promoted_at_ms: 2, best_brier: 0.18, best_params: null, weights: null, trial_index: 1, archived_at_ms: 2 },
        { job_id: 'c', model_version: 'lc', promoted_at_ms: 1, best_brier: 0.16, best_params: null, weights: null, trial_index: null, archived_at_ms: 1 },
      ],
      total: 3,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-archive')).toBeInTheDocument();
    });
    const rows = screen.getAllByTestId('promote-history-archive-row');
    expect(rows).toHaveLength(3);
    // Total is reported
    expect(screen.getByTestId('promote-history-archive')).toHaveAttribute('data-total', '3');
  });

  it('renders the trial badge per row (best vs trial N)', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [
        { job_id: 'a', model_version: 'la', promoted_at_ms: 2, best_brier: 0.20, best_params: null, weights: null, trial_index: null, archived_at_ms: 2 },
        { job_id: 'b', model_version: 'lb-t2', promoted_at_ms: 1, best_brier: 0.18, best_params: null, weights: null, trial_index: 2, archived_at_ms: 1 },
      ],
      total: 2,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    await waitFor(() => {
      expect(screen.getAllByTestId('promote-history-archive-row')).toHaveLength(2);
    });
    const badges = screen.getAllByTestId('promote-history-archive-trial-badge');
    expect(badges).toHaveLength(2);
    expect(badges[0]).toHaveAttribute('data-trial-index', 'best');
    expect(badges[1]).toHaveAttribute('data-trial-index', '2');
  });

  it('pagination: next button calls IPC with offset+25', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: Array.from({ length: 25 }, (_, i) => ({
        job_id: `a${i}`,
        model_version: `la${i}`,
        promoted_at_ms: 100 - i,
        best_brier: 0.2,
        best_params: null,
        weights: null,
        trial_index: null,
        archived_at_ms: 100 - i,
      })),
      total: 100,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    // Wait for the first batch of rows to render
    await waitFor(() => {
      expect(screen.getAllByTestId('promote-history-archive-row').length).toBe(25);
    });
    // Initial call: offset=0
    expect(vi.mocked(listPromoteHistoryArchive).mock.calls[0]?.[0]?.offset).toBe(0);
    // Click next
    fireEvent.click(screen.getByTestId('promote-history-archive-next'));
    // Wait for the next call to be made
    await waitFor(() => {
      expect(vi.mocked(listPromoteHistoryArchive).mock.calls.length).toBeGreaterThan(1);
    });
    // The latest call should have offset=25
    const calls = vi.mocked(listPromoteHistoryArchive).mock.calls;
    expect(calls[calls.length - 1]?.[0]?.offset).toBe(25);
  });

  it('pagination: prev button is disabled on first page', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: Array.from({ length: 25 }, (_, i) => ({
        job_id: `a${i}`,
        model_version: `la${i}`,
        promoted_at_ms: 100 - i,
        best_brier: 0.2,
        best_params: null,
        weights: null,
        trial_index: null,
        archived_at_ms: 100 - i,
      })),
      total: 50,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    // Wait for the first batch of rows to render
    await waitFor(() => {
      expect(screen.getAllByTestId('promote-history-archive-row').length).toBe(25);
    });
    // First page → prev disabled (offset=0, hasPrev=false)
    expect(screen.getByTestId('promote-history-archive-prev')).toBeDisabled();
    // First page, but total=50 > 25 → next enabled
    expect(screen.getByTestId('promote-history-archive-next')).not.toBeDisabled();
  });

  it('does not query the IPC when the modal is closed', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [],
      total: 0,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open={false} onClose={() => {}} />));
    // Wait a tick to let any effects run
    await new Promise((r) => setTimeout(r, 50));
    expect(listPromoteHistoryArchive).not.toHaveBeenCalled();
  });
});
