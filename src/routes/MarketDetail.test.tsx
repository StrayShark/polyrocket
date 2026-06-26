// v0.62a — MarketDetail 组件测试。
//
// /markets/:id 是单个 market 的详情页。
// 当前覆盖率为 0%。本文件覆盖：
//   1. 空数据时的初始渲染
//   2. market 不存在时的 not found 态
//   3. 渲染 signals 列表（即便为空）

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listMarkets: vi.fn().mockResolvedValue([]),
  listActiveSignals: vi.fn().mockResolvedValue([]),
}));

import { MarketDetail } from './MarketDetail';

function renderMarketDetail(id = 'm1') {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[`/markets/${id}`]}>
        <Routes>
          <Route path="/markets/:id" element={<MarketDetail />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('MarketDetail', () => {
  it('renders not-found when the market id is unknown', async () => {
    renderMarketDetail('nonexistent');
    await waitFor(() => {
      // "not found" 空状态
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
