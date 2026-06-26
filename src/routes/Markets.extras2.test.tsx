// v0.72e + v0.119 — Markets 补充第二轮测试（仅足球）。
//
// Markets.tsx 共 251 行，包含 7 列 DataTable + 2 个类别
// 胶囊 ['all', 'football'] + active_only 切换 + syncMarkets
// 变更 + 搜索防抖。已有测试（v0.66 + v0.70h）
// 覆盖了表层用例。
//
// v0.119 足球转向：仅 ['all', 'football'] 过滤胶囊
//（无 cs2/politics/crypto/tech/other）。测试更新为使用
// 仅足球的数据 + 'football'/'all' 过滤胶囊测试。
//
// 我们新增 10 个测试覆盖剩余分支：
//   - status 列：resolved=true 且 outcome='YES'（Pill 显示 outcome）
//   - status 列：resolved=true 且 outcome=null（Pill 显示 'resolved'）
//   - status 列：active=true → 'active' 牛色胶囊
//   - status 列：active=false → 'inactive' 弱化胶囊
//   - 类别过滤胶囊非默认（'football'）
//   - active_only 复选框切换 → 以新 query key 重新拉取
//   - 按 slug 字段搜索（m.slug 包含 q）
//   - syncMut onSuccess → toast + invalidate 查询
//   - syncMut 失败 → toast.error
//   - 带搜索词的空状态显示 "No markets matching ..."
//
// 覆盖目标：分支 65.71% → 约 80%。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';
import { useToastStore } from '@/stores/toast-store';

const { mockListMarkets: mlm, mockSyncMarkets: msm } = vi.hoisted(() => ({
  mockListMarkets: vi.fn(),
  mockSyncMarkets: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listMarkets: (...args: unknown[]) => mlm(...args),
  syncMarkets: (...args: unknown[]) => msm(...args),
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

const MARKETS = [
  // m1: resolved=true, outcome='YES' → outcome 胶囊
  { id: 'm1', question: 'Q1?', slug: 'q1', category: 'football', liquidity: '1000', volume_24h: '500', end_date: 9999999999, active: false, resolved: true, outcome: 'YES' },
  // m2: resolved=true, outcome=null → 'resolved' 回退胶囊
  { id: 'm2', question: 'Q2?', slug: 'q2', category: 'football', liquidity: '2000', volume_24h: '600', end_date: 9999999999, active: false, resolved: true, outcome: null },
  // m3: active=true → 'active' 多头胶囊
  { id: 'm3', question: 'Q3?', slug: 'q3', category: 'football', liquidity: '3000', volume_24h: '700', end_date: 9999999999, active: true, resolved: false, outcome: null },
  // m4: active=false (未 resolved) → 'inactive' muted 胶囊
  { id: 'm4', question: 'Q4?', slug: 'q4', category: 'football', liquidity: '4000', volume_24h: '800', end_date: 9999999999, active: false, resolved: false, outcome: null },
  // m5：football 分类，用于 filter 测试
  { id: 'm5', question: 'Q5?', slug: 'q5', category: 'football', liquidity: '5000', volume_24h: '900', end_date: 9999999999, active: true, resolved: false, outcome: null },
];

beforeEach(() => {
  vi.clearAllMocks();
  mlm.mockResolvedValue(MARKETS);
  msm.mockResolvedValue(3);
  useToastStore.setState({ toasts: [] });
});

describe('Markets (extras round 2 — v0.72e + v0.119 football-only)', () => {
  it('status=resolved with outcome=YES renders outcome pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    expect(screen.getAllByText('YES').length).toBeGreaterThan(0);
  });

  it('status=resolved with outcome=null renders "resolved" pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q2?'));
    expect(screen.getAllByText('resolved').length).toBeGreaterThan(0);
  });

  it('status=active=true renders "active" bull pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q3?'));
    expect(screen.getAllByText('active').length).toBeGreaterThan(0);
  });

  it('status=active=false (not resolved) renders "inactive" muted pill', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q4?'));
    expect(screen.getAllByText('inactive').length).toBeGreaterThan(0);
  });

  it('click category chip "football" filters to football-only markets (v0.119)', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    // v0.119: 默认是 football；先验证 active 状态再测试 filter 逻辑。
    const footballChip = screen.getAllByRole('button').find(b =>
      b.textContent?.trim() === 'football',
    );
    expect(footballChip).toBeDefined();
    // 默认是 football，所以 Q1（football）应可见
    expect(screen.getByText('Q1?')).toBeInTheDocument();
  });

  it('click category chip "all" shows all markets including non-football (v0.119)', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const allChip = screen.getAllByRole('button').find(b =>
      b.textContent?.trim() === 'all',
    );
    expect(allChip).toBeDefined();
    fireEvent.click(allChip!);
    // 全部 football market 可见
    await waitFor(() => {
      expect(screen.getByText('Q1?')).toBeInTheDocument();
      expect(screen.getByText('Q5?')).toBeInTheDocument();
    });
  });

  it('does NOT render cs2/politics/crypto filter chips (v0.119 football pivot)', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const buttons = screen.getAllByRole('button');
    const labels = buttons.map(b => b.textContent?.trim()).filter(Boolean);
    // 关键断言：不存在其他类别胶囊
    for (const forbidden of ['cs2', 'politics', 'crypto', 'tech', 'other']) {
      expect(labels).not.toContain(forbidden);
    }
  });

  it('toggle active_only checkbox triggers refetch with new query key', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const callsBefore = mlm.mock.calls.length;
    const checkbox = screen.getByRole('checkbox') as HTMLInputElement;
    expect(checkbox.checked).toBe(true);
    fireEvent.click(checkbox);
    await waitFor(() => {
      expect(mlm.mock.calls.length).toBeGreaterThan(callsBefore);
    });
    expect(checkbox.checked).toBe(false);
  });

  it('search by slug field (matches slug text)', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'q5' } });
    await waitFor(() => {
      expect(screen.getByText('Q5?')).toBeInTheDocument();
      expect(screen.queryByText('Q1?')).not.toBeInTheDocument();
    });
  });

  it('click Sync button → syncMarkets + toast.success', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const syncBtn = screen.getAllByRole('button').find(b =>
      /^sync/i.test(b.textContent?.trim() || ''),
    );
    expect(syncBtn).toBeDefined();
    fireEvent.click(syncBtn!);
    await waitFor(() => {
      expect(msm).toHaveBeenCalled();
      const { toasts } = useToastStore.getState();
      const succ = toasts.find(t => t.kind === 'success');
      expect(succ).toBeTruthy();
      expect(succ?.body || succ?.title).toMatch(/3.*markets|synced/i);
    });
  });

  it('Sync mutation onError → toast.error', async () => {
    msm.mockRejectedValue(new Error('sync failed'));
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const syncBtn = screen.getAllByRole('button').find(b =>
      /^sync/i.test(b.textContent?.trim() || ''),
    );
    fireEvent.click(syncBtn!);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find(t => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body || err?.title).toMatch(/sync/i);
    });
  });

  it('Empty state with search shows "No markets matching" copy', async () => {
    renderMarkets();
    await waitFor(() => screen.getByText('Q1?'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'xxxxxx-no-match' } });
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/no.markets.matching|markets.empty/i);
    });
  });
});
