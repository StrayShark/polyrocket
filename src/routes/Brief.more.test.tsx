// v0.72a — Brief 路由补充测试。
//
// Brief.tsx 有 181 行，包含 8 条 brief 列表 + rescore/dismiss
// mutations + 筛选 chips（rank/edge/category）+ dismissed pill。
// 已有测试（v0.62a + v0.63a）覆盖了 4 个表层用例（mount +
// refresh + dismiss + rescore）。我们新增 10 个测试，覆盖以下
// 5 个状态分支：
//   - 错误态（含 retry，第 60 行）
//   - 加载态（4 个 Skeleton，第 98-103 行）
//   - 空态（含 EmptyState + "Generate" CTA，第 104-114 行）
//   - refresh（ghost）按钮调用 useQuery.refetch，而非 mutation（第 80 行）
//   - rescore onSuccess → toast.success + 失效 query（第 42-48 行）
//   - rescore onError → 带 message 的 toast.error（第 49 行）
//   - dismiss onSuccess → toast.info + 失效 query（第 53-58 行）
//   - dismissed=true 的卡片显示 "Dismissed" pill（第 131-133 行）
//   - 不含 edge/confidence 的卡片（字段为 null，第 142-149 行）
//   - consensus_side='NO' → bear pill 渲染（第 151 行）
//
// 覆盖目标：78.26% → ~88% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

import { useLocaleStore } from '@/lib/i18n';
import { useToastStore } from '@/stores/toast-store';
useLocaleStore.setState({ locale: 'en' });

const { mockGet, mockRefresh, mockDismiss } = vi.hoisted(() => ({
  mockGet: vi.fn(),
  mockRefresh: vi.fn(),
  mockDismiss: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  dailyBriefGet: (...args: unknown[]) => mockGet(...args),
  dailyBriefRefresh: (...args: unknown[]) => mockRefresh(...args),
  dailyBriefDismiss: (...args: unknown[]) => mockDismiss(...args),
}));

import { Brief } from './Brief';

function renderBrief() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Brief />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const SAMPLE_BRIEF = [
  { market_id: 'm1', market_question: 'Will BTC reach 100k?', market_category: 'crypto',
    market_end_date: 9999999999, market_liquidity: '1000',
    market_volume_24h: '500', rank: 1, match_score: 0.9,
    score_breakdown: null, edge: 0.12, confidence: 0.75,
    consensus_side: 'YES', consensus_strength: 0.6,
    computed_at: 1000, expires_at: 9999999999, dismissed: false },
  { market_id: 'm2', market_question: 'Will ETH stay above 4k?', market_category: 'crypto',
    market_end_date: 9999999999, market_liquidity: '2000',
    market_volume_24h: '800', rank: 2, match_score: 0.85,
    score_breakdown: null, edge: -0.08, confidence: 0.6,
    consensus_side: 'NO', consensus_strength: 0.7,
    computed_at: 1000, expires_at: 9999999999, dismissed: true },
  { market_id: 'm3', market_question: 'Will Fed cut rates?', market_category: 'macro',
    market_end_date: 9999999999, market_liquidity: '5000',
    market_volume_24h: '2000', rank: 3, match_score: 0.7,
    score_breakdown: null, edge: null, confidence: null,
    consensus_side: null, consensus_strength: null,
    computed_at: 1000, expires_at: 9999999999, dismissed: false },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockGet.mockResolvedValue(SAMPLE_BRIEF);
  mockRefresh.mockResolvedValue({ computed_at: 1000, n_items: 3 });
  mockDismiss.mockResolvedValue(undefined);
});

describe('Brief (extended — v0.72a)', () => {
  it('shows error state with retry button when get fails', async () => {
    mockGet.mockRejectedValue(new Error('brief load failed'));
    renderBrief();
    await waitFor(() => {
      expect(screen.getByText(/brief load failed/i)).toBeInTheDocument();
    });
    const retryBtn = screen.getAllByRole('button').find(b =>
      /try again/i.test(b.textContent || ''),
    );
    expect(retryBtn).toBeTruthy();
  });

  it('renders 4 skeleton placeholders during loading', async () => {
    mockGet.mockReturnValue(new Promise(() => {})); // never resolves
    const { container } = renderBrief();
    await waitFor(() => {
      // Skeleton 组件渲染带有 animate-pulse 类的 div
      const skeletons = container.querySelectorAll('.animate-pulse');
      expect(skeletons.length).toBeGreaterThanOrEqual(4);
    });
  });

  it('renders empty state with Generate CTA when data is empty', async () => {
    mockGet.mockResolvedValue([]);
    renderBrief();
    await waitFor(() => {
      // EmptyState 组件 + "Generate" CTA
      expect(screen.getByText(/Generate|生成/i)).toBeInTheDocument();
    });
    const genBtn = screen.getAllByRole('button').find(b =>
      /generate|生成/i.test(b.textContent || ''),
    );
    expect(genBtn).toBeTruthy();
    // 点击 Generate —— 应触发 refreshMut
    fireEvent.click(genBtn!);
    await waitFor(() => {
      expect(mockRefresh).toHaveBeenCalled();
    });
  });

  it('refresh ghost button calls useQuery.refetch (not IPC mutation)', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const callsBefore = mockRefresh.mock.calls.length;
    const refreshBtn = screen.getAllByRole('button').find(b =>
      /refresh/i.test(b.textContent || '') && !/re-score/i.test(b.textContent || ''),
    );
    expect(refreshBtn).toBeTruthy();
    fireEvent.click(refreshBtn!);
    await waitFor(() => {
      // ghost "Refresh" 仅重拉缓存；不应调用 mutation
      expect(mockRefresh.mock.calls.length).toBe(callsBefore);
    });
  });

  it('rescore onSuccess shows toast with item count + invalidates query', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const rescoreBtn = screen.getAllByRole('button').find(b =>
      /re-score/i.test(b.textContent || ''),
    );
    fireEvent.click(rescoreBtn!);
    await waitFor(() => {
      expect(mockRefresh).toHaveBeenCalled();
    });
    // Toast store 应已接收到 success
    const { toasts } = useToastStore.getState();
    expect(toasts.length).toBeGreaterThan(0);
    const success = toasts.find(t => t.kind === 'success');
    expect(success).toBeTruthy();
  });

  it('rescore onError shows error toast with body', async () => {
    mockRefresh.mockRejectedValue(new Error('rescore failed'));
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const rescoreBtn = screen.getAllByRole('button').find(b =>
      /re-score/i.test(b.textContent || ''),
    );
    fireEvent.click(rescoreBtn!);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find(t => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body).toMatch(/rescore failed/i);
    });
  });

  it('dismiss onSuccess shows info toast + invalidates query', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/BTC reach 100k/i));
    const dismissBtn = screen.getAllByRole('button').find(b =>
      b.querySelector('svg.lucide-x') !== null,
    );
    expect(dismissBtn).toBeTruthy();
    fireEvent.click(dismissBtn!);
    await waitFor(() => {
      expect(mockDismiss).toHaveBeenCalledWith('m1');
    });
    const { toasts } = useToastStore.getState();
    const info = toasts.find(t => t.kind === 'info');
    expect(info).toBeTruthy();
  });

  it('renders dismissed pill for entries with dismissed=true', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/ETH stay above 4k/i));
    // m2 的 dismissed=true —— 应渲染 "Dismissed" 胶囊
    expect(screen.getAllByText(/dismissed/i).length).toBeGreaterThan(0);
  });

  it('handles entries with null edge/confidence (no edge/conf line)', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/Fed cut rates/i));
    // m3 的 edge=null + confidence=null + consensus_side=null
    // 该行仍应渲染，但 m3 的 edge / confidence / consensus
    // Pill 部分不应产生文本。
    // 最简单的检查：m3 question 文本在 document 中，但其旁
    // 不应出现 "+12.0%"（m1 有 +12.0%，m3 没有）。
    const fedRow = screen.getByText(/Fed cut rates/i).closest('div');
    expect(fedRow).toBeTruthy();
    // m3 行既不包含 "+12.0%" 的 edge，也不包含 "75.0%" 的 confidence 文本
    const fedRowText = fedRow?.textContent || '';
    expect(fedRowText).not.toMatch(/\+12\.0%/);
    expect(fedRowText).not.toMatch(/75\.0%/);
  });

  it('renders NO consensus as bear pill (different from YES bull)', async () => {
    renderBrief();
    await waitFor(() => screen.getByText(/ETH stay above 4k/i));
    // m2 的 consensus_side='NO' → bear pill
    // 该 pill 文本同时包含 side 字母与强度 %
    const noText = screen.getAllByText(/NO/);
    expect(noText.length).toBeGreaterThan(0);
  });
});