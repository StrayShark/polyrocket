/**
 * PromoteHistoryChart 组件测试(v0.22a)。
 *
 * 图表是自包含的 SVG sparkline。复用
 * `useQuery(['promote-history'], listPromoteHistory)`
 * 模式(与 PromoteHistory 相同),但渲染方式
 * 不同(SVG 点 + 连线,无每行详情)。
 *
 * 图表有 4 种渲染状态:
 *  1. 加载中     → skeleton
 *  2. 错误       → ErrorState
 *  3. empty       → "No brier data yet" 占位
 *  4. populated   → 带点和连线的 SVG
 *
 * 此处测试 mock `@/ipc` 和 react-query(经 QueryClient
 * wrapper)。模式与 PromoteHistory.test.tsx 一致。
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listPromoteHistory: vi.fn(),
  rollbackModel: vi.fn(),
}));

import { listPromoteHistory } from '@/ipc';
import { PromoteHistoryChart } from '@/components/feedback/PromoteHistoryChart';

// 极简 i18n shim —— 真实的 useT 从 Zustand store 中拉取。
vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

describe('PromoteHistoryChart (v0.22a)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistory).mockReset();
  });

  it('renders the empty state when no entries have brier data', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 0,
      entries: [],
      message: 'no active model yet',
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-empty')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-empty').textContent).toContain(
      'promote.chart.empty'
    );
  });

  it('skips entries with null brier (v0.18 back-compat)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        // 第一条: null brier(应被跳过)
        { job_id: 'train-old', model_version: 'logistic-train-old', promoted_at_ms: 1_700_000_000_000, best_brier: null, best_params: null },
        // 第二条: 真实 brier
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    // 仅 1 个点(null-brier 的 entry 已被过滤)
    const dots = screen.getAllByTestId('promote-history-chart-dot');
    expect(dots).toHaveLength(1);
    expect(dots[0]).toHaveAttribute('data-job-id', 'train-aaa');
  });

  it('renders one dot per entry with brier data', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.20, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-ccc', model_version: 'logistic-train-ccc', promoted_at_ms: 1_700_002_000_000, best_brier: 0.14, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    const dots = screen.getAllByTestId('promote-history-chart-dot');
    expect(dots).toHaveLength(3);
    expect(dots[0]).toHaveAttribute('data-brier', '0.2');
    expect(dots[1]).toHaveAttribute('data-brier', '0.18');
    expect(dots[2]).toHaveAttribute('data-brier', '0.14');
  });

  it('shows "improving" trend when latest brier is lower than first', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1_700_000_000_000, best_brier: 0.20, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'c', model_version: 'c', promoted_at_ms: 1_700_002_000_000, best_brier: 0.14, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-trend')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-trend')).toHaveAttribute('data-trend', 'up');
  });

  it('shows "worsening" trend when latest brier is higher than first', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1_700_000_000_000, best_brier: 0.14, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'c', model_version: 'c', promoted_at_ms: 1_700_002_000_000, best_brier: 0.22, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-trend')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-trend')).toHaveAttribute('data-trend', 'down');
  });

  it('shows "flat" trend when all briers are equal', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'c', model_version: 'c', promoted_at_ms: 1_700_002_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart-trend')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart-trend')).toHaveAttribute('data-trend', 'flat');
  });

  it('exposes the count via data-points attribute', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 5,
      entries: Array.from({ length: 5 }, (_, i) => ({
        job_id: `train-${i}`,
        model_version: `logistic-train-${i}`,
        promoted_at_ms: 1_700_000_000_000 + i * 1000,
        best_brier: 0.15 + i * 0.01,
        best_params: null,
      })),
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-chart')).toHaveAttribute('data-points', '5');
  });
});

// =================================================================
// ==================== v0.29a — 悬停提示 =====================
// =================================================================

describe('PromoteHistoryChart hover tooltips (v0.29a)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistory).mockReset();
  });

  it('renders a <title> child for each dot (native browser tooltip)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.20, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    const { container } = render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    // 每个 <circle> 点都有一个 <title> 子元素
    const titles = container.querySelectorAll('circle[data-testid="promote-history-chart-dot"] > title');
    expect(titles).toHaveLength(2);
    // 第一个点的 title 包含其 model version 和 brier
    expect(titles[0].textContent).toMatch(/logistic-train-aaa/);
    expect(titles[0].textContent).toMatch(/Brier 0\.200/);
  });

  it('renders invisible hit-area circles for easier hovering', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1, best_brier: 0.2, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 2, best_brier: 0.18, best_params: null },
        { job_id: 'c', model_version: 'c', promoted_at_ms: 3, best_brier: 0.16, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    const hits = screen.getAllByTestId('promote-history-chart-hit');
    expect(hits).toHaveLength(3);
    // 每个 hit 有 data-hit-index 属性
    expect(hits[0]).toHaveAttribute('data-hit-index', '0');
    expect(hits[1]).toHaveAttribute('data-hit-index', '1');
    expect(hits[2]).toHaveAttribute('data-hit-index', '2');
  });

  it('shows the custom tooltip on hover and hides on leave', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.20, best_params: null, trial_index: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null, trial_index: 2 },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });

    // hover 前不显示 tooltip
    expect(screen.queryByTestId('promote-history-chart-tooltip')).toBeNull();

    // hover 第二个 hit 区域
    const hits = screen.getAllByTestId('promote-history-chart-hit');
    fireEvent.mouseEnter(hits[1]);

    // tooltip 现在可见,展示被 hover 的 entry 的数据
    const tooltip = screen.getByTestId('promote-history-chart-tooltip');
    expect(tooltip).toBeInTheDocument();
    expect(tooltip).toHaveAttribute('data-job-id', 'train-bbb');
    expect(tooltip).toHaveAttribute('data-brier', '0.18');
    expect(tooltip).toHaveAttribute('data-trial-index', '2');

    // 在 SVG 上 mouse leave 隐藏 tooltip
    const svg = screen.getByTestId('promote-history-chart-svg');
    fireEvent.mouseLeave(svg);
    expect(screen.queryByTestId('promote-history-chart-tooltip')).toBeNull();
  });

  it('grows the hovered dot and adds a stroke ring', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'a', model_version: 'a', promoted_at_ms: 1, best_brier: 0.2, best_params: null },
        { job_id: 'b', model_version: 'b', promoted_at_ms: 2, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistoryChart />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-chart')).toBeInTheDocument();
    });
    const hits = screen.getAllByTestId('promote-history-chart-hit');
    fireEvent.mouseEnter(hits[0]);
    const dots = screen.getAllByTestId('promote-history-chart-dot');
    // hover 的点更大
    expect(dots[0]).toHaveAttribute('r', '3.5');
    expect(dots[1]).toHaveAttribute('r', '2.5');
    // hover 的点带 stroke
    expect(dots[0]).toHaveAttribute('stroke-width', '1');
    expect(dots[1]).toHaveAttribute('stroke-width', '0');
  });
});
