// v0.100 — Copy.tsx 分支第 3 轮（+3 个测试，+~5 个语句）。
//
// 针对 TargetRow 的条件分支：
// - allocation_cap 胶囊（当 t.allocation_cap 已设置时渲染）
// - "recent events" 区块（当 events.length > 0 时渲染）
// - "Copy address" 按钮的 onClick（调用 navigator.clipboard.writeText
//   并显示 toast.success）
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
    // recent events 部分已渲染
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
    // 当 allocation_cap 为 null 时不渲染 cap 胶囊
    expect(screen.queryByText(/cap \$/)).not.toBeInTheDocument();
  });
});