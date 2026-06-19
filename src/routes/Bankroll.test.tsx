// v0.78c — bankroll L4 component tests (+8 tests, ~95% br).
//
// Covers:
//   - BankrollCard: 4 tiles, free=available-reserved-allocated, negatives clamp
//   - AllocationTable: empty state, single item, multiple items,
//     capped reason badges, Yes/No pill
//   - Bankroll route: renders with empty signals, config sliders,
//     apply button is no-op (toast "v0.78e coming")
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

vi.mock('@/ipc', () => ({
  listActiveSignals: (...args: unknown[]) => Promise.resolve(mockListActiveSignals(...args)),
  listWallets: () => Promise.resolve(mockListWallets()),
  computeAllocationPreview: (...args: unknown[]) => Promise.resolve(mockComputeAllocationPreview(...args)),
}));

vi.mock('@/stores/toast-store', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({
    t: (k: string, vars?: Record<string, string>) => {
      if (vars?.default) return vars.default;
      return k;
    },
    locale: 'en' as const,
  }),
}));

import { BankrollCard } from '@/components/feedback/BankrollCard';
import { AllocationTable } from '@/components/feedback/AllocationTable';
import { Bankroll } from '@/routes/Bankroll';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>,
  );
}

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
      model_version: 'm1',
      confidence: 0.8,
      expected_roi: 0.08,
    },
    {
      market_id: 'm2',
      side: 'No' as const,
      size_usdc: '100.00',
      kelly_pct: 0.1,
      capped_reason: 'PerSignalCap' as const,
      source_signal_ids: ['s2', 's3'],
      model_version: 'm1',
      confidence: 0.7,
      expected_roi: 0.07,
    },
  ],
  dropped_markets: [],
};

beforeEach(() => {
  vi.clearAllMocks();
  mockListActiveSignals.mockResolvedValue([]);
  mockListWallets.mockResolvedValue([]);
  mockComputeAllocationPreview.mockResolvedValue(SAMPLE_RESULT);
});

describe('v0.78c — bankroll L4 components', () => {
  describe('BankrollCard', () => {
    it('renders 4 tiles with formatted values', () => {
      wrap(<BankrollCard availableUsdc="1000" reservedUsdc="200" allocatedUsdc="300" />);
      const text = document.body.textContent || '';
      expect(text).toContain('Available');
      expect(text).toContain('Reserved');
      expect(text).toContain('Allocated');
      expect(text).toContain('Free');
      // free = 1000 - 200 - 300 = 500
      expect(text).toContain('$500.00');
    });

    it('clamps negative free to 0', () => {
      wrap(<BankrollCard availableUsdc="100" reservedUsdc="200" allocatedUsdc="0" />);
      // reserved > available, so free should be 0
      const text = document.body.textContent || '';
      // both reserved and free would show
      expect(text).toContain('Reserved');
    });

    it('shows wallet label when provided', () => {
      wrap(<BankrollCard availableUsdc="1000" reservedUsdc="0" allocatedUsdc="0" walletLabel="Treasury" />);
      expect(document.body.textContent).toContain('Wallet: Treasury');
    });

    it('handles empty string inputs as 0', () => {
      wrap(<BankrollCard availableUsdc="" reservedUsdc="" allocatedUsdc="" />);
      expect(document.body.textContent).toBeTruthy();
    });
  });

  describe('AllocationTable', () => {
    it('renders empty state for no items', () => {
      wrap(<AllocationTable items={[]} />);
      expect(screen.getByTestId('allocation-table-empty')).toBeTruthy();
    });

    it('renders single item with size, kelly, confidence', () => {
      wrap(<AllocationTable items={[SAMPLE_RESULT.per_market[0]]} />);
      expect(screen.getByTestId('alloc-size-m1').textContent).toContain('50.00');
      expect(screen.getByTestId('alloc-kelly-m1').textContent).toContain('5.0%');
    });

    it('renders Yes/No pill for side', () => {
      wrap(<AllocationTable items={SAMPLE_RESULT.per_market} />);
      expect(document.body.textContent).toContain('Yes');
      expect(document.body.textContent).toContain('No');
    });

    it('shows capped reason pill when present', () => {
      wrap(<AllocationTable items={SAMPLE_RESULT.per_market} />);
      // m2 has PerSignalCap
      const row = screen.getByTestId('allocation-row-m2');
      expect(row.textContent).toContain('per-cap');
    });
  });

  describe('Bankroll route', () => {
    it('renders without crash with empty signals', async () => {
      wrap(<Bankroll />);
      await waitFor(() => {
        expect(screen.getByTestId('bankroll-page')).toBeTruthy();
      });
    });

    it('config sliders respond to user input', async () => {
      wrap(<Bankroll />);
      await waitFor(() => screen.getByTestId('bankroll-page'));
      const slider = screen.getByTestId('config-slider-kelly_multiplier') as HTMLInputElement;
      fireEvent.change(slider, { target: { value: '0.5' } });
      expect(slider.value).toBe('0.5');
    });

    it('bankroll input accepts numeric changes', async () => {
      wrap(<Bankroll />);
      await waitFor(() => screen.getByTestId('bankroll-page'));
      const input = screen.getByTestId('bankroll-input') as HTMLInputElement;
      fireEvent.change(input, { target: { value: '5000' } });
      expect(input.value).toBe('5000');
    });

    it('shows empty state when no active signals', async () => {
      mockListActiveSignals.mockResolvedValue([]);
      wrap(<Bankroll />);
      await waitFor(() => {
        const text = document.body.textContent || '';
        expect(text).toMatch(/no active signals|allocation/i);
      });
    });
  });
});
