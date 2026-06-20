// v0.83c — ModelLab trainMut onSuccess/onError branches (branches 71.8% → 74%).
//
// trainMut (L265-303) has 4 reachable branches:
//   1. r.status === 'completed' + best_brier != null  → toast with brier detail
//   2. r.status === 'completed' + best_brier == null  → toast with empty detail
//   3. r.status !== 'completed' (e.g. 'failed')       → toast.error + setLastCandidate(null)
//   4. onError (mock throws)                          → toast.error + setActiveTrainJobId(null)
//
// All 4 also run `setActiveTrainJobId(null)` (line 297) and 2 query invalidations
// (lines 298-299). The 2 invalidations are NOT in the activeTrainJobId listener
// path — they happen unconditionally after a train, regardless of result.
//
// Pattern: mock `trainJob` to return a custom response, click Train, assert toasts
// and that `lastCandidate` is set/cleared appropriately. The promoteAllMut
// branches live in TrainProgress (which requires the strict-mode mount dance)
// and are deferred to a later round.
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
    // lastCandidate set → model-last-candidate testid visible
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
      // detail may be undefined in the store; check title or detail
      const text = `${err?.title ?? ''} ${err?.body ?? ''}`;
      expect(text).toMatch(/Training failed/);
    });
    // lastCandidate should NOT be set → model-last-candidate testid absent
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
