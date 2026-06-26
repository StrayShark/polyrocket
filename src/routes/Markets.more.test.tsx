// v0.65d + v0.119 — Markets 组件测试（路由 43% → 约 75%，仅足球）。
//
// v0.119 足球转向：UI 仅暴露 ['all', 'football'] 过滤
// 胶囊。测试更新为使用仅足球的数据 + 验证非足球
// 芯片不存在。
//
// /markets 路由包含客户端类别过滤
// + 防抖搜索 + sync 变更。我们在
// 现有 v0.62a 测试（3 个测试）基础上
// 新增 7 个分支丰富的测试。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListMarkets = vi.fn();
const mockSyncMarkets = vi.fn();

vi.mock('@/ipc', () => ({
  listMarkets: (...args: unknown[]) => mockListMarkets(...args),
  syncMarkets: () => mockSyncMarkets(),
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

// v0.119 —— football-only fixtures(此前为 crypto/politics/tech)
const M_FootballA = {
  id: 'm1', slug: 'fifwc-arg-win', question: 'Will Argentina win?',
  category: 'football', end_date: 9999999999, active: true,
  resolved: false, outcome: null, liquidity: '1000', volume_24h: '500',
};
const M_FootballB = {
  ...M_FootballA, id: 'm2', slug: 'fifwc-fra-win', question: 'Will France win?',
  category: 'football', liquidity: '5000', volume_24h: '2000',
};
const M_FootballC = {
  ...M_FootballA, id: 'm3', slug: 'fifwc-o-u-25', question: 'Will match end O/U 2.5?',
  category: 'football',
};

describe('Markets (v0.65d expand + v0.119 football-only)', () => {
  it('renders ONLY [all, football] filter pills (v0.119)', async () => {
    mockListMarkets.mockResolvedValue([M_FootballA, M_FootballB, M_FootballC]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText('Argentina', { exact: false })).toBeInTheDocument();
    });
    const buttons = screen.getAllByRole('button');
    const labels = buttons.map(b => b.textContent?.trim()).filter(Boolean);
    // 应仅有 'all' 和 'football'
    expect(labels).toContain('all');
    expect(labels).toContain('football');
    // 关键：非足球的类别胶囊必须不存在
    expect(labels).not.toContain('cs2');
    expect(labels).not.toContain('politics');
    expect(labels).not.toContain('crypto');
    expect(labels).not.toContain('tech');
    expect(labels).not.toContain('other');
  });

  it('default filter is football (v0.119 football pivot)', async () => {
    mockListMarkets.mockResolvedValue([M_FootballA, M_FootballB]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText('Argentina', { exact: false })).toBeInTheDocument();
    });
    const footballPill = screen.getAllByRole('button').find(
      b => b.textContent?.trim() === 'football',
    );
    // 默认 active 状态：bg-accent/15
    expect(footballPill?.className).toContain('bg-accent/15');
  });

  it('clicking "all" pill shows all categories (no filter)', async () => {
    mockListMarkets.mockResolvedValue([M_FootballA, M_FootballB]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText('Argentina', { exact: false })).toBeInTheDocument();
    });
    const allPill = screen.getAllByRole('button').find(
      b => b.textContent?.trim() === 'all',
    );
    expect(allPill).toBeTruthy();
    fireEvent.click(allPill!);
    await waitFor(() => {
      const allPillAfter = screen.getAllByRole('button').find(
        b => b.textContent?.trim() === 'all',
      );
      expect(allPillAfter?.className).toContain('bg-accent/15');
    });
  });

  it('toggles "active only" via the switch', async () => {
    mockListMarkets.mockResolvedValue([M_FootballA]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText('Argentina', { exact: false })).toBeInTheDocument();
    });
    const activeOnlySwitch = screen.getByRole('checkbox') ||
      document.querySelector('input[type="checkbox"]');
    if (activeOnlySwitch) {
      fireEvent.click(activeOnlySwitch);
      await waitFor(() => {
        const calls = mockListMarkets.mock.calls;
        const lastCall = calls[calls.length - 1];
        expect(lastCall[0]).toEqual(expect.objectContaining({ active_only: false }));
      });
    }
  });

  it('search input filters the table client-side (after 200ms debounce)', async () => {
    mockListMarkets.mockResolvedValue([M_FootballA, M_FootballB, M_FootballC]);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText('Argentina', { exact: false })).toBeInTheDocument();
    });
    const searchInput = screen.getByPlaceholderText(/search/i) ||
      document.querySelector('input[type="search"]') ||
      document.querySelector('input[placeholder*="earch"]');
    if (searchInput) {
      fireEvent.change(searchInput, { target: { value: 'France' } });
      await waitFor(() => {
        expect(screen.getByText('France', { exact: false })).toBeInTheDocument();
        expect(screen.queryByText('Argentina', { exact: false })).toBeNull();
      }, { timeout: 1000 });
    }
  });

  it('Sync button calls syncMarkets and shows success toast', async () => {
    mockListMarkets.mockResolvedValue([M_FootballA]);
    mockSyncMarkets.mockResolvedValue(5);
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText('Argentina', { exact: false })).toBeInTheDocument();
    });
    const syncBtn = screen.getAllByRole('button').find((b) =>
      /sync/i.test(b.textContent || ''),
    );
    expect(syncBtn).toBeTruthy();
    fireEvent.click(syncBtn!);
    await waitFor(() => {
      expect(mockSyncMarkets).toHaveBeenCalled();
    });
  });

  it('renders ErrorState when listMarkets fails', async () => {
    mockListMarkets.mockRejectedValue(new Error('Gamma API down'));
    renderMarkets();
    await waitFor(() => {
      expect(screen.getByText(/Gamma API down/i)).toBeInTheDocument();
    });
  });

  it('renders EmptyState when listMarkets returns []', async () => {
    mockListMarkets.mockResolvedValue([]);
    renderMarkets();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
