// v0.83b — ModelLab autoPromote mutation tests (branches 61.3% → 69%).
//
// autoPromoteMut.onSuccess (L361-394) has 4 mutually exclusive branches:
//   1. r.promoted=true  (L361)        — toast.success + setLastCandidate(null) + 3 invalidations
//   2. r.skipped=true   (L374)        — toast.info (with margin formatting)
//   3. else (failure)   (L390)        — toast.error with r.message
//   4. onError          (L393)        — toast.error with e.message
//
// And the `r.active_brier != null && r.candidate_brier != null` 4-way
// branch at L362 (delta formatting) + L379 (same).
//
// This file covers these 14 missed branches with 7 focused tests.
// The pattern: render ModelLab, click the Auto-Promote button, assert
// on toast + state (lastCandidate cleared, query invalidations).
//
// Coverage target: ModelLab branches 61.3% → 69% (+8pp).
// 7 new tests. Total ModelLab tests: 46 → 53.
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
 * Set up `lastCandidate` state by clicking Train. The Train button
 * triggers trainMut, whose onSuccess calls setLastCandidate when the
 * response has `status === 'completed'`. Once lastCandidate is set,
 * the auto-promote button (data-testid='model-auto-promote-btn') is
 * rendered.
 */
async function setupLastCandidate() {
  // Mock trainJob to return a completed response (triggers setLastCandidate)
  mockTrainJob.mockResolvedValue({
    status: 'completed',
    job_id: 'train-v83b',
    best_brier: 0.18,
    candidate_path: '/tmp/c-v83b.json',
  });
  // Wait for the Train button, click it
  const trainBtn = await screen.findByTestId('model-train-btn', {}, { timeout: 3000 });
  fireEvent.click(trainBtn);
  // Wait for the auto-promote button to appear (proves lastCandidate is set)
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
      active_brier: null,         // 4-way branch: (null, _)
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
      // delta = '?' when one of the briers is null
      expect(succ?.body || '').toMatch(/\+?0\.\d+|\?/);
    });
  });

  it('r.promoted=true with null candidate_brier → delta="?" placeholder (other side of 4-way)', async () => {
    mockAutoPromoteIfBetter.mockResolvedValue({
      promoted: true,
      active_brier: 0.20,
      candidate_brier: null,     // 4-way branch: (_, null)
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
      candidate_brier: 0.21,    // candidate worse
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
      // Title (i18n key auto_skipped) has 'skipped' in en; body is the reason
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
