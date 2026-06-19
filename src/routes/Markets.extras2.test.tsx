// v0.72e — Markets extras round 2 tests.
//
// Markets.tsx is 251 lines with 7-column DataTable + 7 category
// chips + active_only toggle + syncMarkets mutation + search
// debounce. Existing tests (v0.66 + v0.70h) cover surface cases.
// We add 10 tests covering the remaining branches:
//   - status column: resolved=true with outcome='YES' (Pill shows outcome)
//   - status column: resolved=true with outcome=null (Pill shows 'resolved')
//   - status column: active=true → 'active' bull pill
//   - status column: active=false → 'inactive' muted pill
//   - category filter chip non-default (e.g. 'crypto')
//   - active_only checkbox toggle → refetch with new query key
//   - search by slug field (m.slug contains q)
//   - syncMut onSuccess → toast + invalidate queries
//   - syncMut onError → toast.error
//   - Empty state with search term shows "No markets matching ..."
//
// Coverage target: branches 65.71% → ~80%.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';
import { useToastStore } from '@/stores/toast-store';

const { mockListMarkets: mlm, mockSyncMarkets: msm } = vi.hoisted(() => ({
  mockListMarkets: vi.fn(),
  mockSyncMarkets: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listMarkets: (...args: unknown[]) => mlm(...args),
  syncMarkets: (...args: unknown[]) => msm(...args),
}));

import { Markets } from './Markets';

function renderMarkets() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Markets />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const MARKETS = [
  // m1: resolved=true, outcome='YES' → outcome pill
  { id: 'm1', question: 'Q1?', slug: 'q1', category: 'crypto', liquidity: '1000', volume_24h: '500', end_date: 9999999999, active: false, resolved: true, outcome: 'YES' },
  // m2: resolved=true, outcome=null → 'resolved' fallback pill
  { id: 'm2', question: 'Q2?', slug: 'q2', category: 'crypto', liquidity: '2000', volume_24h: '600', end_date: 9999999999, active: false, resolved: true, outcome: null },
  // m3: active=true → 'active' bull pill
  { id: 'm3', question: 'Q3?', slug: 'q3', category: 'crypto', liquidity: '3000', volume_24h: '700', end_date: 9999999999, active: true, resolved: false, outcome: null },
  // m4: active=false (not resolved) → 'inactive' muted pill
  { id: 'm4', question: 'Q4?', slug: 'q4', category: 'football', liquidity: '4000', volume_24h: '800', end_date: 9999999999, active: false, resolved: false, outcome: null },
  // m5: cs2 category for filter test
  { id: 'm5', question: 'Q5?', slug: 'q5', category: 'cs2', liquidity: '5000', volume_24h: '900', end_date: 9999999999, active: true, resolved: false, outcome: null },
];

beforeEach(() => {
  vi.clearAllMocks();
  mlm.mockResolvedValue(MARKETS);
  msm.mockResolvedValue(3);
  useToastStore.setState({ toasts: [] });
});

describe('Markets (extras round 2 — v0.72e)', () => {
  it('status=resolved with outcome=YES renders outcome pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    expect(screen.getAllByText('YES').length).toBeGreaterThan(0);
  });

  it('status=resolved with outcome=null renders "resolved" pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q2?'));
    expect(screen.getAllByText('resolved').length).toBeGreaterThan(0);
  });

  it('status=active=true renders "active" bull pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q3?'));
    expect(screen.getAllByText('active').length).toBeGreaterThan(0);
  });

  it('status=active=false (not resolved) renders "inactive" muted pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q4?'));
    expect(screen.getAllByText('inactive').length).toBeGreaterThan(0);
  });

  it('click category chip "crypto" filters to crypto-only markets', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const cryptoChip = screen.getAllByRole('button').find(b =>
      b.textContent?.trim() === 'crypto',
    );
    expect(cryptoChip).toBeDefined();
    fireEvent.click(cryptoChip!);
    await waitFor(() => {
      expect(screen.getByText('Q1?')).toBeInTheDocument();
      // Q5 is cs2, should be filtered out
      expect(screen.queryByText('Q5?')).not.toBeInTheDocument();
    });
  });

  it('click category chip "cs2" filters to cs2-only markets', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const cs2Chip = screen.getAllByRole('button').find(b =>
      b.textContent?.trim() === 'cs2',
    );
    expect(cs2Chip).toBeDefined();
    fireEvent.click(cs2Chip!);
    await waitFor(() => {
      expect(screen.getByText('Q5?')).toBeInTheDocument();
      expect(screen.queryByText('Q1?')).not.toBeInTheDocument();
    });
  });

  it('toggle active_only checkbox triggers refetch with new query key', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const callsBefore = mlm.mock.calls.length;
    const checkbox = screen.getByRole('checkbox') as HTMLInputElement;
    expect(checkbox.checked).toBe(true);
    fireEvent.click(checkbox);
    await waitFor(() => {
      expect(mlm.mock.calls.length).toBeGreaterThan(callsBefore);
    });
    expect(checkbox.checked).toBe(false);
  });

  it('search by slug field (matches slug text)', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'q5' } });
    await waitFor(() => {
      expect(screen.getByText('Q5?')).toBeInTheDocument();
      expect(screen.queryByText('Q1?')).not.toBeInTheDocument();
    });
  });

  it('click Sync button → syncMarkets + toast.success', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const syncBtn = screen.getAllByRole('button').find(b =>
      /^sync/i.test(b.textContent?.trim() || ''),
    );
    expect(syncBtn).toBeDefined();
    fireEvent.click(syncBtn!);
    await waitFor(() => {
      expect(msm).toHaveBeenCalled();
      const { toasts } = useToastStore.getState();
      const succ = toasts.find(t => t.kind === 'success');
      expect(succ).toBeTruthy();
      expect(succ?.body || succ?.title).toMatch(/3.*markets|synced/i);
    });
  });

  it('Sync mutation onError → toast.error', async () => {
    msm.mockRejectedValue(new Error('sync failed'));
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const syncBtn = screen.getAllByRole('button').find(b =>
      /^sync/i.test(b.textContent?.trim() || ''),
    );
    fireEvent.click(syncBtn!);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find(t => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body || err?.title).toMatch(/sync/i);
    });
  });

  it('Empty state with search shows "No markets matching" copy', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'xxxxxx-no-match' } });
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/no.markets.matching|markets.empty/i);
    });
  });
});