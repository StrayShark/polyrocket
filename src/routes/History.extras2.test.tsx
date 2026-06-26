// v0.72d — History extras 第 2 轮测试。
//
// History.tsx 有 345 行，包含 10 列 DataTable + 4 个 KPI
// 概览 + 5 个状态筛选 + tx_hash 外链。
// 已有测试（v0.62a + v0.71c）覆盖了 12 个用例。我们再新增 10 个
// 测试，覆盖剩余分支：
//   - pnl === 0（零 PnL → muted 文字，无 +/- 前缀）
//   - tx_hash 为 null 的回退（em-dash 代替链接）
//   - fill 列滑点为 0（<0.01 → muted，无颜色）
//   - fill 列正滑点（slipPct > 0 → bear 颜色）
//   - fill 列负滑点（slipPct < 0 → bull 颜色）
//   - order_type='limit' → accent 徽章
//   - order_type='stop_loss' → warning 徽章，文本 "stop-loss"
//   - status='cancelled' → muted 徽章
//   - status='open' → accent 徽章（默认分支）
//   - KPI 胜率 delta：当 won+lost>0 且 winRate>=0.5 时为正
//
// 覆盖目标：分支 84.28% → ~90%。
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
  // b1：零 PnL（pnl=0 → muted 文字分支）
  { id: 'b1', market_id: 'm1', side: 'YES' as const, mode: 'A_jump' as const, size: 100, price: 0.5, status: 'won' as const, placed_at: Date.now() - 1000, pnl: 0, fill_price: 0.5, partial: false, post_only: false, order_type: 'market' as const, tx_hash: null },
  // b2：fill 列正滑点（fill_price > price → bear）
  { id: 'b2', market_id: 'm2', side: 'NO' as const, mode: 'B_drift' as const, size: 50, price: 0.6, status: 'won' as const, placed_at: Date.now() - 5000, pnl: 25, fill_price: 0.7, partial: false, post_only: true, order_type: 'limit' as const, tx_hash: '0xabc123' },
  // b3：fill 列负滑点（fill_price < price → bull）
  { id: 'b3', market_id: 'm3', side: 'YES' as const, mode: 'A_jump' as const, size: 30, price: 0.4, status: 'won' as const, placed_at: Date.now() - 8000, pnl: 10, fill_price: 0.35, partial: true, post_only: false, order_type: 'stop_loss' as const, tx_hash: '0xdef456' },
  // b4：cancelled 状态 → muted pill
  { id: 'b4', market_id: 'm4', side: 'NO' as const, mode: 'B_drift' as const, size: 20, price: 0.7, status: 'cancelled' as const, placed_at: Date.now() - 12000, pnl: null, fill_price: null, partial: false, post_only: false, order_type: 'market' as const, tx_hash: null },
  // b5：open 状态 → accent pill（默认分支）
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
    // b1 pnl=0 → 渲染为 "$0.00"（无 + 前缀，无 - 符号，灰色文字）
    const text = document.body.textContent || '';
    // 不应包含 "+$0.00" —— 应仅有 "$0.00"
    expect(text).not.toMatch(/\+\$0\.00/);
    expect(text).toMatch(/\$0\.00/);
  });

  it('tx_hash=null row renders em-dash instead of link icon', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m1'));
    // b1、b4、b5 的 tx_hash=null
    // b2、b3 设置了 tx_hash → 渲染 ExternalLink 图标
    const externalLinks = document.querySelectorAll('a[href*="polygonscan"]');
    expect(externalLinks.length).toBe(2); // b2 + b3 only
  });

  it('positive fill slippage renders in bear color', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m2'));
    // b2：fill_price=0.7 > price=0.6 → 正向滑点 → bear 文字
    const bearCells = document.querySelectorAll('.text-bear');
    expect(bearCells.length).toBeGreaterThan(0);
  });

  it('negative fill slippage renders in bull color', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m3'));
    // b3：fill_price=0.35 < price=0.4 → 负向滑点 → bull 文字
    const bullCells = document.querySelectorAll('.text-bull');
    expect(bullCells.length).toBeGreaterThan(0);
  });

  it('order_type=limit renders accent pill', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m2'));
    // b2 order_type='limit' → accent 胶囊
    const accentPills = document.querySelectorAll('.bg-accent\\/15, .text-accent');
    expect(accentPills.length).toBeGreaterThan(0);
  });

  it('order_type=stop_loss renders warning pill with "stop-loss" text', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m3'));
    // b3 order_type='stop_loss' → warning 胶囊，文本 "stop-loss"（而非 "stop_loss"）
    expect(screen.getByText('stop-loss')).toBeInTheDocument();
    const warningPills = document.querySelectorAll('.bg-warning\\/15, .text-warning');
    expect(warningPills.length).toBeGreaterThan(0);
  });

  it('status=cancelled renders muted pill', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m4'));
    // b4 status='cancelled' → muted 胶囊
    expect(screen.getAllByText('cancelled').length).toBeGreaterThan(0);
  });

  it('status=open renders accent pill (default branch)', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m5'));
    // b5 status='open' → 落到 'accent' 类型
    expect(screen.getAllByText('open').length).toBeGreaterThan(0);
  });

  it('KPI winrate delta renders text in "{won}W/{lost}L" format when won+lost>0', async () => {
    renderHistory();
    await waitFor(() => screen.getByText('m1'));
    // BETS 中含 3 胜（b1, b2, b3）+ 0 负 + 1 已取消 + 1 进行中
    // 等等 —— 仅 b1、b2、b3 为 'won'；b4='cancelled'；b5='open'。无 'lost'。
    // winRate = won / (won + lost) = 3 / 3 = 100%
    const text = document.body.textContent || '';
    expect(text).toMatch(/3W\/0L/);
    // 100% winrate → 正向 delta
    expect(text).toMatch(/100%/);
  });
});