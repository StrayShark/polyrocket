// v0.89a — Bankroll.tsx branches round (+18 tests, branches 36.36→70%).
//
// Covers Bankroll.tsx branches NOT covered by Bankroll.test.tsx (v0.78c):
//   - signalsQuery.error → ErrorState render
//   - walletsQuery.error → "Failed to load wallets" message
//   - allocationQuery.data truthy + per_market non-empty → AllocationResultView
//   - allocationQuery.data truthy + per_market empty → Apply disabled
//   - allocationQuery.data truthy + dropped_markets → "Dropped" message
//   - applyMut success path → toast + setBankrollConfig + invalidate
//   - applyMut error path → toast.error
//   - applyMut.mutationFn with no activeWallet → throws
//   - bankrollInput = '0' / empty → allocationQuery disabled
//   - activeWallet undefined → walletLabel '—'
//   - activeWallet exists → wallet label rendered
//   - Recompute button onClick → refetch called
//   - ConfigSliders: min_edge_pct + max_total_exposure_pct slider changes
//     (kelly_multiplier + max_per_signal_pct already covered in v0.78c)
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/lib/format', () => ({
  fmtUsdc: (v: number) => `$${v.toFixed(2)}`,
  fmtPct: (v: number) => `${(v * 100).toFixed(1)}%`,
  fmtConfidence: (v: number) => v.toFixed(2),
  fmtCents: (v: number) => `${v}c`,
  fmtRelativeTime: (v: number) => `${v}s ago`,
}));

const mockListActiveSignals = vi.fn();
const mockListWallets = vi.fn();
const mockComputeAllocationPreview = vi.fn();
const mockApplyAllocation = vi.fn();
const mockSetBankrollConfig = vi.fn();

vi.mock('@/ipc', () => ({
  listActiveSignals: (...args: unknown[]) => Promise.resolve(mockListActiveSignals(...args)),
  listWallets: () => Promise.resolve(mockListWallets()),
  computeAllocationPreview: (...args: unknown[]) => Promise.resolve(mockComputeAllocationPreview(...args)),
  applyAllocation: (...args: unknown[]) => Promise.resolve(mockApplyAllocation(...args)),
  setBankrollConfig: (...args: unknown[]) => Promise.resolve(mockSetBankrollConfig(...args)),
}));

vi.mock('@/stores/toast-store', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({
    t: (k: string, vars?: Record<string, string>) => vars?.default ?? k,
    locale: 'en' as const,
  }),
}));

import { Bankroll } from '@/routes/Bankroll';
import { toast } from '@/stores/toast-store';

function wrap() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Bankroll />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const SAMPLE_SIGNAL = {
  market_id: 'm1',
  computed_at: 1700000000,
  model_version: 'v1',
  predicted_prob: 0.7,
  market_prob: 0.5,
  edge: 0.2,
  confidence: 0.8,
  horizon_hours: 24,
  rationale: null,
};

const SAMPLE_RESULT = {
  total_allocated_usdc: '150.00',
  reserved_usdc: '200.00',
  per_market: [
    {
      market_id: 'm1',
      side: 'Yes' as const,
      size_usdc: '50.00',
      kelly_pct: 0.05,
      capped_reason: null,
      source_signal_ids: ['s1'],
      model_version: 'v1',
      confidence: 0.8,
      expected_roi: 0.08,
    },
  ],
  dropped_markets: [],
};

const SAMPLE_RESULT_WITH_DROPS = {
  ...SAMPLE_RESULT,
  dropped_markets: ['m_liquid_low', 'm_no_book'],
};

const SAMPLE_RESULT_EMPTY = {
  ...SAMPLE_RESULT,
  per_market: [],
};

const SAMPLE_WALLET = {
  id: 'w1',
  address: '0xabc',
  label: 'Treasury',
  chain_id: 137,
  wallet_type: 'eoa',
  created_at: 1700000000,
  last_synced_at: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  // Default happy path: signals + wallet + result all loaded
  mockListActiveSignals.mockResolvedValue([SAMPLE_SIGNAL]);
  mockListWallets.mockResolvedValue([SAMPLE_WALLET]);
  mockComputeAllocationPreview.mockResolvedValue(SAMPLE_RESULT);
  mockApplyAllocation.mockResolvedValue('batch-uuid-1234');
  mockSetBankrollConfig.mockResolvedValue(undefined);
});

describe('v0.89a — Bankroll.tsx branches round', () => {
  // 1. signalsQuery.error → ErrorState
  it('shows ErrorState when signalsQuery fails', async () => {
    mockListActiveSignals.mockRejectedValue(new Error('signals backend down'));
    wrap();
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/signals backend down/i);
    });
  });

  // 2. walletsQuery.error → "Failed to load wallets"
  it('shows wallet-load-failed banner when walletsQuery fails', async () => {
    mockListWallets.mockRejectedValue(new Error('wallet backend down'));
    wrap();
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/failed to load wallets/i);
    });
  });

  // 3. allocationQuery.data truthy + per_market non-empty → AllocationResultView with Apply enabled
  it('renders allocation result with Apply button enabled when result has items', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    const btn = screen.getByTestId('alloc-apply') as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
    expect(document.body.textContent).toContain('Apply allocation');
  });

  // 4. allocationQuery.data with empty per_market → Apply disabled
  it('disables Apply button when result has no items', async () => {
    mockComputeAllocationPreview.mockResolvedValue(SAMPLE_RESULT_EMPTY);
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    const btn = screen.getByTestId('alloc-apply') as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  // 5. dropped_markets → "Dropped (liquidity)" message
  it('shows dropped markets message when result has drops', async () => {
    mockComputeAllocationPreview.mockResolvedValue(SAMPLE_RESULT_WITH_DROPS);
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    expect(document.body.textContent).toContain('Dropped (liquidity)');
    expect(document.body.textContent).toContain('m_liquid_low');
  });

  // 6. applyMut success → applyAllocation called + setBankrollConfig persists
  it('calls applyAllocation + persists config on apply success', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    fireEvent.click(screen.getByTestId('alloc-apply'));
    await waitFor(() => {
      expect(mockApplyAllocation).toHaveBeenCalled();
      expect(mockSetBankrollConfig).toHaveBeenCalledWith('w1', expect.any(Object));
    });
  });

  // 7. applyMut error → toast.error
  it('fires error toast when applyAllocation rejects', async () => {
    mockApplyAllocation.mockRejectedValue(new Error('apply failed: insufficient bankroll'));
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    fireEvent.click(screen.getByTestId('alloc-apply'));
    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        'Apply failed',
        expect.stringContaining('apply failed: insufficient bankroll'),
      );
    });
  });

  // 8. applyMut.mutationFn with no activeWallet → throws (caught by react-query, surfaces as error)
  it('shows apply error when no active wallet exists', async () => {
    mockListWallets.mockResolvedValue([]); // no wallets → activeWallet undefined
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    fireEvent.click(screen.getByTestId('alloc-apply'));
    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        'Apply failed',
        expect.stringContaining('no active wallet'),
      );
    });
  });

  // 9. bankrollInput = '0' → allocationQuery disabled (parseFloat > 0 is false)
  it('hides AllocationResultView when bankroll input is 0', async () => {
    wrap();
    // Wait for initial allocationQuery call to complete (alloc-apply visible)
    await waitFor(() => screen.getByTestId('alloc-apply'));
    // Set bankroll to 0 — query becomes disabled, no data
    const input = screen.getByTestId('bankroll-input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: '0' } });
    // AllocationResultView should disappear (data becomes undefined)
    await waitFor(() => {
      expect(screen.queryByTestId('alloc-apply')).toBeNull();
    }, { timeout: 2000 });
  });

  // 10. bankrollInput = '' → bankrollUsdc = '0' (falsy fallback)
  it('treats empty bankroll input as 0', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const input = screen.getByTestId('bankroll-input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: '' } });
    await waitFor(() => {
      expect(input.value).toBe('');
    });
    // No crash; page still renders
    expect(screen.getByTestId('bankroll-page')).toBeTruthy();
  });

  // 11. activeWallet undefined → walletLabel '—'
  it('shows em-dash for wallet label when no wallets exist', async () => {
    mockListWallets.mockResolvedValue([]);
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    // Wait for activeWalletQuery to resolve and BankrollCard to render with '—' label
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/Wallet:\s*—/);
    }, { timeout: 2000 });
  });

  // 12. activeWallet exists → wallet label rendered
  it('shows wallet label when wallet exists', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    // Wait for wallet query to resolve and BankrollCard to render
    await waitFor(() => {
      expect(document.body.textContent).toContain('Wallet: Treasury');
    }, { timeout: 2000 });
  });

  // 13. Recompute button onClick → allocationQuery.refetch called
  it('calls computeAllocationPreview again when Recompute is clicked', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    mockComputeAllocationPreview.mockClear();
    fireEvent.click(screen.getByTestId('bankroll-compute'));
    await waitFor(() => {
      expect(mockComputeAllocationPreview).toHaveBeenCalled();
    });
  });

  // 14. ConfigSliders: min_edge_pct slider change
  it('responds to min_edge_pct slider change', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const slider = screen.getByTestId('config-slider-min_edge_pct') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.15' } });
    expect(slider.value).toBe('0.15');
    expect(document.body.textContent).toMatch(/15%/);
  });

  // 15. ConfigSliders: max_total_exposure_pct slider change
  it('responds to max_total_exposure_pct slider change', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const slider = screen.getByTestId('config-slider-max_total_exposure_pct') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.5' } });
    expect(slider.value).toBe('0.5');
    expect(document.body.textContent).toMatch(/50% of bankroll/);
  });

  // 16. ConfigSliders: reserve_pct slider change
  it('responds to reserve_pct slider change', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const slider = screen.getByTestId('config-slider-reserve_pct') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.4' } });
    expect(slider.value).toBe('0.4');
    expect(document.body.textContent).toMatch(/40% kept/);
  });

  // 17. applyMut success with NO activeWallet persists config — wait, this is the throw case.
  // Better: apply success when activeWallet?.id is undefined (only wallet exists but no id)
  it('skips setBankrollConfig persist when wallet has no id', async () => {
    mockListWallets.mockResolvedValue([
      { ...SAMPLE_WALLET, id: undefined as unknown as string },
    ]);
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    fireEvent.click(screen.getByTestId('alloc-apply'));
    await waitFor(() => {
      expect(toast.success).toHaveBeenCalled();
      expect(mockSetBankrollConfig).not.toHaveBeenCalled();
    });
  });

  // 18. applyMut onSuccess calls setBankrollConfig with current config snapshot
  it('persists current config snapshot (not initial defaults) on apply success', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    // change a slider first to ensure config is updated
    const slider = screen.getByTestId('config-slider-kelly_multiplier') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.5' } });
    // Wait for re-render after config change so alloc-apply is back
    await waitFor(() => screen.getByTestId('alloc-apply'), { timeout: 2000 });
    fireEvent.click(screen.getByTestId('alloc-apply'));
    await waitFor(() => {
      expect(mockSetBankrollConfig).toHaveBeenCalledWith(
        'w1',
        expect.objectContaining({ kelly_multiplier: 0.5 }),
      );
    });
  });
});
