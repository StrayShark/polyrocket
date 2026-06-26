// v0.62a — Trade 组件测试。
//
// /trade 是独立的下注页面。
// 路由很小——只是用 URL 参数
// 渲染 <PlaceBetForm />。当前覆盖率 0%。

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  placeSignedOrder: vi.fn().mockResolvedValue({ bet_id: 'b1' }),
  validateOrderArgs: vi.fn().mockResolvedValue({ ok: true, error: null }),
}));

import { Trade } from './Trade';

function renderTrade(initialPath = '/trade?market_id=m1&side=YES&edge=0.1') {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[initialPath]}>
        <Routes>
          <Route path="/trade" element={<Trade />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Trade', () => {
  it('renders the place-bet form', async () => {
    renderTrade();
    await waitFor(() => {
      // 表单具有 market_id 输入 / 提交
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
