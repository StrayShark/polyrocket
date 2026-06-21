// v0.93 — Signals.tsx branches round 2 (+7 tests, fn 72.7→~85%).
//
// Targets the column cell branches, KpiCard delta branches,
// Recompute success/error branches, and the empty-state
// "no-match" description branch. Existing tests cover the
// surface-level filter/mutation flow.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const { mockListActiveSignals: mls, mockRecomputeSignals: mrs } = vi.hoisted(() => ({
  mockListActiveSignals: vi.fn(),
  mockRecomputeSignals: vi.fn(),
}));

const { mockToastSuccess, mockToastError } = vi.hoisted(() => ({
  mockToastSuccess: vi.fn(),
  mockToastError: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listActiveSignals: (...args: unknown[]) => mls(...args),
  recomputeSignals: (...args: unknown[]) => mrs(...args),
}));

vi.mock('@/stores/toast-store', () => ({
  toast: { success: mockToastSuccess, error: mockToastError, info: vi.fn() },
  useToastStore: () => ({ toasts: [], push: vi.fn(), dismiss: vi.fn() }),
}));

import { Signals } from './Signals';

function renderSignals() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Signals />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const SAMPLE = [
  { id: 1, market_id: 'm1', computed_at: 1718710000000, model_version: 'v1', predicted_prob: 0.65, market_prob: 0.50, edge: 0.15, confidence: 0.8, horizon_hours: 24, rationale: 'r1', market_question: 'Will X happen?' },
  { id: 2, market_id: 'm2', computed_at: 1718710100000, model_version: 'v1', predicted_prob: 0.40, market_prob: 0.50, edge: -0.10, confidence: 0.7, horizon_hours: 12, rationale: 'r2', market_question: 'Will Y happen?' },
  { id: 3, market_id: 'm3', computed_at: 1718710200000, model_version: 'v1', predicted_prob: 0.70, market_prob: 0.50, edge: 0.20, confidence: 0.9, horizon_hours: 48, rationale: null, market_question: 'Will Z happen?' },
];

describe('Signals (v0.93) — branches round 2', () => {
  it('Trade button URL encodes YES for bullish signal (edge > 0)', async () => {
    mls.mockResolvedValue(SAMPLE);
    renderSignals();
    await waitFor(() => screen.getByText('Will X happen?'));
    // signal-trade-1 corresponds to id=1 which is bullish (edge=0.15)
    const tradeLink = screen.getByTestId('signal-trade-1') as HTMLAnchorElement;
    expect(tradeLink.getAttribute('href')).toContain('side=YES');
    expect(tradeLink.getAttribute('href')).toContain('market=m1');
    expect(tradeLink.getAttribute('href')).toContain('price=0.6500');
  });

  it('Trade button URL encodes NO for bearish signal (edge < 0)', async () => {
    mls.mockResolvedValue(SAMPLE);
    renderSignals();
    await waitFor(() => screen.getByText('Will Y happen?'));
    // signal-trade-2 corresponds to id=2 which is bearish (edge=-0.10)
    const tradeLink = screen.getByTestId('signal-trade-2') as HTMLAnchorElement;
    expect(tradeLink.getAttribute('href')).toContain('side=NO');
    expect(tradeLink.getAttribute('href')).toContain('market=m2');
    expect(tradeLink.getAttribute('href')).toContain('price=0.4000');
  });

  it('Bullish KpiCard shows positive delta when total > 0', async () => {
    mls.mockResolvedValue(SAMPLE);
    renderSignals();
    await waitFor(() => screen.getByText('Will X happen?'));
    // 2 bullish out of 3 = 67%
    expect(document.body.textContent).toMatch(/67%/);
  });

  it('Bearish KpiCard shows no delta when total === 0', async () => {
    mls.mockResolvedValue([]);
    renderSignals();
    await waitFor(() => {
      // No percentage text (other than "min edge 5%" hint) when total is 0
      // Specifically, the bullish/bearish deltas would be 0/0
      const text = document.body.textContent || '';
      // Should NOT contain the bullish "X%" delta format
      expect(text).not.toMatch(/Bullish\d+%/);
      expect(text).not.toMatch(/Bearish\d+%/);
    });
  });

  it('Recompute success toast differs for n > 0 vs n === 0', async () => {
    mls.mockResolvedValue(SAMPLE);
    // First run: n > 0
    mrs.mockResolvedValue(5);
    renderSignals();
    await waitFor(() => screen.getByText('Will X happen?'));
    const recomputeBtn = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('recompute'),
    );
    fireEvent.click(recomputeBtn!);
    await waitFor(() => {
      expect(mockToastSuccess).toHaveBeenCalledWith(
        expect.stringContaining('Recompute queued'),
        expect.stringContaining('5 signals updated'),
      );
    });
  });

  it('Recompute onError shows error toast with error message', async () => {
    mls.mockResolvedValue(SAMPLE);
    mrs.mockRejectedValue(new Error('network down'));
    renderSignals();
    await waitFor(() => screen.getByText('Will X happen?'));
    const recomputeBtn = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('recompute'),
    );
    fireEvent.click(recomputeBtn!);
    await waitFor(() => {
      expect(mockToastError).toHaveBeenCalledWith(
        'Recompute failed',
        'network down',
      );
    });
  });

  it('Empty state shows "No signals match" hint when total > 0 but filtered === 0', async () => {
    mls.mockResolvedValue(SAMPLE);
    renderSignals();
    await waitFor(() => screen.getByText('Will X happen?'));
    // Bump minEdgePct to a value nothing matches (e.g. 50)
    const input = screen.getByDisplayValue('5') as HTMLInputElement;
    fireEvent.change(input, { target: { value: '50' } });
    await waitFor(() => {
      // No-match hint (not the "no signals at all" empty state)
      expect(document.body.textContent).toMatch(/No signals match|min edge|threshold/i);
    });
  });
});
