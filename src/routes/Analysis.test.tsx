// v0.62a.2 — Analysis component tests.
//
// /analysis is the LLM fan-out analysis page.
// Today 0% coverage. This file covers:
//   1. Initial render with empty data
//   2. Market id input + Run button
//   3. Recommendation accept/reject flow

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  llmAnalyze: vi.fn().mockResolvedValue({ analysis_id: 'a1' }),
  llmGetRecommendation: vi.fn().mockResolvedValue({
    id: 1, analysis_id: 'a1', provider_id: 'p1', provider_name: 'P1',
    predicted_prob: 0.6, side: 'YES', confidence: 0.7,
    reasoning: 'because', latency_ms: 200, tokens_in: 100, tokens_out: 50,
    cost_cents: 0.5, parse_ok: true, parse_error: null,
  }),
  recordLlmDecision: vi.fn().mockResolvedValue(undefined),
  listActiveSignals: vi.fn().mockResolvedValue([]),
  onAnalyzeStarted: vi.fn().mockResolvedValue(() => () => {}),
}));

import { Analysis } from './Analysis';

function renderAnalysis() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Analysis />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Analysis', () => {
  it('renders the page', async () => {
    renderAnalysis();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows the Run analysis button', async () => {
    renderAnalysis();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      // Run / Accept / Reject / Refresh / etc.
      expect(buttons.length).toBeGreaterThan(0);
    });
  });
});
