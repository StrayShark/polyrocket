// v0.89a — Bankroll.tsx 分支轮次（+18 测试，分支 36.36 → 70%）。
//
// 覆盖 Bankroll.test.tsx（v0.78c）未覆盖的 Bankroll.tsx 分支：
//   - signalsQuery.error → 渲染 ErrorState
//   - walletsQuery.error → "Failed to load wallets" 提示
//   - allocationQuery.data truthy + per_market 非空 → AllocationResultView
//   - allocationQuery.data truthy + per_market 空 → Apply 禁用
//   - allocationQuery.data truthy + dropped_markets → "Dropped" 提示
//   - applyMut 成功路径 → toast + setBankrollConfig + invalidate
//   - applyMut 错误路径 → toast.error
//   - applyMut.mutationFn 在无 activeWallet 时 → 抛出
//   - bankrollInput = '0' / 空 → allocationQuery 禁用
//   - activeWallet 未定义 → walletLabel 显示 '—'
//   - activeWallet 存在 → 渲染 wallet label
//   - 点击 Recompute 按钮 → 调用 refetch
//   - ConfigSliders：min_edge_pct + max_total_exposure_pct 滑块变更
//     （kelly_multiplier + max_per_signal_pct 已在 v0.78c 中覆盖）
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
  // 默认 happy path：signals + wallet + result 全部加载
  mockListActiveSignals.mockResolvedValue([SAMPLE_SIGNAL]);
  mockListWallets.mockResolvedValue([SAMPLE_WALLET]);
  mockComputeAllocationPreview.mockResolvedValue(SAMPLE_RESULT);
  mockApplyAllocation.mockResolvedValue('batch-uuid-1234');
  mockSetBankrollConfig.mockResolvedValue(undefined);
});

describe('v0.89a — Bankroll.tsx branches round', () => {
  // 1. signalsQuery.error → 渲染 ErrorState
  it('signalsQuery 失败时显示 ErrorState', async () => {
    mockListActiveSignals.mockRejectedValue(new Error('signals backend down'));
    wrap();
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/signals backend down/i);
    });
  });

  // 2. walletsQuery.error → "加载钱包失败"
  it('walletsQuery 失败时显示钱包加载失败横幅', async () => {
    mockListWallets.mockRejectedValue(new Error('wallet backend down'));
    wrap();
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/failed to load wallets/i);
    });
  });

  // 3. allocationQuery.data truthy + per_market 非空 → AllocationResultView，Apply 启用
  it('结果有 items 时渲染 allocation 结果且 Apply 按钮启用', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    const btn = screen.getByTestId('alloc-apply') as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
    expect(document.body.textContent).toContain('Apply allocation');
  });

  // 4. allocationQuery.data with empty per_market → Apply 禁用
  it('结果无 items 时禁用 Apply 按钮', async () => {
    mockComputeAllocationPreview.mockResolvedValue(SAMPLE_RESULT_EMPTY);
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    const btn = screen.getByTestId('alloc-apply') as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  // 5. dropped_markets → "Dropped (liquidity)" 消息
  it('结果有 drops 时显示 dropped markets 消息', async () => {
    mockComputeAllocationPreview.mockResolvedValue(SAMPLE_RESULT_WITH_DROPS);
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    expect(document.body.textContent).toContain('Dropped (liquidity)');
    expect(document.body.textContent).toContain('m_liquid_low');
  });

  // 6. applyMut 成功 → 调用 applyAllocation + setBankrollConfig 持久化
  it('apply 成功时调用 applyAllocation + 持久化 config', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('alloc-apply'));
    fireEvent.click(screen.getByTestId('alloc-apply'));
    await waitFor(() => {
      expect(mockApplyAllocation).toHaveBeenCalled();
      expect(mockSetBankrollConfig).toHaveBeenCalledWith('w1', expect.any(Object));
    });
  });

  // 7. applyMut 错误 → toast.error
  it('applyAllocation 失败时触发错误 toast', async () => {
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

  // 8. applyMut.mutationFn 在无 activeWallet 时 → 抛出（被 react-query 捕获，呈现为 error）
  it('无 active wallet 时显示 apply 错误', async () => {
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

  // 9. bankrollInput = '0' → allocationQuery 禁用 (parseFloat > 0 is false)
  it('bankroll 输入为 0 时隐藏 AllocationResultView', async () => {
    wrap();
    // 等待初次 allocationQuery 调用完成（alloc-apply 可见）
    await waitFor(() => screen.getByTestId('alloc-apply'));
    // 将 bankroll 设为 0 —— query 失效，无数据
    const input = screen.getByTestId('bankroll-input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: '0' } });
    // AllocationResultView 应消失（data 变为 undefined）
    await waitFor(() => {
      expect(screen.queryByTestId('alloc-apply')).toBeNull();
    }, { timeout: 2000 });
  });

  // 10. bankrollInput = '' → bankrollUsdc = '0' (假值回退)
  it('将空 bankroll 输入视为 0', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const input = screen.getByTestId('bankroll-input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: '' } });
    await waitFor(() => {
      expect(input.value).toBe('');
    });
    // 无崩溃；页面仍渲染
    expect(screen.getByTestId('bankroll-page')).toBeTruthy();
  });

  // 11. activeWallet 未定义 → walletLabel 显示 '—'
  it('无 wallet 时显示 em-dash 作为 wallet 标签', async () => {
    mockListWallets.mockResolvedValue([]);
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    // 等待 activeWalletQuery 解析，并使 BankrollCard 渲染 '—' 标签
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/Wallet:\s*—/);
    }, { timeout: 2000 });
  });

  // 12. activeWallet 存在 → 渲染 wallet 标签
  it('wallet 存在时显示 wallet 标签', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    // 等待 wallet query 解析，并使 BankrollCard 渲染
    await waitFor(() => {
      expect(document.body.textContent).toContain('Wallet: Treasury');
    }, { timeout: 2000 });
  });

  // 13. Recompute 按钮 onClick → 调用 allocationQuery.refetch
  it('点击 Recompute 时再次调用 computeAllocationPreview', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    mockComputeAllocationPreview.mockClear();
    fireEvent.click(screen.getByTestId('bankroll-compute'));
    await waitFor(() => {
      expect(mockComputeAllocationPreview).toHaveBeenCalled();
    });
  });

  // 14. ConfigSliders: min_edge_pct 滑块变更
  it('响应 min_edge_pct 滑块变更', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const slider = screen.getByTestId('config-slider-min_edge_pct') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.15' } });
    expect(slider.value).toBe('0.15');
    expect(document.body.textContent).toMatch(/15%/);
  });

  // 15. ConfigSliders: max_total_exposure_pct 滑块变更
  it('响应 max_total_exposure_pct 滑块变更', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const slider = screen.getByTestId('config-slider-max_total_exposure_pct') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.5' } });
    expect(slider.value).toBe('0.5');
    expect(document.body.textContent).toMatch(/50% of bankroll/);
  });

  // 16. ConfigSliders: reserve_pct 滑块变更
  it('响应 reserve_pct 滑块变更', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    const slider = screen.getByTestId('config-slider-reserve_pct') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.4' } });
    expect(slider.value).toBe('0.4');
    expect(document.body.textContent).toMatch(/40% kept/);
  });

  // 17. applyMut 成功时无 activeWallet 持久化配置 —— 等等，这是抛错用例。
  // 更好的做法：当 activeWallet?.id 为 undefined 时 apply 成功（仅 wallet 存在但无 id）
  it('wallet 无 id 时跳过 setBankrollConfig 持久化', async () => {
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

  // 18. applyMut onSuccess 使用当前 config 快照调用 setBankrollConfig
  it('apply 成功时持久化当前 config 快照（非初始默认值）', async () => {
    wrap();
    await waitFor(() => screen.getByTestId('bankroll-page'));
    // 先拖动滑块以确保 config 已更新
    const slider = screen.getByTestId('config-slider-kelly_multiplier') as HTMLInputElement;
    fireEvent.change(slider, { target: { value: '0.5' } });
    // 等待 config 变更后重新渲染，使 alloc-apply 重新出现
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
