/**
 * PromoteHistory 组件测试(v0.19c)。
 *
 * v0.19c 在 ModelLab 页面新增只读 "Promotion history"
 * 面板。它调用 `list_promote_history` IPC,
 * 按时间倒序展示 entry(最新在前),并渲染
 * 带颜色提示(绿/黄/红)的 Brier 徽章。
 *
 * 面板有 4 种渲染状态:
 *  1. loading     → 3 个 skeleton 行
 *  2. 错误       → ErrorState
 *  3. empty       → "No promotions yet" 空状态
 *  4. populated   → N 行,最新在前
 *
 * 此处测试 mock `@/ipc`(经 vi.mock)和 react-query
 * (经 QueryClient wrapper)。模式与现有
 * AnalyzeProgress.test.tsx + TrainProgress.test.tsx 保持一致。
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
import { PromoteHistory } from '@/components/feedback/PromoteHistory';

// 极简的 i18n 替身 —— 真实的 useT 来自 Zustand
// store。测试只验证字符串能渲染;key 本身
// 在 i18n.test.ts 中单独验证。
vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

const mockListPromoteHistory = vi.mocked(listPromoteHistory);

describe('PromoteHistory (v0.19c)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistory).mockReset();
  });

  it('renders the empty state when history is empty', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 0,
      entries: [],
      message: 'no active model yet; train + promote to start history',
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-loading')).toBeInTheDocument();
    });
    // 等待空状态替换 skeleton
    await waitFor(() => {
      expect(screen.getByText('promote.history.empty')).toBeInTheDocument();
    });
    expect(screen.getByText('promote.history.empty_desc')).toBeInTheDocument();
  });

  it('renders rows in reverse-chronological order (newest first)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        // 最旧在前(Python 的自然顺序)
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.16, best_params: null },
        { job_id: 'train-ccc', model_version: 'logistic-train-ccc', promoted_at_ms: 1_700_002_000_000, best_brier: 0.14, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const rows = screen.getAllByTestId('promote-history-row');
    expect(rows).toHaveLength(3);
    // 最新(ccc)应排第一,最旧(aaa)应排最后
    expect(rows[0]).toHaveAttribute('data-job-id', 'train-ccc');
    expect(rows[1]).toHaveAttribute('data-job-id', 'train-bbb');
    expect(rows[2]).toHaveAttribute('data-job-id', 'train-aaa');
  });

  it('renders the Brier score with proper formatting', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.184, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-brier')).toBeInTheDocument();
    });
    const brier = screen.getByTestId('promote-history-brier');
    expect(brier).toHaveAttribute('data-brier', '0.184');
    expect(brier.textContent).toBe('0.184');
  });

  it('shows em-dash for null Brier (v0.18 active.json back-compat)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: null, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-brier')).toBeInTheDocument();
    });
    const brier = screen.getByTestId('promote-history-brier');
    expect(brier).toHaveAttribute('data-brier', '');
    expect(brier.textContent).toBe('—');
  });

  it('exposes the count via data-count attribute', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 5,
      entries: Array.from({ length: 5 }, (_, i) => ({
        job_id: `train-${i}`,
        model_version: `logistic-train-${i}`,
        promoted_at_ms: 1_700_000_000_000 + i * 1000,
        best_brier: 0.15,
        best_params: null,
      })),
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history')).toHaveAttribute('data-count', '5');
    expect(screen.getAllByTestId('promote-history-row')).toHaveLength(5);
  });

  it('hides the Rollback button on the active row (v0.20c)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.16, best_params: null },
      ],
      message: null,
    });
    // active model 是 train-bbb(较新的那一个,
    // 但我们按最新在前展示,所以它是第一行)。
    render(wrap(<PromoteHistory activeModelVersion="logistic-train-bbb" />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    // active 行被打标
    const rows = screen.getAllByTestId('promote-history-row');
    expect(rows[0]).toHaveAttribute('data-active', 'true');
    expect(rows[1]).toHaveAttribute('data-active', 'false');
    // 只有非 active 行有 Rollback 按钮
    const rollbackButtons = screen.getAllByTestId('promote-history-rollback');
    expect(rollbackButtons).toHaveLength(1);
    // active 徽章位于 active 行
    expect(screen.getByTestId('promote-history-active-badge')).toBeInTheDocument();
  });

  it('shows Rollback button on all rows when no active version is set', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb', promoted_at_ms: 1_700_001_000_000, best_brier: 0.16, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory activeModelVersion={null} />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const rollbackButtons = screen.getAllByTestId('promote-history-rollback');
    expect(rollbackButtons).toHaveLength(2);
    // 无 active 徽章
    expect(screen.queryByTestId('promote-history-active-badge')).toBeNull();
  });

  it('opens confirmation modal when Rollback is clicked (v0.20c)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-rollback')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('promote-history-rollback'));
    // 确认 modal 打开
    await waitFor(() => {
      expect(screen.getByTestId('rollback-confirm-btn')).toBeInTheDocument();
    });
    expect(screen.getByText('rollback.confirm.title')).toBeInTheDocument();
  });

  it('shows "best trial" badge when trial_index is null (v0.24a)', async () => {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-aaa', model_version: 'logistic-train-aaa', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null, trial_index: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const badge = screen.getByTestId('promote-history-trial-badge');
    expect(badge).toHaveAttribute('data-trial-index', 'best');
    expect(badge.textContent).toContain('promote.history.trial_best');
  });

  it('shows "best trial" badge when trial_index is missing (v0.18 back-compat, v0.24a)', async () => {
    // v0.21a 之前的旧 history entry 完全没有
    // trial_index。L1 类型将其标为 optional,
    // 因此 undefined 应当与 null 等同(best)。
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-old', model_version: 'logistic-train-old', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const badge = screen.getByTestId('promote-history-trial-badge');
    expect(badge).toHaveAttribute('data-trial-index', 'best');
  });

  it('shows "trial #N" badge for bulk-promoted trials (v0.24a)', async () => {
    // trial_index=2 → 0-based 2,展示为 "trial #3"
    // (人为展示采用 1-based)
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 1,
      entries: [
        { job_id: 'train-bbb', model_version: 'logistic-train-bbb-t2', promoted_at_ms: 1_700_000_000_000, best_brier: 0.18, best_params: null, trial_index: 2 },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const badge = screen.getByTestId('promote-history-trial-badge');
    expect(badge).toHaveAttribute('data-trial-index', '2');
    expect(badge.textContent).toContain('promote.history.trial_n');
  });
});

// =================================================================
// ================== v0.30a — trial 类型筛选 =====================
// =================================================================

describe('PromoteHistory trial-type filter (v0.30a)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistory).mockReset();
  });

  // 辅助函数:3 个 entry,best + bulk 混合。
  //  - train-a: 最佳 trial（trial_index 为 null）     — Brier 0.20
  //  - train-b: 批量 trial 0（trial_index 为 0）      — Brier 0.18
  //  - train-c: 批量 trial 1（trial_index 为 1）      — Brier 0.16
  function mockMixed() {
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 3,
      entries: [
        { job_id: 'train-a', model_version: 'logistic-train-a', promoted_at_ms: 1_700_000_000_000, best_brier: 0.20, best_params: null, trial_index: null },
        { job_id: 'train-b', model_version: 'logistic-train-b-t0', promoted_at_ms: 1_700_001_000_000, best_brier: 0.18, best_params: null, trial_index: 0 },
        { job_id: 'train-c', model_version: 'logistic-train-c-t1', promoted_at_ms: 1_700_002_000_000, best_brier: 0.16, best_params: null, trial_index: 1 },
      ],
      message: null,
    });
  }

  it('renders the filter chips with "all" as default', async () => {
    mockMixed();
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-filter')).toBeInTheDocument();
    });
    expect(screen.getByTestId('promote-history-filter-all')).toBeInTheDocument();
    expect(screen.getByTestId('promote-history-filter-best')).toBeInTheDocument();
    expect(screen.getByTestId('promote-history-filter-bulk')).toBeInTheDocument();
    expect(screen.getByTestId('promote-history-filter')).toHaveAttribute('data-active', 'all');
  });

  it('shows all 3 entries by default (filter=all)', async () => {
    mockMixed();
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history')).toBeInTheDocument();
    });
    const rows = screen.getAllByTestId('promote-history-row');
    expect(rows).toHaveLength(3);
  });

  it('filter=best shows only best-trial entries (v0.30a)', async () => {
    mockMixed();
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-filter-best')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('promote-history-filter-best'));
    await waitFor(() => {
      const rows = screen.getAllByTestId('promote-history-row');
      expect(rows).toHaveLength(1);
    });
    // 剩下的行是 best-trial 那一行
    const row = screen.getByTestId('promote-history-row');
    expect(row).toHaveAttribute('data-job-id', 'train-a');
    expect(screen.getByTestId('promote-history-filter')).toHaveAttribute('data-active', 'best');
  });

  it('filter=bulk shows only bulk-trial entries (v0.30a)', async () => {
    mockMixed();
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-filter-bulk')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('promote-history-filter-bulk'));
    await waitFor(() => {
      const rows = screen.getAllByTestId('promote-history-row');
      expect(rows).toHaveLength(2);
    });
    const rows = screen.getAllByTestId('promote-history-row');
    expect(rows[0]).toHaveAttribute('data-job-id', 'train-c'); // 最新在前
    expect(rows[1]).toHaveAttribute('data-job-id', 'train-b');
  });

  it('shows "no entries match" message when filter has 0 results (v0.30a)', async () => {
    // 只有 bulk entry,然后过滤到 "best" → 0 结果
    vi.mocked(listPromoteHistory).mockResolvedValue({
      ok: true,
      count: 2,
      entries: [
        { job_id: 'a', model_version: 'a-t0', promoted_at_ms: 1, best_brier: 0.2, best_params: null, trial_index: 0 },
        { job_id: 'b', model_version: 'b-t1', promoted_at_ms: 2, best_brier: 0.18, best_params: null, trial_index: 1 },
      ],
      message: null,
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-filter-best')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('promote-history-filter-best'));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-filtered-empty')).toBeInTheDocument();
    });
    expect(screen.queryAllByTestId('promote-history-row')).toHaveLength(0);
  });

  it('renders reason icon with the per-entry reason (v0.41a)', async () => {
    mockListPromoteHistory.mockResolvedValue({
      ok: true,
      message: 'ok',
      count: 1,
      entries: [
        {
          job_id: 'train-bbb',
          model_version: 'logistic-train-bbb',
          promoted_at_ms: 1_700_000_000_000,
          best_brier: 0.17,
          best_params: null,
          trial_index: null,
          reason: 'Promoted as best trial',
        },
      ],
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-row')).toBeInTheDocument();
    });
    const icon = screen.getByTestId('promote-history-reason-icon');
    expect(icon).toBeInTheDocument();
    expect(icon.getAttribute('data-reason')).toBe('Promoted as best trial');
    expect(icon.getAttribute('title')).toBe('Promoted as best trial');
  });

  it('falls back to generic "Promoted" tooltip when reason is missing (v0.41a back-compat)', async () => {
    mockListPromoteHistory.mockResolvedValue({
      ok: true,
      message: 'ok',
      count: 1,
      entries: [
        {
          job_id: 'train-ccc',
          model_version: 'logistic-train-ccc',
          promoted_at_ms: 1_700_000_000_000,
          best_brier: 0.19,
          best_params: null,
          trial_index: 1,
          // reason 缺失 —— v0.41 之前的 archive
        },
      ],
    });
    render(wrap(<PromoteHistory />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-row')).toBeInTheDocument();
    });
    const icon = screen.getByTestId('promote-history-reason-icon');
    expect(icon).toBeInTheDocument();
    expect(icon.getAttribute('data-reason')).toBe('');
    expect(icon.getAttribute('title')).toBe('promote.history.reason_fallback');
  });
});
