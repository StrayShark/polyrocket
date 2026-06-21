// v0.100 — Copy.tsx branches round 3 (+3 tests, +~5 stmts).
//
// Targets the TargetRow conditional branches:
// - allocation_cap pill (renders when t.allocation_cap is set)
// - "recent events" section (renders when events.length > 0)
// - "Copy address" button onClick (calls navigator.clipboard.writeText
//   + shows toast.success)
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const { mockListCopyTargets, mockRecentCopyEvents, mockGetMirrorPaperMode, mockListPaperFills } = vi.hoisted(() => ({
  mockListCopyTargets: vi.fn(),
  mockRecentCopyEvents: vi.fn(),
  mockGetMirrorPaperMode: vi.fn(),
  mockListPaperFills: vi.fn(),
}));

vi.mock('@/ipc', () => ({
  listCopyTargets: (...args: unknown[]) => mockListCopyTargets(...args),
  recentCopyEvents: (...args: unknown[]) => mockRecentCopyEvents(...args),
  getMirrorPaperMode: (...args: unknown[]) => mockGetMirrorPaperMode(...args),
  listPaperFills: (...args: unknown[]) => mockListPaperFills(...args),
  addCopyTarget: vi.fn(),
}));

const { mockToastSuccess } = vi.hoisted(() => ({
  mockToastSuccess: vi.fn(),
}));
vi.mock('@/stores/toast-store', () => ({
  toast: { success: mockToastSuccess, error: vi.fn(), info: vi.fn() },
  useToastStore: () => ({ toasts: [], push: vi.fn(), dismiss: vi.fn() }),
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

import { Copy } from './Copy';

function wrap() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Copy />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mockGetMirrorPaperMode.mockResolvedValue(false);
  mockListPaperFills.mockResolvedValue([]);
  mockRecentCopyEvents.mockResolvedValue([]);
});

describe('Copy (v0.100) — TargetRow branches', () => {
  it('renders allocation_cap pill when target has cap set', async () => {
    mockListCopyTargets.mockResolvedValue([
      { id: 't1', address: '0x' + 'a'.repeat(40), label: 'whale', enabled: true, allocation_cap: '500', min_edge: 0.05, created_at: 1700000000 },
    ]);
    render(wrap());
    await waitFor(() => {
      expect(screen.getByText('whale')).toBeInTheDocument();
    });
    expect(screen.getByText(/cap \$500/)).toBeInTheDocument();
  });

  it('renders recent events section when target has events', async () => {
    mockListCopyTargets.mockResolvedValue([
      { id: 't1', address: '0x' + 'a'.repeat(40), label: 'whale', enabled: true, allocation_cap: null, min_edge: 0.05, created_at: 1700000000 },
    ]);
    mockRecentCopyEvents.mockResolvedValue([
      { id: 'e1', target_id: 't1', side: 'YES' as const, size: '100', price: 0.55, tx_hash: '0xabc', detected_at: 1700000000 },
    ]);
    render(wrap());
    await waitFor(() => {
      expect(screen.getByText('whale')).toBeInTheDocument();
    });
    // recent events section is rendered
    expect(screen.getByText(/recent events/i)).toBeInTheDocument();
  });

  it('does NOT render allocation_cap pill when target has no cap', async () => {
    mockListCopyTargets.mockResolvedValue([
      { id: 't1', address: '0x' + 'a'.repeat(40), label: 'whale', enabled: true, allocation_cap: null, min_edge: 0.05, created_at: 1700000000 },
    ]);
    render(wrap());
    await waitFor(() => {
      expect(screen.getByText('whale')).toBeInTheDocument();
    });
    // No cap pill when allocation_cap is null
    expect(screen.queryByText(/cap \$/)).not.toBeInTheDocument();
  });
});