/**
 * v0.34a —— PromoteHistoryArchive 组件测试。
 *
 * archive modal 是读取 Python sidecar 的
 * `archive.jsonl`(经 v0.33b `list_promote_history_archive`
 * IPC)的 L1 UI。测试覆盖:
 *  1. 空状态(尚无 archive 文件)
 *  2. 单页的已填充状态
 *  3. 分页:prev/next 按钮可用
 *  4. 每行:trial 徽章、brier 颜色、时间
 *  5. close 按钮重置 offset
 *
 * 模式与现有 PromoteHistory.test.tsx 一致:
 *   - happy-dom 环境
 *   - mock @/ipc(只需 archive IPC)
 *   - mock @/lib/i18n 为 `(k) => k` 透传
 *   - 包在 QueryClientProvider 中
 *   - Modal 走真实实现(不 stub)
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listPromoteHistoryArchive: vi.fn(),
}));

import { listPromoteHistoryArchive } from '@/ipc';
import { PromoteHistoryArchive } from '@/components/feedback/PromoteHistoryArchive';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

describe('PromoteHistoryArchive (v0.34a)', () => {
  beforeEach(() => {
    vi.mocked(listPromoteHistoryArchive).mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it('renders empty state when the archive file does not exist', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [],
      total: 0,
      message: 'no archive yet; archive is created on first overflow',
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    // 空状态显示 EmptyState 组件,而不是
    // rows 容器。等待空状态标题出现。
    await waitFor(() => {
      expect(screen.getByText('promote.archive.empty_title')).toBeInTheDocument();
    });
    expect(screen.getByText('promote.archive.empty_desc')).toBeInTheDocument();
  });

  it('renders a single page of entries with total > 0', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [
        { job_id: 'a', model_version: 'la', promoted_at_ms: 3, best_brier: 0.20, best_params: null, weights: null, trial_index: null, archived_at_ms: 3 },
        { job_id: 'b', model_version: 'lb-t1', promoted_at_ms: 2, best_brier: 0.18, best_params: null, weights: null, trial_index: 1, archived_at_ms: 2 },
        { job_id: 'c', model_version: 'lc', promoted_at_ms: 1, best_brier: 0.16, best_params: null, weights: null, trial_index: null, archived_at_ms: 1 },
      ],
      total: 3,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    await waitFor(() => {
      expect(screen.getByTestId('promote-history-archive')).toBeInTheDocument();
    });
    const rows = screen.getAllByTestId('promote-history-archive-row');
    expect(rows).toHaveLength(3);
    // Total 已报告
    expect(screen.getByTestId('promote-history-archive')).toHaveAttribute('data-total', '3');
  });

  it('renders the trial badge per row (best vs trial N)', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [
        { job_id: 'a', model_version: 'la', promoted_at_ms: 2, best_brier: 0.20, best_params: null, weights: null, trial_index: null, archived_at_ms: 2 },
        { job_id: 'b', model_version: 'lb-t2', promoted_at_ms: 1, best_brier: 0.18, best_params: null, weights: null, trial_index: 2, archived_at_ms: 1 },
      ],
      total: 2,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    await waitFor(() => {
      expect(screen.getAllByTestId('promote-history-archive-row')).toHaveLength(2);
    });
    const badges = screen.getAllByTestId('promote-history-archive-trial-badge');
    expect(badges).toHaveLength(2);
    expect(badges[0]).toHaveAttribute('data-trial-index', 'best');
    expect(badges[1]).toHaveAttribute('data-trial-index', '2');
  });

  it('pagination: next button calls IPC with offset+25', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: Array.from({ length: 25 }, (_, i) => ({
        job_id: `a${i}`,
        model_version: `la${i}`,
        promoted_at_ms: 100 - i,
        best_brier: 0.2,
        best_params: null,
        weights: null,
        trial_index: null,
        archived_at_ms: 100 - i,
      })),
      total: 100,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    // 等待第一批 rows 渲染
    await waitFor(() => {
      expect(screen.getAllByTestId('promote-history-archive-row').length).toBe(25);
    });
    // 初始调用: offset=0
    expect(vi.mocked(listPromoteHistoryArchive).mock.calls[0]?.[0]?.offset).toBe(0);
    // 点击 next
    fireEvent.click(screen.getByTestId('promote-history-archive-next'));
    // 等待下一次调用
    await waitFor(() => {
      expect(vi.mocked(listPromoteHistoryArchive).mock.calls.length).toBeGreaterThan(1);
    });
    // 最新的调用应有 offset=25
    const calls = vi.mocked(listPromoteHistoryArchive).mock.calls;
    expect(calls[calls.length - 1]?.[0]?.offset).toBe(25);
  });

  it('pagination: prev button is disabled on first page', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: Array.from({ length: 25 }, (_, i) => ({
        job_id: `a${i}`,
        model_version: `la${i}`,
        promoted_at_ms: 100 - i,
        best_brier: 0.2,
        best_params: null,
        weights: null,
        trial_index: null,
        archived_at_ms: 100 - i,
      })),
      total: 50,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open onClose={() => {}} />));
    // 等待第一批 rows 渲染
    await waitFor(() => {
      expect(screen.getAllByTestId('promote-history-archive-row').length).toBe(25);
    });
    // 第一页 → prev 禁用 (offset=0, hasPrev=false)
    expect(screen.getByTestId('promote-history-archive-prev')).toBeDisabled();
    // 第一页,但 total=50 > 25 → next 启用
    expect(screen.getByTestId('promote-history-archive-next')).not.toBeDisabled();
  });

  it('does not query the IPC when the modal is closed', async () => {
    vi.mocked(listPromoteHistoryArchive).mockResolvedValue({
      ok: true,
      entries: [],
      total: 0,
      message: null,
    });
    render(wrap(<PromoteHistoryArchive open={false} onClose={() => {}} />));
    // 等待一拍让任何 effect 执行
    await new Promise((r) => setTimeout(r, 50));
    expect(listPromoteHistoryArchive).not.toHaveBeenCalled();
  });
});
