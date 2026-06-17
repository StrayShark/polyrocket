// @vitest-environment happy-dom
/**
 * TrainProgress component tests (v0.17e).
 *
 * Strategy: mock `@/ipc` so the 2 `on*` listen functions
 * return no-op unlisten functions immediately. The tests
 * then render the component with a fixed `jobId` and
 * fire synthetic `started` / `finished` events, asserting
 * the structural DOM (test ids, status, table rows, best
 * highlight).
 */
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, act } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Mock `@/ipc` so we control the event stream.
// ---------------------------------------------------------------------------

type StartedCb = (e: { job_id: string; n_trials: number; epochs: number; started_at: number }) => void;
type FinishedCb = (e: { job_id: string; status: string; best_brier: number | null; best_params: Record<string, number> | null; trials: Array<{ lr: number; reg: number; brier: number; weights: Record<string, number> }>; duration_ms: number; candidate_path: string | null; message: string | null; finished_at: number }) => void;

let startedSubs: StartedCb[] = [];
let finishedSubs: FinishedCb[] = [];

vi.mock('@/ipc', () => ({
  onTrainStarted: (cb: StartedCb) => {
    startedSubs.push(cb);
    return Promise.resolve(() => { startedSubs = startedSubs.filter((s) => s !== cb); });
  },
  onTrainFinished: (cb: FinishedCb) => {
    finishedSubs.push(cb);
    return Promise.resolve(() => { finishedSubs = finishedSubs.filter((s) => s !== cb); });
  },
}));

async function flushListeners() {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
}

function fireStarted(e: Parameters<StartedCb>[0]) {
  act(() => { for (const s of [...startedSubs]) s(e); });
}
function fireFinished(e: Parameters<FinishedCb>[0]) {
  act(() => { for (const s of [...finishedSubs]) s(e); });
}

const origError = console.error;
beforeEach(() => {
  console.error = vi.fn();
  startedSubs = [];
  finishedSubs = [];
});
afterEach(() => {
  cleanup();
  console.error = origError;
});

import { TrainProgress } from './TrainProgress';

const JID = 'train-test-1';

describe('TrainProgress (v0.17e)', () => {
  it('renders nothing when jobId is null', () => {
    const { container } = render(<TrainProgress jobId={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when jobId is set but no started event', () => {
    const { container } = render(<TrainProgress jobId={JID} />);
    expect(container.firstChild).toBeNull();
  });

  it('seeds a "running" header on the started event', async () => {
    render(<TrainProgress jobId={JID} />);
    await flushListeners();
    fireStarted({ job_id: JID, n_trials: 4, epochs: 80, started_at: 1000 });
    const grid = screen.getByTestId('train-progress');
    expect(grid).toBeInTheDocument();
    expect(grid).toHaveAttribute('data-running', 'true');
    expect(grid).toHaveAttribute('data-job-id', JID);
  });

  it('renders the per-trial table with the best trial highlighted', async () => {
    render(<TrainProgress jobId={JID} />);
    await flushListeners();
    fireStarted({ job_id: JID, n_trials: 4, epochs: 80, started_at: 1000 });
    fireFinished({
      job_id: JID,
      status: 'completed',
      best_brier: 0.184,
      best_params: { w0: 0.10, w1: 0.20, w2: 0.30 },
      trials: [
        { lr: 0.05, reg: 0.01, brier: 0.184, weights: { w0: 0.10, w1: 0.20, w2: 0.30 } },
        { lr: 0.10, reg: 0.01, brier: 0.210, weights: { w0: 0.10, w1: 0.20, w2: 0.30 } },
        { lr: 0.05, reg: 0.10, brier: 0.225, weights: { w0: 0.10, w1: 0.20, w2: 0.30 } },
        { lr: 0.10, reg: 0.10, brier: 0.243, weights: { w0: 0.10, w1: 0.20, w2: 0.30 } },
      ],
      duration_ms: 4200,
      candidate_path: '/home/x/.polyrocket/sidecar/models/candidate.json',
      message: null,
      finished_at: 5200,
    });
    const grid = screen.getByTestId('train-progress');
    expect(grid).toHaveAttribute('data-running', 'false');
    expect(grid).toHaveAttribute('data-status', 'completed');
    // Trial table: 4 rows
    for (let i = 0; i < 4; i++) {
      const row = screen.getByTestId(`train-progress-row-${i}`);
      expect(row).toBeInTheDocument();
    }
    // The best (lowest brier) is row 0
    expect(screen.getByTestId('train-progress-row-0')).toHaveAttribute('data-best', 'true');
    expect(screen.getByTestId('train-progress-row-1')).toHaveAttribute('data-best', 'false');
    expect(screen.getByTestId('train-progress-row-3')).toHaveAttribute('data-best', 'false');
    // Best footer
    expect(screen.getByTestId('train-progress-best')).toBeInTheDocument();
  });

  it('shows the failure banner when the train fails', async () => {
    render(<TrainProgress jobId={JID} />);
    await flushListeners();
    fireStarted({ job_id: JID, n_trials: 4, epochs: 80, started_at: 1000 });
    fireFinished({
      job_id: JID,
      status: 'failed',
      best_brier: null,
      best_params: null,
      trials: [],
      duration_ms: 500,
      candidate_path: null,
      message: 'sidecar not running',
      finished_at: 1500,
    });
    const grid = screen.getByTestId('train-progress');
    expect(grid).toHaveAttribute('data-status', 'failed');
    const errEl = screen.getByTestId('train-progress-error');
    expect(errEl.textContent).toBe('sidecar not running');
  });

  it('filters events from a different jobId', async () => {
    render(<TrainProgress jobId={JID} />);
    await flushListeners();
    fireStarted({ job_id: 'train-OTHER', n_trials: 4, epochs: 80, started_at: 1000 });
    // The component is keyed to JID, not the OTHER id. So it
    // should still be hidden.
    expect(screen.queryByTestId('train-progress')).toBeNull();
  });

  it('clears state when jobId becomes null', async () => {
    const { rerender, container } = render(<TrainProgress jobId={JID} />);
    await flushListeners();
    fireStarted({ job_id: JID, n_trials: 4, epochs: 80, started_at: 1000 });
    expect(screen.getByTestId('train-progress')).toBeInTheDocument();
    rerender(<TrainProgress jobId={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('highlights the correct best trial (not always row 0)', async () => {
    render(<TrainProgress jobId={JID} />);
    await flushListeners();
    fireStarted({ job_id: JID, n_trials: 3, epochs: 80, started_at: 1000 });
    fireFinished({
      job_id: JID,
      status: 'completed',
      best_brier: 0.150,
      best_params: { w0: 0.05, w1: 0.10, w2: 0.15 },
      trials: [
        { lr: 0.05, reg: 0.01, brier: 0.300, weights: { w0: 0.1, w1: 0.2, w2: 0.3 } },
        { lr: 0.10, reg: 0.01, brier: 0.150, weights: { w0: 0.05, w1: 0.10, w2: 0.15 } },
        { lr: 0.05, reg: 0.10, brier: 0.400, weights: { w0: 0.1, w1: 0.2, w2: 0.3 } },
      ],
      duration_ms: 3000,
      candidate_path: '/tmp/c.json',
      message: null,
      finished_at: 4000,
    });
    // Row 1 has the lowest brier (0.150) — it should be the best
    expect(screen.getByTestId('train-progress-row-0')).toHaveAttribute('data-best', 'false');
    expect(screen.getByTestId('train-progress-row-1')).toHaveAttribute('data-best', 'true');
    expect(screen.getByTestId('train-progress-row-2')).toHaveAttribute('data-best', 'false');
  });
});
