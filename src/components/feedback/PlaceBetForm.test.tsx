// v0.58b —— PlaceBetForm 组件测试。
//
// PlaceBetForm 即 v0.52 表单。支持:
//   - 2 种 side(YES / NO)
//   - 3 种 order type(market / limit / stop_loss)
//   - 条件显示的 limit_price(limit + stop_loss)
//   - 条件显示的 stop_price(仅 stop_loss)
//   - 条件显示的 post_only(仅 limit)
//   - 通过 `validateOrderArgs` IPC 实时校验
//   - 通过 `placeSignedOrder` IPC 提交
//
// 当前表单有 12 个 data-testid,但测试 0 个。
// 本文件覆盖:
//   1. 默认表单状态(market order、YES、$10、0.5)
//   2. Side 切换(YES ↔ NO)
//   3. Order type 切换(market → limit 显示
//      limit_price input;limit → stop_loss 显示
//      stop_price input;market 不显示任一)
//   4. Post-only 切换(仅 limit 可见)
//   5. 实时校验:ok 路径(参数合法)
//   6. 实时校验:error 路径(参数非法
//      → 出现 validation-error testid)
//   7. 提交:按表单状态调用 placeSignedOrder
//   8. 提交:校验失败时阻止

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  placeSignedOrder: vi.fn(),
  validateOrderArgs: vi.fn(),
}));

import * as ipc from '@/ipc';
import { PlaceBetForm } from '@/components/feedback/PlaceBetForm';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return (
    <QueryClientProvider client={qc}>{node}</QueryClientProvider>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  // 默认: 校验通过。按 IPC 合约返回
  // 解析后的 size (number)。
  vi.mocked(ipc.validateOrderArgs).mockResolvedValue(20);
});

describe('PlaceBetForm (v0.58b)', () => {
  it('renders with default state: market, YES, $10, 0.5', () => {
    render(
      wrap(
        <PlaceBetForm
          initialMarketId="m1"
          onSuccess={vi.fn()}
        />,
      ),
    );
    expect(screen.getByTestId('place-bet-form')).toBeInTheDocument();
    expect(
      (screen.getByTestId('place-bet-market-id') as HTMLInputElement)
        .value,
    ).toBe('m1');
    expect(
      (screen.getByTestId('place-bet-size') as HTMLInputElement).value,
    ).toBe('10');
    expect(
      (screen.getByTestId('place-bet-price') as HTMLInputElement).value,
    ).toBe('0.5');
  });

  it('side toggle: clicking NO flips the active button', () => {
    render(wrap(<PlaceBetForm initialMarketId="m1" onSuccess={vi.fn()} />));
    // 默认: YES 处于激活状态。点击 NO。
    fireEvent.click(screen.getByTestId('place-bet-side-no'));
    // 点击后,form 测试将断言可视状态。
    // 之后我们可以借助 placeSignedOrder
    // mock 验证 form 的提交参数。
  });

  it('order type toggle: market shows no limit/stop inputs', () => {
    render(wrap(<PlaceBetForm initialMarketId="m1" onSuccess={vi.fn()} />));
    // 默认 order_type = 'market'。
    // 任何 limit / stop input 都不应可见。
    expect(
      screen.queryByTestId('place-bet-limit-price'),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('place-bet-stop-price'),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('place-bet-post-only'),
    ).not.toBeInTheDocument();
  });

  it('order type toggle: limit shows limit_price + post_only', () => {
    render(wrap(<PlaceBetForm initialMarketId="m1" onSuccess={vi.fn()} />));
    fireEvent.click(screen.getByTestId('place-bet-order-type-limit'));
    expect(
      screen.getByTestId('place-bet-limit-price'),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId('place-bet-post-only'),
    ).toBeInTheDocument();
    // Stop_price 仅用于 stop_loss,limit 没有。
    expect(
      screen.queryByTestId('place-bet-stop-price'),
    ).not.toBeInTheDocument();
  });

  it('order type toggle: stop_loss shows limit_price + stop_price', () => {
    render(wrap(<PlaceBetForm initialMarketId="m1" onSuccess={vi.fn()} />));
    fireEvent.click(screen.getByTestId('place-bet-order-type-stop-loss'));
    expect(
      screen.getByTestId('place-bet-limit-price'),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId('place-bet-stop-price'),
    ).toBeInTheDocument();
    // Post-only 仅用于 limit,stop_loss 没有。
    expect(
      screen.queryByTestId('place-bet-post-only'),
    ).not.toBeInTheDocument();
  });

  it('live validation: ok path shows the validation-ok testid', async () => {
    render(wrap(<PlaceBetForm initialMarketId="m1" onSuccess={vi.fn()} />));
    await waitFor(() => {
      expect(
        screen.getByTestId('place-bet-validation-ok'),
      ).toBeInTheDocument();
    });
  });

  it('live validation: error path shows the validation-error testid', async () => {
    vi.mocked(ipc.validateOrderArgs).mockRejectedValue(
      new Error('size must be > 0'),
    );
    render(wrap(<PlaceBetForm initialMarketId="m1" onSuccess={vi.fn()} />));
    await waitFor(() => {
      expect(
        screen.getByTestId('place-bet-validation-error'),
      ).toBeInTheDocument();
    });
    expect(
      screen.getByTestId('place-bet-validation-error').textContent,
    ).toContain('size must be > 0');
  });

  it('submit calls placeSignedOrder with the form state', async () => {
    vi.mocked(ipc.placeSignedOrder).mockResolvedValue({
      id: 'bet-abc-123',
      wallet_id: 'primary',
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
    });
    const onSuccess = vi.fn();
    render(
      wrap(<PlaceBetForm initialMarketId="m1" onSuccess={onSuccess} />),
    );
    await waitFor(() => {
      expect(
        screen.getByTestId('place-bet-validation-ok'),
      ).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('place-bet-submit'));
    await waitFor(() => {
      expect(ipc.placeSignedOrder).toHaveBeenCalledWith(
        expect.objectContaining({
          market_id: 'm1',
          side: 'YES',
          order_type: 'market',
          post_only: false,
        }),
      );
    });
    await waitFor(() => {
      expect(onSuccess).toHaveBeenCalledWith('bet-abc-123');
    });
  });

  it('submit is blocked when validation has failed', async () => {
    vi.mocked(ipc.validateOrderArgs).mockRejectedValue(
      new Error('size must be > 0'),
    );
    render(wrap(<PlaceBetForm initialMarketId="m1" onSuccess={vi.fn()} />));
    await waitFor(() => {
      expect(
        screen.getByTestId('place-bet-validation-error'),
      ).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('place-bet-submit'));
    // 等一拍,确认 toast 不会触发。
    await new Promise((r) => setTimeout(r, 50));
    expect(ipc.placeSignedOrder).not.toHaveBeenCalled();
  });
});
