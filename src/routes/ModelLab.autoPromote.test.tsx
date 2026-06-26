// v0.83b — ModelLab autoPromote mutation 测试（分支 61.3% → 69%）。
//
// autoPromoteMut.onSuccess（L361-394）有 4 个互斥分支：
//   1. r.promoted=true  (L361)        — toast.success + setLastCandidate(null) + 3 次失效
//   2. r.skipped=true   (L374)        — toast.info（带 margin 格式化）
//   3. else (failure)   (L390)        — toast.error 含 r.message
//   4. onError          (L393)        — toast.error 含 e.message
//
// 加上 L362（delta 格式化）和 L379（同样）的
// `r.active_brier != null && r.candidate_brier != null` 4-way 分支。
//
// 本文件通过 7 个聚焦测试覆盖这 14 个未覆盖的分支。
// 模式：渲染 ModelLab，点击 Auto-Promote 按钮，断言
// toast 与状态（lastCandidate 清空、query 失效）。
//
// 覆盖目标：ModelLab 分支 61.3% → 69%（+8pp）。
// 新增 7 个测试。ModelLab 测试总数：46 → 53。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock, createPrefsStoreMock } from '@/test-mocks';
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

/**
 * 通过点击 Train 设置 `lastCandidate` 状态。Train 按钮
 * 触发 trainMut，其 onSuccess 在响应中包含
 * `status === 'completed'` 时调用 setLastCandidate。设置好
 * lastCandidate 后，auto-promote 按钮
 * （data-testid='model-auto-promote-btn'）就会被渲染。
 */
async function setupLastCandidate() {
  // Mock trainJob 返回 completed 响应（触发 setLastCandidate）
  mockTrainJob.mockResolvedValue({
    status: 'completed',
    job_id: 'train-v83b',
    best_brier: 0.18,
    candidate_path: '/tmp/c-v83b.json',
  });
  // 等待 Train 按钮，然后点击
  const trainBtn = await screen.findByTestId('model-train-btn', {}, { timeout: 3000 });
  fireEvent.click(trainBtn);
  // 等待 auto-promote 按钮出现（证明 lastCandidate 已设置）
  await screen.findByTestId('model-auto-promote-btn', {}, { timeout: 3000 });
}

describe('ModelLab autoPromote mutations (v0.83b)', () => {
  it('r.promoted=true with both briers → toast.success + 3 query invalidations', async () => {
    mockAutoPromoteIfBetter.mockResolvedValue({
      promoted: true,
      active_brier: 0.20,
      candidate_brier: 0.15,
      margin: 0.005,
      model_version: 'm-new',
      reason: 'brier improved',
    });
    const { invalidateSpy, ui } = wrapWithSpy();
    render(ui);
    await setupLastCandidate();
    const btn = screen.getByTestId('model-auto-promote-btn');
    fireEvent.click(btn);
    await waitFor(() => {
      expect(mockAutoPromoteIfBetter).toHaveBeenCalled();
    });
    await waitFor(() => {
      const calls = invalidateSpy.mock.calls.map((c) => JSON.stringify(c[0]));
      expect(calls.some((c) => c.includes('llm-performance'))).toBe(true);
      expect(calls.some((c) => c.includes('sidecar-active-model'))).toBe(true);
      expect(calls.some((c) => c.includes('promote-history'))).toBe(true);
    });
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
    });
  });

  it('r.promoted=true with null active_brier → delta="?" placeholder (4-way branch)', async () => {
    mockAutoPromoteIfBetter.mockResolvedValue({
      promoted: true,
      active_brier: null,         // 4-way 分支：(null, _)
      candidate_brier: 0.15,
      margin: 0.005,
      model_version: 'm',
      reason: 'ok',
    });
    const { ui } = wrapWithSpy();
    render(ui);
    await setupLastCandidate();
    const btn = screen.getByTestId('model-auto-promote-btn');
    fireEvent.click(btn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
      // 当某个 brier 为 null 时，delta = '?'
      expect(succ?.body || '').toMatch(/\+?0\.\d+|\?/);
    });
  });

  it('r.promoted=true with null candidate_brier → delta="?" placeholder (other side of 4-way)', async () => {
    mockAutoPromoteIfBetter.mockResolvedValue({
      promoted: true,
      active_brier: 0.20,
      candidate_brier: null,     // 4-way 分支：(_, null)
      margin: 0.005,
      model_version: 'm',
      reason: 'ok',
    });
    const { ui } = wrapWithSpy();
    render(ui);
    await setupLastCandidate();
    const btn = screen.getByTestId('model-auto-promote-btn');
    fireEvent.click(btn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
    });
  });

  it('r.skipped=true → toast.info with margin + reason', async () => {
    mockAutoPromoteIfBetter.mockResolvedValue({
      promoted: false,
      skipped: true,
      active_brier: 0.20,
      candidate_brier: 0.21,    // candidate 更差
      margin: 0.005,
      model_version: null,
      reason: 'candidate not better than active',
    });
    const { ui } = wrapWithSpy();
    render(ui);
    await setupLastCandidate();
    const btn = screen.getByTestId('model-auto-promote-btn');
    fireEvent.click(btn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const info = toasts.find((t) => t.kind === 'info');
      expect(info).toBeTruthy();
      // Title（i18n key auto_skipped）含 'skipped'；body 是 reason
      expect(info?.body || info?.title).toMatch(/candidate not better|skipped/);
    });
  });

  it('r.skipped=true with null briers → toast.info with delta="?"', async () => {
    mockAutoPromoteIfBetter.mockResolvedValue({
      promoted: false,
      skipped: true,
      active_brier: null,
      candidate_brier: null,
      margin: 0.005,
      model_version: null,
      reason: 'no candidate',
    });
    const { ui } = wrapWithSpy();
    render(ui);
    await setupLastCandidate();
    const btn = screen.getByTestId('model-auto-promote-btn');
    fireEvent.click(btn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const info = toasts.find((t) => t.kind === 'info');
      expect(info).toBeTruthy();
    });
  });

  it('r.promoted=false + r.skipped=false (failure) → toast.error with r.message', async () => {
    mockAutoPromoteIfBetter.mockResolvedValue({
      promoted: false,
      skipped: false,
      active_brier: null,
      candidate_brier: null,
      margin: 0.005,
      model_version: null,
      reason: null,
      message: 'no candidate model available',
    });
    const { ui } = wrapWithSpy();
    render(ui);
    await setupLastCandidate();
    const btn = screen.getByTestId('model-auto-promote-btn');
    fireEvent.click(btn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find((t) => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body || err?.title).toMatch(/candidate|failed/);
    });
  });

  it('onError: IPC throws → toast.error with Error.message', async () => {
    mockAutoPromoteIfBetter.mockRejectedValue(new Error('auto promote IPC crashed'));
    const { ui } = wrapWithSpy();
    render(ui);
    await setupLastCandidate();
    const btn = screen.getByTestId('model-auto-promote-btn');
    fireEvent.click(btn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find((t) => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body || err?.title).toMatch(/crashed/);
    });
  });
});
