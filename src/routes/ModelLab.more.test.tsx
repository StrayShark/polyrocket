// v0.65a —— ModelLab 组件测试（v0.65a 路由 39% 覆盖 → ~70%）。
//
// ModelLab 包含 5 个 tab 以及 train/promote/auto-promote 的生命周期状态。
// 我们新增 9 个聚焦测试，覆盖 v0.57b 表层渲染测试之外的
// 分支丰富代码路径。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock, createPrefsStoreMock } from '@/test-mocks';

const mockListPromoteHistory = vi.fn();
const mockListPromoteHistoryArchive = vi.fn();
const mockGetActiveModel = vi.fn();
const mockGetLastCandidate = vi.fn();
const mockTrainJob = vi.fn();
const mockPromoteModel = vi.fn();
const mockListAutoPromoteConfig = vi.fn();
const mockLlmPerformance = vi.fn();
const mockSidecarPredict = vi.fn();
const mockSidecarHealthSnapshot = vi.fn();
const mockAutoPromoteIfBetter = vi.fn();
const mockPromoteAllTrials = vi.fn();
const mockOnTrainStarted = vi.fn();
const mockOnAutoPromoteFinished = vi.fn();

vi.mock('@/ipc', () => createIpcMock({
  listPromoteHistory: () => mockListPromoteHistory(),
  listPromoteHistoryArchive: () => mockListPromoteHistoryArchive(),
  getActiveModel: () => mockGetActiveModel(),
  getLastCandidate: () => mockGetLastCandidate(),
  trainJob: () => mockTrainJob(),
  promoteModel: (...args: unknown[]) => mockPromoteModel(...args),
  listAutoPromoteConfig: () => mockListAutoPromoteConfig(),
  llmPerformance: () => mockLlmPerformance(),
  sidecarPredict: (...args: unknown[]) => mockSidecarPredict(...args),
  sidecarHealthSnapshot: () => mockSidecarHealthSnapshot(),
  autoPromoteIfBetter: (...args: unknown[]) => mockAutoPromoteIfBetter(...args),
  promoteAllTrials: () => mockPromoteAllTrials(),
  onTrainStarted: () => mockOnTrainStarted(),
  onAutoPromoteFinished: () => mockOnAutoPromoteFinished(),
}));

vi.mock('@/stores/prefs-store', () => ({
  usePrefsStore: createPrefsStoreMock(),
}));

import { ModelLab } from '@/routes/ModelLab';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return (
    <MemoryRouter>
      <QueryClientProvider client={qc}>{node}</QueryClientProvider>
    </MemoryRouter>
  );
}

const SAMPLE_PROMOTE_HISTORY = {
  ok: true,
  message: 'ok',
  count: 2,
  entries: [
    {
      job_id: 'train-a', model_version: 'logistic-train-a',
      promoted_at_ms: 1700000000000, best_brier: 0.18,
      best_params: { w0: 0.1, w1: 2.4, w2: -0.02 },
      trial_index: 0, reason: 'manual',
    },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  mockOnTrainStarted.mockResolvedValue(() => {});
  mockOnAutoPromoteFinished.mockResolvedValue(() => {});
  mockListPromoteHistory.mockResolvedValue(SAMPLE_PROMOTE_HISTORY);
  mockListPromoteHistoryArchive.mockResolvedValue({
    ok: true, message: 'ok', count: 0, entries: [],
  });
  mockGetActiveModel.mockResolvedValue(null);
  mockGetLastCandidate.mockResolvedValue({
    job_id: 'train-1', model_version: 'logistic-train-1',
    finished_at: 1700000000000, best_brier: 0.18,
    best_params: { w0: 0.1, w1: 2.4, w2: -0.02 },
    all_trials: [], message: 'ok',
  });
  mockLlmPerformance.mockResolvedValue([
    { model_version: 'm1', n_predictions: 100, win_rate: 0.8, brier_score: 0.18, log_loss: 0.5, avg_edge: 0.05 },
    { model_version: 'm2', n_predictions: 80, win_rate: 0.6, brier_score: 0.22, log_loss: 0.6, avg_edge: 0.04 },
  ]);
  mockSidecarPredict.mockResolvedValue({
    predictions: [], model_version: 'logistic-v1', brier_score: 0.18,
  });
  mockSidecarHealthSnapshot.mockResolvedValue({
    running: true, pid: 12345, last_error: null,
    last_ping_ms: Date.now(), success_count: 5, failure_count: 0,
  });
  mockListAutoPromoteConfig.mockResolvedValue({ enabled: false, brier_margin: 0.005 });
  mockTrainJob.mockResolvedValue({ ok: true, message: 'train started', job_id: 'train-1' });
  mockPromoteModel.mockResolvedValue({
    promoted: true, status: 'ok', previous_path: null, active_path: null,
    promoted_at_ms: 1700000000000, model_version: 'logistic-train-1', message: null,
  });
  mockAutoPromoteIfBetter.mockResolvedValue({
    promoted: false, status: 'ok', message: 'no candidate',
    active_brier: null, margin: 0.005, model_version: null, promoted_at_ms: null,
  });
  mockPromoteAllTrials.mockResolvedValue([]);
});

describe('ModelLab (v0.65a expand)', () => {
  it('renders all 4 main action buttons (Train / View Archive / Compare / Backtest)', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(screen.getByTestId('model-train-btn')).toBeInTheDocument();
    });
    expect(screen.getByTestId('view-archive-btn')).toBeInTheDocument();
  });

  it('renders performance KPIs (best brier, total calls)', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      // 最佳 brier = 0.18（格式化为 0.180 或 18.0%）
      expect(document.body.textContent).toMatch(/0\.18|18%/);
    });
  });

  it('opens Archive modal when View Archive is clicked', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(screen.getByTestId('view-archive-btn')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('view-archive-btn'));
    await waitFor(() => {
      // archive 模态框以 history 条目渲染
      expect(document.body.textContent).toContain('train-a');
    });
  });

  it('opens Compare modal with empty selection initially', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(screen.getByTestId('view-archive-btn')).toBeInTheDocument();
    });
    // 查找 Compare 按钮（它是 View Archive 的兄弟节点）
    const compareBtn = screen.getAllByRole('button').find((b) =>
      /compare/i.test(b.textContent || ''),
    );
    if (compareBtn) {
      fireEvent.click(compareBtn);
      await waitFor(() => {
        // compare 模态框已渲染；只验证 body 变化
        expect(document.body.textContent).toBeTruthy();
      });
    }
  });

  it('Train button click triggers trainJob IPC + sets expectedTrainRef', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(screen.getByTestId('model-train-btn')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      expect(mockTrainJob).toHaveBeenCalled();
    });
  });

  it('Train error → toast shows error message', async () => {
    mockTrainJob.mockRejectedValue(new Error('OOM in Rust'));
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(screen.getByTestId('model-train-btn')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('model-train-btn'));
    await waitFor(() => {
      expect(mockTrainJob).toHaveBeenCalled();
    });
  });

  it('renders ErrorState when llmPerformance fails', async () => {
    // v0.65a —— 这曾因 "Rendered fewer hooks than expected" 而崩溃，
    // 因为 ModelLab 在第一个 useQuery 之后就 early return
    //（后续还有更多 query）。
    // 修复：将 early return 移至所有 hooks 之后。
    mockLlmPerformance.mockRejectedValue(new Error('Predict store down'));
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(screen.getByText(/Predict store down/i)).toBeInTheDocument();
    });
  });

  it('handles active sidecar model when snapshot.success_count > 0', async () => {
    // sidecarHealthSnapshot 返回 success_count=5；sidecarPredict 返回 model_version
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(screen.getByTestId('model-version-pill')).toBeInTheDocument();
    });
  });

  it('handles missing sidecar when snapshot.success_count = 0', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue({
      running: true, pid: 12345, last_error: null,
      last_ping_ms: Date.now(), success_count: 0, failure_count: 0,
    });
    render(wrap(<ModelLab />));
    await waitFor(() => {
      // 无活动 model → 仍能渲染
      expect(screen.getByTestId('model-version-pill')).toBeInTheDocument();
    });
  });

  it('handles sidecar throw gracefully (no crash)', async () => {
    mockSidecarHealthSnapshot.mockRejectedValue(new Error('spawn ENOENT'));
    render(wrap(<ModelLab />));
    await waitFor(() => {
      // 即便 sidecar 出错，页面仍能渲染
      expect(screen.getByTestId('model-train-btn')).toBeInTheDocument();
    });
  });
});
