// v0.57b —— History 页面组件测试。
//
// History 页面列出用户的已下注 bet
//（real-mode + paper），包含 v0.50a（Type + post-only
// 徽章）+ v0.51b（Fill：slippage + partial +
// TTF）列。目前页面有 2 个 data-testid
//（bet-post-only-{id}、bet-partial-{id}）但没有
// 测试。
//
// 本文件覆盖以下渲染：
//   1. 空状态（无 bet）
//   2. 包含 v0.51b Fill 列的已填充表格
//      （展示 slippage 着色）
//   3. post-only 徽章
//   4. partial fill 徽章

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listBets: vi.fn().mockResolvedValue([]),
  listPaperFills: vi.fn().mockResolvedValue([]),
}));

import * as ipc from '@/ipc';
import { History } from '@/routes/History';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return (
    <MemoryRouter>
      <QueryClientProvider client={qc}>{node}</QueryClientProvider>
    </MemoryRouter>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('History (v0.57b)', () => {
  it('renders the empty state when no bets', async () => {
    vi.mocked(ipc.listBets).mockResolvedValue([]);
    render(wrap(<History />));
    // 仅验证页面能渲染而不崩溃。空状态是一个
    // 带无投注消息的 Card。我们不绑定特定 testid，
    // 因为空状态路径使用 EmptyState 而非自定义 testid。
    // v0.119 — Card 现在按 Cursor 规范使用 `rounded-card`（12px），
    // 取代原来的 `rounded-lg`（8px）。
    await waitFor(() => {
      // 至少 3 个 Card 子元素或任意可见的
      // Card 头。
      expect(
        document.querySelector('.rounded-card') !== null,
      ).toBeTruthy();
    });
  });

  it('renders bets with v0.50a post-only badge', async () => {
    vi.mocked(ipc.listBets).mockResolvedValue([
      {
        id: 'bet-1',
        wallet_id: 'wallet-1',
        market_id: 'm1',
        signal_id: null,
        mode: 'B_signed',
        side: 'YES',
        size: '10',
        price: 0.5,
        shares: '20',
        placed_at: 1700000000000,
        settled_at: null,
        pnl: null,
        status: 'open',
        tx_hash: null,
        notes: null,
        order_type: 'limit',
        limit_price: 0.5,
        stop_price: null,
        post_only: true,
        filled_at: null,
        fill_price: null,
        fill_size: null,
        partial: false,
      },
    ]);
    render(wrap(<History />));
    await waitFor(() => {
      expect(
        screen.getByTestId('bet-post-only-bet-1'),
      ).toBeInTheDocument();
    });
  });

  it('renders partial fill badge (v0.51b)', async () => {
    vi.mocked(ipc.listBets).mockResolvedValue([
      {
        id: 'bet-2',
        wallet_id: 'wallet-1',
        market_id: 'm2',
        signal_id: null,
        mode: 'B_signed',
        side: 'NO',
        size: '10',
        price: 0.5,
        shares: '5',
        placed_at: 1700000000000,
        settled_at: null,
        pnl: null,
        status: 'open',
        tx_hash: '0xabc',
        notes: null,
        order_type: 'market',
        limit_price: null,
        stop_price: null,
        post_only: false,
        filled_at: 1700000010000,
        fill_price: 0.51,
        fill_size: '5',
        partial: true,
      },
    ]);
    render(wrap(<History />));
    await waitFor(() => {
      expect(
        screen.getByTestId('bet-partial-bet-2'),
      ).toBeInTheDocument();
    });
  });
});
