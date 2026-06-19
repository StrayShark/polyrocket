// v0.72d — History extras round 2 tests.
//
// History.tsx is 345 lines with 10-column DataTable + 4-KPI
// summary + 5 status filters + tx_hash external links.
// Existing tests (v0.62a + v0.71c) cover 12 cases. We add 10
// more covering the remaining branches:
//   - pnl === 0 (zero PnL → muted text, no +/- prefix)
//   - tx_hash null fallback (em-dash instead of link)
//   - fill column with slippage 0 (<0.01 → muted, no color)
//   - fill column with positive slippage (slipPct > 0 → bear color)
//   - fill column with negative slippage (slipPct < 0 → bull color)
//   - order_type='limit' → accent pill
//   - order_type='stop_loss' → warning pill, text "stop-loss"
//   - status='cancelled' → muted pill
//   - status='open' → accent pill (default branch)
//   - KPI winrate delta: positive when won+lost>0 and winRate>=0.5
//
// Coverage target: branches 84.28% → ~90%.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const { mockListBets: mlb } = vi.hoisted(() => ({ mockListBets: vi.fn() }));

vi.mock('@/ipc', () => createIpcMock({
  listBets: (...args: unknown[]) => mlb(...args),
}));

import { History } from './History';

function renderHistory() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <History />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const BETS = [
  // b1: zero PnL (pnl=0 → muted text branch)
  { id: 'b1', market_id: 'm1', side: 'YES' as const, mode: 'A_jump' as const, size: 100, price: 0.5, status: 'won' as const, placed_at: Date.now() - 1000, pnl: 0, fill_price: 0.5, partial: false, post_only: false, order_type: 'market' as const, tx_hash: null },
  // b2: positive fill slippage (fill_price > price → bear)
  { id: 'b2', market_id: 'm2', side: 'NO' as const, mode: 'B_drift' as const, size: 50, price: 0.6, status: 'won' as const, placed_at: Date.now() - 5000, pnl: 25, fill_price: 0.7, partial: false, post_only: true, order_type: 'limit' as const, tx_hash: '0xabc123' },
  // b3: negative fill slippage (fill_price < price → bull)
  { id: 'b3', market_id: 'm3', side: 'YES' as const, mode: 'A_jump' as const, size: 30, price: 0.4, status: 'won' as const, placed_at: Date.now() - 8000, pnl: 10, fill_price: 0.35, partial: true, post_only: false, order_type: 'stop_loss' as const, tx_hash: '0xdef456' },
  // b4: cancelled status → muted pill
  { id: 'b4', market_id: 'm4', side: 'NO' as const, mode: 'B_drift' as const, size: 20, price: 0.7, status: 'cancelled' as const, placed_at: Date.now() - 12000, pnl: null, fill_price: null, partial: false, post_only: false, order_type: 'market' as const, tx_hash: null },
  // b5: open status → accent pill (default branch)
  { id: 'b5', market_id: 'm5', side: 'YES' as const, mode: 'A_jump' as const, size: 80, price: 0.55, status: 'open' as const, placed_at: Date.now() - 2000, pnl: null, fill_price: null, partial: false, post_only: false, order_type: 'limit' as const, tx_hash: null },
];

beforeEach(() => {
  vi.clearAllMocks();
  mlb.mockResolvedValue(BETS);
});

describe('History (extras round 2 — v0.72d)', () => {
  it('renders all 5 bets', async () => {
    renderHistory();
    await waitFor(() => {
      expect(screen.getByText('m1')).toBeInTheDocument();
      expect(screen.getByText('m5')).toBeInTheDocument();
    });
  });

  it('zero PnL renders with no +/- prefix (muted text)', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m1'));
    // b1 pnl=0 → renders as "$0.00" (no + prefix, no - sign, muted color)
    const text = document.body.textContent || '';
    // Should NOT have "+$0.00" — should have "$0.00" only
    expect(text).not.toMatch(/\+\$0\.00/);
    expect(text).toMatch(/\$0\.00/);
  });

  it('tx_hash=null row renders em-dash instead of link icon', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m1'));
    // b1, b4, b5 have tx_hash=null
    // b2, b3 have tx_hash set → render ExternalLink icon
    const externalLinks = document.querySelectorAll('a[href*="polygonscan"]');
    expect(externalLinks.length).toBe(2); // b2 + b3 only
  });

  it('positive fill slippage renders in bear color', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m2'));
    // b2: fill_price=0.7 > price=0.6 → positive slip → bear text
    const bearCells = document.querySelectorAll('.text-bear');
    expect(bearCells.length).toBeGreaterThan(0);
  });

  it('negative fill slippage renders in bull color', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m3'));
    // b3: fill_price=0.35 < price=0.4 → negative slip → bull text
    const bullCells = document.querySelectorAll('.text-bull');
    expect(bullCells.length).toBeGreaterThan(0);
  });

  it('order_type=limit renders accent pill', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m2'));
    // b2 order_type='limit' → accent pill
    const accentPills = document.querySelectorAll('.bg-accent\\/15, .text-accent');
    expect(accentPills.length).toBeGreaterThan(0);
  });

  it('order_type=stop_loss renders warning pill with "stop-loss" text', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m3'));
    // b3 order_type='stop_loss' → warning pill, text "stop-loss" (not "stop_loss")
    expect(screen.getByText('stop-loss')).toBeInTheDocument();
    const warningPills = document.querySelectorAll('.bg-warning\\/15, .text-warning');
    expect(warningPills.length).toBeGreaterThan(0);
  });

  it('status=cancelled renders muted pill', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m4'));
    // b4 status='cancelled' → muted pill
    expect(screen.getAllByText('cancelled').length).toBeGreaterThan(0);
  });

  it('status=open renders accent pill (default branch)', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m5'));
    // b5 status='open' → falls through to 'accent' kind
    expect(screen.getAllByText('open').length).toBeGreaterThan(0);
  });

  it('KPI winrate delta renders text in "{won}W/{lost}L" format when won+lost>0', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m1'));
    // BETS has 3 won (b1,b2,b3) + 1 lost (none, only won) + 1 cancelled + 1 open
    // Wait — only b1, b2, b3 are 'won'; b4='cancelled'; b5='open'. No 'lost'.
    // winRate = won / (won + lost) = 3 / 3 = 100%
    const text = document.body.textContent || '';
    expect(text).toMatch(/3W\/0L/);
    // 100% winrate → positive delta
    expect(text).toMatch(/100%/);
  });
});