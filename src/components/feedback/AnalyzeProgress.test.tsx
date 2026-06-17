// @vitest-environment happy-dom
/**
 * AnalyzeProgress component tests (v0.15d).
 *
 * Strategy: mock `@/ipc` so the 4 `on*` listen functions
 * return no-op unlisten functions immediately. The tests
 * then render the component with a fixed `analysisId` and
 * assert the structural DOM (test ids, status pills, etc.).
 *
 * The mock is also used to **simulate events**: instead of
 * firing real Tauri events, we expose a `__trigger*` helper
 * from the mocked module. The test calls e.g.
 * `__triggerProviderDone({...})` and asserts the DOM updates.
 *
 * This keeps the tests fast (no Tauri's event bus) and
 * deterministic (no timing). Each fire* is wrapped in act()
 * to flush React's state updates.
 */
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup, act } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Mock `@/ipc` so we control the event stream.
// ---------------------------------------------------------------------------

type StartedCb = (e: { analysis_id: string; market_id: string; prompt_version: string; providers: string[]; started_at: number }) => void;
type ProviderCb = (e: { analysis_id: string; provider_id: string; ok: boolean; latency_ms: number; tokens_in: number | null; tokens_out: number | null; cost_cents: number; error_kind: string; error_message: string | null; finished_at: number }) => void;
type ConsensusCb = (e: { analysis_id: string; status: 'completed' | 'partial' | 'failed'; n_success: number; n_failed: number; consensus_pred: number | null; consensus_side: string | null; consensus_conf: number | null }) => void;
type FinishedCb = (e: { analysis_id: string; status: 'completed' | 'partial' | 'failed'; total_latency_ms: number; total_cost_cents: number; n_success: number; n_failed: number; finished_at: number }) => void;

let startedSubs: StartedCb[] = [];
let providerSubs: ProviderCb[] = [];
let consensusSubs: ConsensusCb[] = [];
let finishedSubs: FinishedCb[] = [];

vi.mock('@/ipc', () => ({
  onAnalyzeStarted: (cb: StartedCb) => {
    startedSubs.push(cb);
    return Promise.resolve(() => { startedSubs = startedSubs.filter((s) => s !== cb); });
  },
  onProviderDone: (cb: ProviderCb) => {
    providerSubs.push(cb);
    return Promise.resolve(() => { providerSubs = providerSubs.filter((s) => s !== cb); });
  },
  onConsensusDone: (cb: ConsensusCb) => {
    consensusSubs.push(cb);
    return Promise.resolve(() => { consensusSubs = consensusSubs.filter((s) => s !== cb); });
  },
  onAnalyzeFinished: (cb: FinishedCb) => {
    finishedSubs.push(cb);
    return Promise.resolve(() => { finishedSubs = finishedSubs.filter((s) => s !== cb); });
  },
}));

// Wait for the component's useEffect to register its listeners.
// The component calls onAnalyzeStarted(...) which returns a Promise
// that resolves on the next microtask; the .then chains another
// microtask before pushing to unsubs. So we need to flush 2 ticks
// of microtasks plus a setTimeout(0) to be safe.
async function flushListeners() {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
}

// Helpers to push events to all subscribers. Wrapped in act() so
// React's setState calls schedule a re-render and the DOM is
// up-to-date by the time the test asserts.
function fireStarted(e: Parameters<StartedCb>[0]) {
  act(() => {
    for (const s of [...startedSubs]) s(e);
  });
}
function fireProviderDone(e: Parameters<ProviderCb>[0]) {
  act(() => {
    for (const s of [...providerSubs]) s(e);
  });
}
function fireConsensus(e: Parameters<ConsensusCb>[0]) {
  act(() => {
    for (const s of [...consensusSubs]) s(e);
  });
}
function fireFinished(e: Parameters<FinishedCb>[0]) {
  act(() => {
    for (const s of [...finishedSubs]) s(e);
  });
}

// Suppress console.error for the "act()" warnings
// (the mocked on* functions are async; React warnings).
const origError = console.error;
beforeEach(() => {
  console.error = vi.fn();
  startedSubs = [];
  providerSubs = [];
  consensusSubs = [];
  finishedSubs = [];
});
afterEach(() => {
  cleanup();
  console.error = origError;
});

// Now import the component (must be after the mock)
import { AnalyzeProgress } from './AnalyzeProgress';

const AID = 'a-test-1';

describe('AnalyzeProgress (v0.15d)', () => {
  it('renders nothing when analysisId is null', () => {
    const { container } = render(<AnalyzeProgress analysisId={null} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when analysisId is set but no events have fired', () => {
    const { container } = render(<AnalyzeProgress analysisId={AID} />);
    // No started event yet → component is hidden
    expect(container.firstChild).toBeNull();
  });

  it('seeds a "pending" grid on the started event', async () => {
    render(<AnalyzeProgress analysisId={AID} />);
    await flushListeners();
    fireStarted({
      analysis_id: AID,
      market_id: 'm1',
      prompt_version: 'v1',
      providers: ['anthropic', 'openai', 'google'],
      started_at: 1000,
    });
    const grid = screen.getByTestId('analyze-progress');
    expect(grid).toBeInTheDocument();
    expect(grid).toHaveAttribute('data-running', 'true');
    for (const id of ['anthropic', 'openai', 'google']) {
      const row = screen.getByTestId(`analyze-progress-row-${id}`);
      expect(row).toHaveAttribute('data-status', 'pending');
    }
  });

  it('updates a provider to "ok" on provider_done with ok=true', async () => {
    render(<AnalyzeProgress analysisId={AID} />);
    await flushListeners();
    fireStarted({
      analysis_id: AID, market_id: 'm1', prompt_version: 'v1',
      providers: ['anthropic', 'openai'], started_at: 1000,
    });
    fireProviderDone({
      analysis_id: AID, provider_id: 'anthropic', ok: true,
      latency_ms: 1234, tokens_in: 100, tokens_out: 50, cost_cents: 0.04,
      error_kind: 'none', error_message: null, finished_at: 2000,
    });
    const row = screen.getByTestId('analyze-progress-row-anthropic');
    expect(row).toHaveAttribute('data-status', 'ok');
    expect(row.textContent).toContain('1.2s');
    const openaiRow = screen.getByTestId('analyze-progress-row-openai');
    expect(openaiRow).toHaveAttribute('data-status', 'pending');
  });

  it('updates a provider to "failed" with error_kind visible', async () => {
    render(<AnalyzeProgress analysisId={AID} />);
    await flushListeners();
    fireStarted({
      analysis_id: AID, market_id: 'm1', prompt_version: 'v1',
      providers: ['openai'], started_at: 1000,
    });
    fireProviderDone({
      analysis_id: AID, provider_id: 'openai', ok: false,
      latency_ms: 500, tokens_in: 0, tokens_out: 0, cost_cents: 0.0,
      error_kind: 'rate_limit', error_message: '429', finished_at: 2000,
    });
    const row = screen.getByTestId('analyze-progress-row-openai');
    expect(row).toHaveAttribute('data-status', 'failed');
    const errEl = screen.getByTestId('analyze-progress-error-openai');
    expect(errEl.textContent).toBe('rate_limit');
    expect(errEl.getAttribute('title')).toBe('429');
  });

  it('filters events from a different analysis_id', async () => {
    render(<AnalyzeProgress analysisId={AID} />);
    await flushListeners();
    fireStarted({
      analysis_id: AID, market_id: 'm1', prompt_version: 'v1',
      providers: ['anthropic'], started_at: 1000,
    });
    // Event for a *different* analysis_id — must be ignored
    fireProviderDone({
      analysis_id: 'a-OTHER', provider_id: 'anthropic', ok: true,
      latency_ms: 9999, tokens_in: 0, tokens_out: 0, cost_cents: 0.0,
      error_kind: 'none', error_message: null, finished_at: 2000,
    });
    const row = screen.getByTestId('analyze-progress-row-anthropic');
    expect(row).toHaveAttribute('data-status', 'pending');
  });

  it('marks grid as not-running and shows totals on finished event', async () => {
    render(<AnalyzeProgress analysisId={AID} />);
    await flushListeners();
    fireStarted({
      analysis_id: AID, market_id: 'm1', prompt_version: 'v1',
      providers: ['anthropic', 'openai'], started_at: 1000,
    });
    fireProviderDone({
      analysis_id: AID, provider_id: 'anthropic', ok: true,
      latency_ms: 1000, tokens_in: 50, tokens_out: 25, cost_cents: 0.02,
      error_kind: 'none', error_message: null, finished_at: 1500,
    });
    fireProviderDone({
      analysis_id: AID, provider_id: 'openai', ok: false,
      latency_ms: 800, tokens_in: 0, tokens_out: 0, cost_cents: 0.0,
      error_kind: 'network', error_message: 'timeout', finished_at: 1800,
    });
    fireConsensus({
      analysis_id: AID, status: 'partial', n_success: 1, n_failed: 1,
      consensus_pred: 0.62, consensus_side: 'YES', consensus_conf: 0.71,
    });
    fireFinished({
      analysis_id: AID, status: 'partial', total_latency_ms: 1800,
      total_cost_cents: 0.02, n_success: 1, n_failed: 1, finished_at: 2000,
    });
    const grid = screen.getByTestId('analyze-progress');
    expect(grid).toHaveAttribute('data-running', 'false');
    // Both providers have final state
    expect(screen.getByTestId('analyze-progress-row-anthropic')).toHaveAttribute('data-status', 'ok');
    expect(screen.getByTestId('analyze-progress-row-openai')).toHaveAttribute('data-status', 'failed');
  });

  it('clears state when analysisId becomes null', async () => {
    const { rerender, container } = render(<AnalyzeProgress analysisId={AID} />);
    await flushListeners();
    fireStarted({
      analysis_id: AID, market_id: 'm1', prompt_version: 'v1',
      providers: ['anthropic'], started_at: 1000,
    });
    expect(screen.getByTestId('analyze-progress')).toBeInTheDocument();
    rerender(<AnalyzeProgress analysisId={null} />);
    expect(container.firstChild).toBeNull();
  });
});
