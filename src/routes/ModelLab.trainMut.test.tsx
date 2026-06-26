// v0.83c — ModelLab trainMut onSuccess/onError 分支（分支 71.8% → 74%）。
//
// trainMut（L265-303）共有 4 个可达分支：
//   1. r.status === 'completed' + best_brier != null  → 带 brier 详情的 toast
//   2. r.status === 'completed' + best_brier == null  → 详情为空的 toast
//   3. r.status !== 'completed'（如 'failed'）        → toast.error + setLastCandidate(null)
//   4. onError（mock 抛出）                            → toast.error + setActiveTrainJobId(null)
//
// 4 个分支都会执行 `setActiveTrainJobId(null)`（第 297 行）以及
// 2 次 query 失效（第 298-299 行）。这 2 次失效不在
// activeTrainJobId 监听器路径内 —— 它们在每次 train 之后
// 无条件触发，与结果无关。
//
// 模式：mock `trainJob` 返回自定义响应，点击 Train，断言 toast
// 以及 `lastCandidate` 是否被相应设置/清空。promoteAllMut
// 分支位于 TrainProgress 中（它需要 strict-mode mount 步骤），
// 留待后续轮次处理。
// @vitest-environment happy-dom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { createIpcMock } from '@/test-mocks';
import { createPrefsStoreMock } from '@/test-mocks';
import { useToastStore } from '@/stores/toast-store';

const mockListPromoteHistory = vi.fn();
const mockListPromoteHistoryArchive = vi.fn();
const mockGetActiveModel = vi.fn();
const mockTrainJob = vi.fn();
const mockPromoteModel = vi.fn();
const mockAutoPromoteIfBetter = vi.fn();
const mockPromoteAllTrials = vi.fn();
const mockLlmPerformance = vi.fn();
const mockSidecarPredict = vi.fn();
const mockSidecarHealthSnapshot = vi.fn();
const mockOnTrainStarted = vi.fn();
const mockOnAutoPromoteFinished = vi.fn();

vi.mock('@/ipc', () => createIpcMock({
  listPromoteHistory: () => mockListPromoteHistory(),
  listPromoteHistoryArchive: () => mockListPromoteHistoryArchive(),
  getActiveModel: () => mockGetActiveModel(),
  trainJob: () => mockTrainJob(),
  promoteModel: (...args: unknown[]) => mockPromoteModel(...args),
  autoPromoteIfBetter: (...args: unknown[]) => mockAutoPromoteIfBetter(...args),
  promoteAllTrials: () => mockPromoteAllTrials(),
  llmPerformance: () => mockLlmPerformance(),
  sidecarPredict: (...args: unknown[]) => mockSidecarPredict(...args),
  sidecarHealthSnapshot: () => mockSidecarHealthSnapshot(),
  onTrainStarted: () => mockOnTrainStarted(),
  onAutoPromoteFinished: () => mockOnAutoPromoteFinished(),
}));

vi.mock('@/stores/prefs-store', () => ({
  usePrefsStore: createPrefsStoreMock({
    autoPromoteAfterTrain: false,
    autoPromoteNotify: false,
    autoPromoteSkippedNotify: false,
    autoPromoteBrierMargin: 0.005,
    notificationsEnabled: false,
  }),
}));

import { ModelLab } from './ModelLab';

function wrapWithSpy() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  const invalidateSpy = vi.spyOn(qc, 'invalidateQueries');
  return { qc, invalidateSpy, ui: (
    <MemoryRouter>
      <QueryClientProvider client={qc}>
        <ModelLab />
      </QueryClientProvider>
    </MemoryRouter>
  ) };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockOnTrainStarted.mockResolvedValue(() => {});
  mockOnAutoPromoteFinished.mockResolvedValue(() => {});
  mockListPromoteHistory.mockResolvedValue({ entries: [] });
  mockListPromoteHistoryArchive.mockResolvedValue([]);
  mockGetActiveModel.mockResolvedValue(null);
  mockTrainJob.mockResolvedValue({ ok: true, message: 'started', job_id: 'train-default' });
  mockPromoteModel.mockResolvedValue({ promoted: true, model_version: 'm1' });
  mockPromoteAllTrials.mockResolvedValue({ promoted_count: 0 });
  mockLlmPerformance.mockResolvedValue([]);
  mockSidecarPredict.mockResolvedValue({ predictions: [], model_version: null, brier_score: null });
  mockSidecarHealthSnapshot.mockResolvedValue({ success_count: 0, failure_count: 0 });
  useToastStore.setState({ toasts: [] });
});

afterEach(() => cleanup());

describe('ModelLab trainMut branches (v0.83c)', () => {
  it('r.status=completed + best_brier=0.18 → toast with brier detail + lastCandidate set', async () => {
    mockTrainJob.mockResolvedValueOnce({
      status: 'completed',
      job_id: 'train-v83d-1',
      best_brier: 0.18,
      candidate_path: '/tmp/c-1.json',
    });
    const { ui } = wrapWithSpy();
    render(ui);
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
    });
    // lastCandidate 已设置 → model-last-candidate testid 可见
    await screen.findByTestId('model-last-candidate', {}, { timeout: 3000 });
  });

  it('r.status=completed + best_brier=null → toast with empty detail + lastCandidate set', async () => {
    mockTrainJob.mockResolvedValueOnce({
      status: 'completed',
      job_id: 'train-v83d-2',
      best_brier: null,
      candidate_path: null,
    });
    const { ui } = wrapWithSpy();
    render(ui);
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
    });
    await screen.findByTestId('model-last-candidate', {}, { timeout: 3000 });
  });

  it('r.status=failed → toast.error + lastCandidate NOT set', async () => {
    mockTrainJob.mockResolvedValueOnce({
      status: 'failed',
      job_id: 'train-v83d-3',
      message: 'sidecar crashed',
    });
    const { ui } = wrapWithSpy();
    render(ui);
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find((t) => t.kind === 'error');
      expect(err).toBeTruthy();
      // detail 在 store 中可能为 undefined；检查 title 或 detail
      const text = `${err?.title ?? ''} ${err?.body ?? ''}`;
      expect(text).toMatch(/Training failed/);
    });
    // lastCandidate 不应被设置 → model-last-candidate testid 缺失
    expect(screen.queryByTestId('model-last-candidate')).toBeNull();
  });

  it('onError: trainJob throws → toast.error', async () => {
    mockTrainJob.mockRejectedValueOnce(new Error('network timeout'));
    const { ui } = wrapWithSpy();
    render(ui);
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find((t) => t.kind === 'error');
      expect(err).toBeTruthy();
      const text = `${err?.title ?? ''} ${err?.body ?? ''}`;
      expect(text).toMatch(/Training failed|network timeout/);
    });
  });

  it('r.status=completed → invalidates llm-performance and sidecar-active-model queries', async () => {
    mockTrainJob.mockResolvedValueOnce({
      status: 'completed',
      job_id: 'train-v83d-5',
      best_brier: 0.20,
      candidate_path: '/tmp/c-5.json',
    });
    const { invalidateSpy, ui } = wrapWithSpy();
    render(ui);
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      const calls = invalidateSpy.mock.calls.map((c) => JSON.stringify(c[0]));
      expect(calls.some((c) => c.includes('llm-performance'))).toBe(true);
      expect(calls.some((c) => c.includes('sidecar-active-model'))).toBe(true);
    });
  });

  it('r.status=failed → still invalidates llm-performance and sidecar-active-model queries', async () => {
    mockTrainJob.mockResolvedValueOnce({
      status: 'failed',
      job_id: 'train-v83d-6',
      message: 'failed',
    });
    const { invalidateSpy, ui } = wrapWithSpy();
    render(ui);
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      const calls = invalidateSpy.mock.calls.map((c) => JSON.stringify(c[0]));
      expect(calls.some((c) => c.includes('llm-performance'))).toBe(true);
      expect(calls.some((c) => c.includes('sidecar-active-model'))).toBe(true);
    });
  });

  it('r.status=completed (no best_brier) → still invalidates queries', async () => {
    mockTrainJob.mockResolvedValueOnce({
      status: 'completed',
      job_id: 'train-v83d-7',
      best_brier: null,
      candidate_path: null,
    });
    const { invalidateSpy, ui } = wrapWithSpy();
    render(ui);
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      const calls = invalidateSpy.mock.calls.map((c) => JSON.stringify(c[0]));
      expect(calls.some((c) => c.includes('llm-performance'))).toBe(true);
      expect(calls.some((c) => c.includes('sidecar-active-model'))).toBe(true);
    });
  });
});
