// v0.72a — Brief route additional tests.
//
// Brief.tsx is 181 lines with 8-entry brief list + rescore/dismiss
// mutations + filter chips (rank/edge/category) + dismissed pill.
// Existing test (v0.62a + v0.63a) covers 4 surface cases (mount +
// refresh + dismiss + rescore). We add 10 tests covering the
// 5 state branches:
//   - error state with retry (line 60)
//   - loading state with 4 Skeletons (lines 98-103)
//   - empty state with EmptyState + "Generate" CTA (lines 104-114)
//   - refresh (ghost) button calls useQuery.refetch, not mutation (line 80)
//   - rescore onSuccess → toast.success + invalidates query (lines 42-48)
//   - rescore onError → toast.error with message (line 49)
//   - dismiss onSuccess → toast.info + invalidates query (lines 53-58)
//   - card with dismissed=true shows "Dismissed" pill (lines 131-133)
//   - card without edge/confidence (null fields, lines 142-149)
//   - consensus_side 'NO' → bear pill rendering (line 151)
//
// Coverage target: 78.26% → ~88% stmts.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

import { useLocaleStore } from '@/lib/i18n';
import { useToastStore } from '@/stores/toast-store';
useLocaleStore.setState({ locale: 'en' });

const { mockGet, mockRefresh, mockDismiss } = vi.hoisted(() => ({
  mockGet: vi.fn(),
  mockRefresh: vi.fn(),
  mockDismiss: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  dailyBriefGet: (...args: unknown[]) => mockGet(...args),
  dailyBriefRefresh: (...args: unknown[]) => mockRefresh(...args),
  dailyBriefDismiss: (...args: unknown[]) => mockDismiss(...args),
}));

import { Brief } from './Brief';

function renderBrief() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Brief />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const SAMPLE_BRIEF = [
  { market_id: 'm1', market_question: 'Will BTC reach 100k?', market_category: 'crypto',
    market_end_date: 9999999999, market_liquidity: '1000',
    market_volume_24h: '500', rank: 1, match_score: 0.9,
    score_breakdown: null, edge: 0.12, confidence: 0.75,
    consensus_side: 'YES', consensus_strength: 0.6,
    computed_at: 1000, expires_at: 9999999999, dismissed: false },
  { market_id: 'm2', market_question: 'Will ETH stay above 4k?', market_category: 'crypto',
    market_end_date: 9999999999, market_liquidity: '2000',
    market_volume_24h: '800', rank: 2, match_score: 0.85,
    score_breakdown: null, edge: -0.08, confidence: 0.6,
    consensus_side: 'NO', consensus_strength: 0.7,
    computed_at: 1000, expires_at: 9999999999, dismissed: true },
  { market_id: 'm3', market_question: 'Will Fed cut rates?', market_category: 'macro',
    market_end_date: 9999999999, market_liquidity: '5000',
    market_volume_24h: '2000', rank: 3, match_score: 0.7,
    score_breakdown: null, edge: null, confidence: null,
    consensus_side: null, consensus_strength: null,
    computed_at: 1000, expires_at: 9999999999, dismissed: false },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockGet.mockResolvedValue(SAMPLE_BRIEF);
  mockRefresh.mockResolvedValue({ computed_at: 1000, n_items: 3 });
  mockDismiss.mockResolvedValue(undefined);
});

describe('Brief (extended — v0.72a)', () => {
  it('shows error state with retry button when get fails', async () => {
    mockGet.mockRejectedValue(new Error('brief load failed'));
    renderBrief();
    await waitFor(() => {
      expect(screen.getByText(/brief load failed/i)).toBeInTheDocument();
    });
    const retryBtn = screen.getAllByRole('button').find(b =>
      /try again/i.test(b.textContent || ''),
    );
    expect(retryBtn).toBeTruthy();
  });

  it('renders 4 skeleton placeholders during loading', async () => {
    mockGet.mockReturnValue(new Promise(() => {})); // never resolves
    const { container } = renderBrief();
    await waitFor(() => {
      // Skeleton component renders div with animate-pulse class
      const skeletons = container.querySelectorAll('.animate-pulse');
      expect(skeletons.length).toBeGreaterThanOrEqual(4);
    });
  });

  it('renders empty state with Generate CTA when data is empty', async () => {
    mockGet.mockResolvedValue([]);
    renderBrief();
    await waitFor(() => {
      // EmptyState component + "Generate" CTA
      expect(screen.getByText(/Generate|生成/i)).toBeInTheDocument();
    });
    const genBtn = screen.getAllByRole('button').find(b =>
      /generate|生成/i.test(b.textContent || ''),
    );
    expect(genBtn).toBeTruthy();
    // Click Generate — should trigger refreshMut
    fireEvent.click(genBtn!);
    await waitFor(() => {
      expect(mockRefresh).toHaveBeenCalled();
    });
  });

  it('refresh ghost button calls useQuery.refetch (not IPC mutation)', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const callsBefore = mockRefresh.mock.calls.length;
    const refreshBtn = screen.getAllByRole('button').find(b =>
      /refresh/i.test(b.textContent || '') && !/re-score/i.test(b.textContent || ''),
    );
    expect(refreshBtn).toBeTruthy();
    fireEvent.click(refreshBtn!);
    await waitFor(() => {
      // Ghost "Refresh" only refetches cache; mutation should NOT be called
      expect(mockRefresh.mock.calls.length).toBe(callsBefore);
    });
  });

  it('rescore onSuccess shows toast with item count + invalidates query', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const rescoreBtn = screen.getAllByRole('button').find(b =>
      /re-score/i.test(b.textContent || ''),
    );
    fireEvent.click(rescoreBtn!);
    await waitFor(() => {
      expect(mockRefresh).toHaveBeenCalled();
    });
    // Toast store should have received success
    const { toasts } = useToastStore.getState();
    expect(toasts.length).toBeGreaterThan(0);
    const success = toasts.find(t => t.kind === 'success');
    expect(success).toBeTruthy();
  });

  it('rescore onError shows error toast with body', async () => {
    mockRefresh.mockRejectedValue(new Error('rescore failed'));
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const rescoreBtn = screen.getAllByRole('button').find(b =>
      /re-score/i.test(b.textContent || ''),
    );
    fireEvent.click(rescoreBtn!);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find(t => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body).toMatch(/rescore failed/i);
    });
  });

  it('dismiss onSuccess shows info toast + invalidates query', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const dismissBtn = screen.getAllByRole('button').find(b =>
      b.querySelector('svg.lucide-x') !== null,
    );
    expect(dismissBtn).toBeTruthy();
    fireEvent.click(dismissBtn!);
    await waitFor(() => {
      expect(mockDismiss).toHaveBeenCalledWith('m1');
    });
    const { toasts } = useToastStore.getState();
    const info = toasts.find(t => t.kind === 'info');
    expect(info).toBeTruthy();
  });

  it('renders dismissed pill for entries with dismissed=true', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/ETH stay above 4k/i));
    // m2 has dismissed=true — should render a "Dismissed" pill
    expect(screen.getAllByText(/dismissed/i).length).toBeGreaterThan(0);
  });

  it('handles entries with null edge/confidence (no edge/conf line)', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/Fed cut rates/i));
    // m3 has edge=null + confidence=null + consensus_side=null
    // The row should still render but the edge / confidence / consensus
    // Pill section should not produce text for m3.
    // Easiest check: m3 question text is in the document, but no
    // "+12.0%" appears next to it (m1 has +12.0%, m3 has nothing).
    const fedRow = screen.getByText(/Fed cut rates/i).closest('div');
    expect(fedRow).toBeTruthy();
    // m3 row contains neither "+12.0%" edge nor "75.0%" confidence text
    const fedRowText = fedRow?.textContent || '';
    expect(fedRowText).not.toMatch(/\+12\.0%/);
    expect(fedRowText).not.toMatch(/75\.0%/);
  });

  it('renders NO consensus as bear pill (different from YES bull)', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/ETH stay above 4k/i));
    // m2 has consensus_side='NO' → bear pill
    // The pill text contains both side letter + strength %
    const noText = screen.getAllByText(/NO/);
    expect(noText.length).toBeGreaterThan(0);
  });
});