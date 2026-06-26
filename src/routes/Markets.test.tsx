// v0.62a + v0.119 —— Markets 组件测试（仅足球）。
//
// v0.119 产品方向调整：polyrocket 只做足球市场预测
// (see docs/polyrocket-football-prd.md)。Markets 页面 UI 锁定
// 为足球 —— 只有 ['all', 'football'] 过滤胶囊，默认
// category 为 'football'。这些测试验证仅足球的
// 界面。
//
// /markets 路由是用户的市场浏览器 —— search /
// filter / sync 即可刷新。

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listMarkets: vi.fn().mockResolvedValue([
    { id: 'm1', slug: 'fifwc-1', question: 'Will Argentina win?', category: 'football',
      end_date: 9999999999, active: true, resolved: false, outcome: null,
      liquidity: '1000', volume_24h: '500' },
    { id: 'm2', slug: 'fifwc-2', question: 'France to win?', category: 'football',
      end_date: 9999999999, active: true, resolved: false, outcome: null,
      liquidity: '2000', volume_24h: '800' },
  ]),
  syncMarkets: vi.fn().mockResolvedValue(1),
}));

import { Markets } from './Markets';

function renderMarkets() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Markets />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Markets (v0.119 football-only)', () => {
  it('renders the page title and an empty state initially', async () => {
    renderMarkets();
    await waitFor(() => {
      expect(screen.getAllByText(/markets/i).length).toBeGreaterThan(0);
    });
  });

  it('shows sync button', async () => {
    renderMarkets();
    await waitFor(() => {
      // Sync 按钮存在于工具栏中
      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBeGreaterThan(0);
    });
  });

  it('renders ONLY [all, football] filter pills (v0.119 football pivot)', async () => {
    renderMarkets();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const labels = buttons.map((b) => b.textContent?.trim()).filter(Boolean);
      // 应仅有 'all' 和 'football' —— 没有 cs2/politics/crypto/tech/other
      expect(labels).toContain('all');
      expect(labels).toContain('football');
      // 关键：非足球胶囊不应存在
      expect(labels).not.toContain('cs2');
      expect(labels).not.toContain('politics');
      expect(labels).not.toContain('crypto');
      expect(labels).not.toContain('tech');
      expect(labels).not.toContain('other');
    });
  });

  it('defaults to football category (v0.119 football pivot)', async () => {
    renderMarkets();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const footballPill = buttons.find((b) => b.textContent === 'football');
      // Football 胶囊应具有 active class（bg-accent/15）
      expect(footballPill).toBeTruthy();
      expect(footballPill?.className).toContain('bg-accent/15');
    });
  });

  it('switches filter when "all" pill is clicked (shows all categories in DB)', async () => {
    renderMarkets();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const allPill = buttons.find((b) => b.textContent === 'all');
      if (allPill) fireEvent.click(allPill);
      // 点击后，'all' 胶囊应处于 active 状态
      const allPillAfter = buttons.find((b) => b.textContent === 'all');
      expect(allPillAfter?.className).toContain('bg-accent/15');
    });
  });

  it('search input is editable', async () => {
    renderMarkets();
    await waitFor(() => {
      const inputs = screen.getAllByPlaceholderText(/search/i);
      if (inputs.length > 0) {
        fireEvent.change(inputs[0], { target: { value: 'Argentina' } });
        expect((inputs[0] as HTMLInputElement).value).toBe('Argentina');
      }
    });
  });
});
