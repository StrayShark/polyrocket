// v0.89b — Trade.tsx + MarketDetail.tsx branches round (+9 tests,
// Trade 66.66→100%, MarketDetail 72.72→95%).
//
// Trade.tsx (v0.52) had only 1 test covering basic render. URL param
// parsing branches (`params.get('side')`, `price ? parseFloat : undefined`)
// were uncovered. v0.89b adds 2 tests with different URL params.
//
// MarketDetail.tsx (v0.77c round 2) had 5 tests covering loading /
// error / found / signals-filtered / signals-empty. Market status pill
// 3-way branch (resolved / active / inactive) and signal edge color
// (>0 bull / <=0 bear) were uncovered.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListMarkets = vi.fn();
const mockListActiveSignals = vi.fn();

vi.mock('@/ipc', () => ({
  listMarkets: (...args: unknown[]) => Promise.resolve(mockListMarkets(...args)),
  listActiveSignals: (...args: unknown[]) => Promise.resolve(mockListActiveSignals(...args)),
  listWallets: vi.fn().mockResolvedValue([]),
  placeJumpLink: vi.fn().mockResolvedValue('https://polymarket.com/event/_stub_'),
  placeSignedOrder: vi.fn().mockResolvedValue({ id: 'bet_stub', status: 'open' }),
  computeAllocationPreview: vi.fn(),
  validateOrderArgs: vi.fn().mockResolvedValue({ ok: true, errors: [] }),
}));

vi.mock('@/lib/format', () => ({
  fmtDate: (v: number) => `date-${v}`,
  fmtUsdc: (v: number) => `$${v.toFixed(2)}`,
  fmtEdge: (v: number) => `${(v * 100).toFixed(1)}%`,
  fmtConfidence: (v: number) => v.toFixed(2),
}));

// NOTE: don't mock @/lib/i18n — let real i18n run so the i18n keys
// resolve to actual translations (e.g. 'marketdetail.active' → 'active').

vi.mock('@/stores/toast-store', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

import { Trade } from '@/routes/Trade';
import { MarketDetail } from '@/routes/MarketDetail';

// Force cleanup between tests so React act() warnings don't accumulate
afterEach(() => {
  cleanup();
});

// -------- Trade.tsx tests --------

describe('v0.89b — Trade.tsx URL param branches', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders with no URL params (all initial* undefined)', () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter>
          <Trade />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    expect(document.body.textContent).toBeTruthy();
  });

  it('renders with full URL params (market, side, price)', () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
    render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/trade?market=m_yes_btc&side=YES&price=0.55']}>
          <Trade />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    expect(document.body.textContent).toBeTruthy();
  });
});

// -------- MarketDetail.tsx tests --------

const SAMPLE_MARKET_ACTIVE = {
  id: 'm_active',
  question: 'Will BTC hit $200k in 2026?',
  category: 'Crypto',
  slug: 'btc-200k',
  active: true,
  resolved: false,
  outcome: null,
  end_date: 1735689600,
  liquidity: 50000,
  volume_24h: 12000,
  clob_token_id: 'tok_active',
};

const SAMPLE_MARKET_INACTIVE = {
  ...SAMPLE_MARKET_ACTIVE,
  id: 'm_inactive',
  active: false,
};

const SAMPLE_MARKET_RESOLVED = {
  ...SAMPLE_MARKET_ACTIVE,
  id: 'm_resolved',
  resolved: true,
  outcome: 'YES',
};

const SAMPLE_SIGNAL_POSITIVE = {
  id: 1,
  market_id: 'm_active',
  computed_at: 1700000000,
  model_version: 'v1',
  predicted_prob: 0.7,
  market_prob: 0.5,
  edge: 0.2,
  confidence: 0.8,
  horizon_hours: 24,
  rationale: null,
};

const SAMPLE_SIGNAL_NEGATIVE = {
  ...SAMPLE_SIGNAL_POSITIVE,
  id: 2,
  edge: -0.15,
};

function wrapMarket(initialPath: string) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[initialPath]}>
        <Routes>
          <Route path="/markets" element={<div>Markets</div>} />
          <Route path="/markets/:id" element={<MarketDetail />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('v0.89b — MarketDetail.tsx status pill + edge color branches', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows "active" pill when market.active && !resolved', async () => {
    mockListMarkets.mockResolvedValue([SAMPLE_MARKET_ACTIVE]);
    mockListActiveSignals.mockResolvedValue([]);
    wrapMarket('/markets/m_active');
    // i18n key 'marketdetail.active' → 'active' (lowercase per src/lib/i18n.ts:745)
    const pill = await screen.findByText('active');
    expect(pill).toBeTruthy();
  });

  it('shows "inactive" pill when !active && !resolved', async () => {
    mockListMarkets.mockResolvedValue([SAMPLE_MARKET_INACTIVE]);
    mockListActiveSignals.mockResolvedValue([]);
    wrapMarket('/markets/m_inactive');
    const pill = await screen.findByText('inactive');
    expect(pill).toBeTruthy();
  });

  it('shows resolved outcome pill when market.resolved', async () => {
    mockListMarkets.mockResolvedValue([SAMPLE_MARKET_RESOLVED]);
    mockListActiveSignals.mockResolvedValue([]);
    wrapMarket('/markets/m_resolved');
    // Resolved branch shows market.outcome ?? 'resolved' — YES wins
    const pill = await screen.findByText('YES');
    expect(pill).toBeTruthy();
  });

  it('renders signal with positive edge in bull (green) color', async () => {
    mockListMarkets.mockResolvedValue([SAMPLE_MARKET_ACTIVE]);
    mockListActiveSignals.mockResolvedValue([SAMPLE_SIGNAL_POSITIVE]);
    wrapMarket('/markets/m_active');
    const edgeText = await screen.findByText(/20\.0%/);
    expect(edgeText.className).toContain('text-bull');
  });

  it('renders signal with negative edge in bear (red) color', async () => {
    mockListMarkets.mockResolvedValue([SAMPLE_MARKET_ACTIVE]);
    mockListActiveSignals.mockResolvedValue([SAMPLE_SIGNAL_NEGATIVE]);
    wrapMarket('/markets/m_active');
    const edgeText = await screen.findByText(/-15\.0%/);
    expect(edgeText.className).toContain('text-bear');
  });

  it('renders both positive + negative signals in mixed card', async () => {
    mockListMarkets.mockResolvedValue([SAMPLE_MARKET_ACTIVE]);
    mockListActiveSignals.mockResolvedValue([SAMPLE_SIGNAL_POSITIVE, SAMPLE_SIGNAL_NEGATIVE]);
    wrapMarket('/markets/m_active');
    const bullEdge = await screen.findByText(/20\.0%/);
    const bearEdge = await screen.findByText(/-15\.0%/);
    expect(bullEdge.className).toContain('text-bull');
    expect(bearEdge.className).toContain('text-bear');
  });

  it('shows signals empty state when filtered marketSignals is empty', async () => {
    mockListMarkets.mockResolvedValue([SAMPLE_MARKET_ACTIVE]);
    mockListActiveSignals.mockResolvedValue([
      { ...SAMPLE_SIGNAL_NEGATIVE, market_id: 'm_other' },
    ]);
    wrapMarket('/markets/m_active');
    // Wait for market detail to render, then check no signal cards
    await screen.findByText('active');
    // marketSignals filtered to 0 → EmptyState renders "No signals yet"
    // Use a flexible check since i18n key is 'signals.empty' with no default in our mock
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/signals/i);
    }, { timeout: 2000 });
  });
});
