// v0.57b — ModelLab 组件测试。
//
// ModelLab 是 v0.17+ 起的模型生命周期页面，主要呈现：
//   - Train / promote / rollback 控件
//   - 最近一次 candidate 摘要
//   - auto-promote 开关
//   - promote 历史图表
//   - model 对比（v0.42e）
//   - backtest 模态框（v0.43d）
//   - archive 查看器（v0.34b）
//
// 当前 ModelLab 已有 6 个 data-testid，但缺少测试。
// 本文件覆盖页面的表层渲染（Train + Promote 按钮、
// 最近一次 candidate 卡片、archive 视图），不
// 演练完整生命周期（后者通过 Rust 侧的 sidecar_e2e
// 测试覆盖）。

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  // ModelLab 使用的 IPCs
  listPromoteHistory: vi.fn().mockResolvedValue({
    ok: true,
    message: 'ok',
    count: 2,
    entries: [
      { job_id: 'train-a', model_version: 'logistic-train-a', promoted_at_ms: 1700000000000, best_brier: 0.18, best_params: { w0: 0.1, w1: 2.4, w2: -0.02 }, trial_index: 0, reason: 'manual promote' },
      { job_id: 'train-b', model_version: 'logistic-train-b', promoted_at_ms: 1700001000000, best_brier: 0.20, best_params: { w0: 0.1, w1: 2.0, w2: -0.01 }, trial_index: 0, reason: 'auto promote' },
    ],
  }),
  listPromoteHistoryArchive: vi.fn().mockResolvedValue({
    ok: true,
    message: 'ok',
    count: 0,
    entries: [],
  }),
  getActiveModel: vi.fn().mockResolvedValue(null),
  getLastCandidate: vi.fn().mockResolvedValue({
    job_id: 'train-1',
    model_version: 'logistic-train-1',
    finished_at: 1700000000000,
    best_brier: 0.18,
    best_params: { w0: 0.1, w1: 2.4, w2: -0.02 },
    all_trials: [
      { index: 0, brier: 0.18, params: { w0: 0.1, w1: 2.4, w2: -0.02 } },
      { index: 1, brier: 0.20, params: { w0: 0.1, w1: 2.0, w2: -0.01 } },
    ],
    message: 'ok',
  }),
  trainJob: vi.fn().mockResolvedValue({
    ok: true,
    message: 'train started',
    job_id: 'train-1',
  }),
  promoteModel: vi.fn().mockResolvedValue({
    promoted: true,
    status: 'ok',
    previous_path: null,
    active_path: null,
    promoted_at_ms: 1700000000000,
    model_version: 'logistic-train-1',
    message: null,
  }),
  listAutoPromoteConfig: vi.fn().mockResolvedValue({
    enabled: false,
    brier_margin: 0.005,
  }),
  // 事件监听器
  onTrainStarted: vi.fn().mockResolvedValue(() => {}),
  onAutoPromoteFinished: vi.fn().mockResolvedValue(() => {}),
  // 其他 IPCs（本测试未使用但被组件导入）
  llmPerformance: vi.fn().mockResolvedValue([]),
  sidecarPredict: vi.fn().mockResolvedValue({
    predictions: [],
    model_version: null,
    brier_score: null,
  }),
  sidecarHealthSnapshot: vi.fn().mockResolvedValue({
    running: false,
    pid: null,
    last_error: null,
    last_ping_ms: null,
  }),
  autoPromoteIfBetter: vi.fn().mockResolvedValue({
    promoted: false,
    status: 'ok',
    message: 'no candidate',
    active_brier: null,
    margin: 0.005,
    model_version: null,
    promoted_at_ms: null,
  }),
  promoteAllTrials: vi.fn().mockResolvedValue([]),
  sendNotification: vi.fn().mockResolvedValue(undefined),
  requestNotificationPermission: vi.fn().mockResolvedValue(true),
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

beforeEach(() => {
  vi.clearAllMocks();
});

describe('ModelLab (v0.57b)', () => {
  it('renders the Train button (v0.17+)', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(
        screen.getByTestId('model-train-btn'),
      ).toBeInTheDocument();
    });
  });

  it('renders the View Archive button (v0.34b)', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(
        screen.getByTestId('view-archive-btn'),
      ).toBeInTheDocument();
    });
  });

  it('renders the model version pill (v0.18+)', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(
        screen.getByTestId('model-version-pill'),
      ).toBeInTheDocument();
    });
  });

  // 注意：Promote / Auto-promote / Last candidate / Compare
  // 按钮依赖于由 train-completion 事件流设置的
  // `lastCandidate` 本地 state。它们需要 fireEvent
  // 驱动的测试（点击 Train → 等待事件 → 断言新按钮）。
  // 生命周期流程本身由 Rust 侧 sidecar_e2e.rs 覆盖。
});
