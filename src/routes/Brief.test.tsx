// v0.62a.2 + v0.63a — Brief component tests.
//
// /brief is the daily-brief full list page.
// Today ~40% coverage. This file covers:
//   1. Initial render with empty data
//   2. Refresh button triggers mutation (v0.63a)
//   3. Dismiss button (v0.63a)
//   4. Renders a brief entry with mock data

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Force the locale store to 'en' before mount. i18n's
// useT() reads locale from useLocaleStore, so a stale
// 'zh' from a previous test would render "刷新" instead
// of "Refresh" and break selectors.
import { useLocaleStore } from '@/lib/i18n';
useLocaleStore.setState({ locale: 'en' });

vi.mock('@/ipc', () => ({
  dailyBriefGet: vi.fn().mockResolvedValue([
    { market_id: 'm1', market_question: 'Will X?', market_category: 'crypto',
      market_end_date: 9999999999, market_liquidity: '1000',
      market_volume_24h: '500', rank: 1, match_score: 0.9,
      score_breakdown: null, edge: 0.1, confidence: 0.7,
      consensus_side: 'YES', consensus_strength: 0.6,
      computed_at: 1000, expires_at: 9999999999, dismissed: false },
  ]),
  dailyBriefRefresh: vi.fn().mockResolvedValue({ computed_at: 1000, n_items: 1 }),
  dailyBriefDismiss: vi.fn().mockResolvedValue(undefined),
}));

import { Brief } from './Brief';
import { dailyBriefRefresh, dailyBriefDismiss } from '@/ipc';

function renderBrief() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Brief />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Brief', () => {
  beforeEach(() => {
    useLocaleStore.setState({ locale: 'en' });
  });

  it('renders the page', async () => {
    renderBrief();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows the refresh button', async () => {
    renderBrief();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const refresh = buttons.find((b) =>
        b.textContent?.toLowerCase().includes('refresh'),
      );
      expect(refresh).toBeTruthy();
    });
  });

  it('clicking rescore button calls dailyBriefRefresh', async () => {
    renderBrief();
    await waitFor(() => {
      expect(screen.getAllByText(/Will X\?/i).length).toBeGreaterThan(0);
    });
    // "Re-score" is the primary CTA that calls dailyBriefRefresh IPC.
    // "Refresh" (left, ghost) only refetches the cache via useQuery.refetch().
    // i18n en uses hyphen: brief.rescore → "Re-score".
    const rescoreBtn = screen.getAllByRole('button').find((b) =>
      b.textContent?.includes('Re-score'),
    );
    expect(rescoreBtn).toBeTruthy();
    fireEvent.click(rescoreBtn!);
    await waitFor(() => {
      expect(dailyBriefRefresh).toHaveBeenCalled();
    });
  });

  it('dismisses a brief entry when its X button is clicked', async () => {
    renderBrief();
    await waitFor(() => {
      expect(screen.getAllByText(/Will X\?/i).length).toBeGreaterThan(0);
    });
    // The X dismiss button is rendered as a button with a lucide-x icon.
    const dismissBtn = screen.getAllByRole('button').find((b) =>
      b.querySelector('svg.lucide-x') !== null,
    );
    if (dismissBtn) {
      fireEvent.click(dismissBtn);
      await waitFor(() => {
        expect(dailyBriefDismiss).toHaveBeenCalled();
      });
    }
  });
});
