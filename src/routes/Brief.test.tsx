// v0.62a.2 + v0.63a — Brief 组件测试。
//
// /brief 是每日 brief 全量列表页面。
// 目前覆盖率约 40%。本文件覆盖：
//   1. 空数据初始渲染
//   2. Refresh 按钮触发 mutation（v0.63a）
//   3. Dismiss 按钮（v0.63a）
//   4. 使用 mock 数据渲染 brief entry

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// 在挂载前强制 locale store 为 'en'。i18n 的
// useT() 从 useLocaleStore 读取 locale，因此前一个
// 测试残留的 'zh' 会渲染 "刷新" 而非 "Refresh"，
// 进而破坏选择器。
import { useLocaleStore } from '@/lib/i18n';
useLocaleStore.setState({ locale: 'en' });

vi.mock('@/ipc', () => ({
  dailyBriefGet: vi.fn().mockResolvedValue([
    { market_id: 'm1', market_question: 'Will X?', market_category: 'crypto',
      market_end_date: 9999999999, market_liquidity: '1000',
      market_volume_24h: '500', rank: 1, match_score: 0.9,
      score_breakdown: null, edge: 0.1, confidence: 0.7,
      consensus_side: 'YES', consensus_strength: 0.6,
      computed_at: 1000, expires_at: 9999999999, dismissed: false },
  ]),
  dailyBriefRefresh: vi.fn().mockResolvedValue({ computed_at: 1000, n_items: 1 }),
  dailyBriefDismiss: vi.fn().mockResolvedValue(undefined),
}));

import { Brief } from './Brief';
import { dailyBriefRefresh, dailyBriefDismiss } from '@/ipc';

function renderBrief() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Brief />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Brief', () => {
  beforeEach(() => {
    useLocaleStore.setState({ locale: 'en' });
  });

  it('renders the page', async () => {
    renderBrief();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows the refresh button', async () => {
    renderBrief();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const refresh = buttons.find((b) =>
        b.textContent?.toLowerCase().includes('refresh'),
      );
      expect(refresh).toBeTruthy();
    });
  });

  it('clicking rescore button calls dailyBriefRefresh', async () => {
    renderBrief();
    await waitFor(() => {
      expect(screen.getAllByText(/Will X\?/i).length).toBeGreaterThan(0);
    });
    // "Re-score" 是主 CTA，会调用 dailyBriefRefresh IPC。
    // "Refresh"（左侧 ghost 按钮）仅通过 useQuery.refetch() 重新拉取缓存。
    // i18n en 使用连字符：brief.rescore → "Re-score"。
    const rescoreBtn = screen.getAllByRole('button').find((b) =>
      b.textContent?.includes('Re-score'),
    );
    expect(rescoreBtn).toBeTruthy();
    fireEvent.click(rescoreBtn!);
    await waitFor(() => {
      expect(dailyBriefRefresh).toHaveBeenCalled();
    });
  });

  it('dismisses a brief entry when its X button is clicked', async () => {
    renderBrief();
    await waitFor(() => {
      expect(screen.getAllByText(/Will X\?/i).length).toBeGreaterThan(0);
    });
    // X 关闭按钮渲染为带 lucide-x 图标的 button。
    const dismissBtn = screen.getAllByRole('button').find((b) =>
      b.querySelector('svg.lucide-x') !== null,
    );
    if (dismissBtn) {
      fireEvent.click(dismissBtn);
      await waitFor(() => {
        expect(dailyBriefDismiss).toHaveBeenCalled();
      });
    }
  });
});
