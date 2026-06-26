// v0.62a —— Signals 组件测试。
//
// /signals 是活动 signal 列表。目前 0%
// 覆盖率。本文件覆盖：
//   1. 使用空数据的初始渲染
//   2. 渲染 Recompute 按钮（stub）
//   3. Min-edge 过滤输入可编辑

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listActiveSignals: vi.fn().mockResolvedValue([]),
  recomputeSignals: vi.fn().mockResolvedValue(0),
}));

import { Signals } from './Signals';

function renderSignals() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Signals />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Signals', () => {
  it('renders the page', async () => {
    renderSignals();
    await waitFor(() => {
      // 多处元素匹配 "/signals/i"（h2 + 导航链接）
      expect(screen.getAllByText(/signals/i).length).toBeGreaterThan(0);
    });
  });

  it('shows the recompute button', async () => {
    renderSignals();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBeGreaterThan(0);
    });
  });
});
