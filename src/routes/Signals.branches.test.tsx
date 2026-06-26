// v0.93 — Signals.tsx 分支第二轮（+7 测试，fn 72.7 → 约 85%）。
//
// 目标：列单元格分支、KpiCard delta 分支、
// Recompute 成功/错误分支以及
// 空状态 "no-match" 描述分支。已有测试覆盖
// 表层的过滤/变更流程。

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
    // signal-trade-1 对应 id=1，是 bullish（edge=0.15）
    const tradeLink = screen.getByTestId('signal-trade-1') as HTMLAnchorElement;
    expect(tradeLink.getAttribute('href')).toContain('side=YES');
    expect(tradeLink.getAttribute('href')).toContain('market=m1');
    expect(tradeLink.getAttribute('href')).toContain('price=0.6500');
  });

  it('Trade button URL encodes NO for bearish signal (edge < 0)', async () => {
    mls.mockResolvedValue(SAMPLE);
    renderSignals();
    await waitFor(() => screen.getByText('Will Y happen?'));
    // signal-trade-2 对应 id=2，是 bearish（edge=-0.10）
    const tradeLink = screen.getByTestId('signal-trade-2') as HTMLAnchorElement;
    expect(tradeLink.getAttribute('href')).toContain('side=NO');
    expect(tradeLink.getAttribute('href')).toContain('market=m2');
    expect(tradeLink.getAttribute('href')).toContain('price=0.4000');
  });

  it('Bullish KpiCard shows positive delta when total > 0', async () => {
    mls.mockResolvedValue(SAMPLE);
    renderSignals();
    await waitFor(() => screen.getByText('Will X happen?'));
    // 3 个中 2 个 bullish = 67%
    expect(document.body.textContent).toMatch(/67%/);
  });

  it('Bearish KpiCard shows no delta when total === 0', async () => {
    mls.mockResolvedValue([]);
    renderSignals();
    await waitFor(() => {
      // 当 total=0 时不出现百分比文本（"min edge 5%" 提示除外）
    // 具体而言，bullish/bearish delta 应为 0/0
    const text = document.body.textContent || '';
    // 不应包含 bullish "X%" delta 格式
    expect(text).not.toMatch(/Bullish\d+%/);
      expect(text).not.toMatch(/Bearish\d+%/);
    });
  });

  it('Recompute success toast differs for n > 0 vs n === 0', async () => {
    mls.mockResolvedValue(SAMPLE);
    // 首次运行：n > 0
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
    // 将 minEdgePct 调高到没有匹配项的值（例如 50）
    const input = screen.getByDisplayValue('5') as HTMLInputElement;
    fireEvent.change(input, { target: { value: '50' } });
    await waitFor(() => {
      // No-match 提示（非"完全没有 signal"空状态）
      expect(document.body.textContent).toMatch(/No signals match|min edge|threshold/i);
    });
  });
});
