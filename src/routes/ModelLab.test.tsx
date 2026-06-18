// v0.57b — ModelLab component tests.
//
// The ModelLab is the v0.17+ lifecycle page. It
// surfaces:
//   - Train / promote / rollback controls
//   - Last candidate summary
//   - Auto-promote toggle
//   - Promote history chart
//   - Model comparison (v0.42e)
//   - Backtest modal (v0.43d)
//   - Archive viewer (v0.34b)
//
// Today it has 6 data-testids but no tests.
// This file covers the surface-level rendering
// of the page (Train + Promote buttons, last
// candidate card, archive view) without
// exercising the full lifecycle (which is
// tested in Rust via sidecar_e2e).

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  // IPCs ModelLab uses
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
  // Event listeners
  onTrainStarted: vi.fn().mockResolvedValue(() => {}),
  onAutoPromoteFinished: vi.fn().mockResolvedValue(() => {}),
  // Other IPCs (not used in these tests but
  // imported by the component)
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

  // Note: the Promote / Auto-promote / Last
  // candidate / Compare buttons are conditional
  // on a `lastCandidate` local state that's set
  // by the train-completion event flow. They
  // would need a fireEvent-driven test (click
  // Train → wait for event → assert on the new
  // buttons). The lifecycle flow itself is
  // covered by the Rust sidecar_e2e.rs.
});
