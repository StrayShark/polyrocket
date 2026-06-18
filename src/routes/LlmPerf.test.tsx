// v0.62a — LlmPerf component tests.
//
// /llm-perf is the LLM performance analytics
// page. Today 0% coverage.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  llmPerformance: vi.fn().mockResolvedValue([]),
  llmStatsByConfidence: vi.fn().mockResolvedValue([]),
  llmStatsByPrompt: vi.fn().mockResolvedValue([]),
  llmStatsCostEfficiency: vi.fn().mockResolvedValue([]),
  llmStatsExport: vi.fn().mockResolvedValue(''),
}));

import { LlmPerf } from './LlmPerf';

function renderLlmPerf() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <LlmPerf />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('LlmPerf', () => {
  it('renders the page', async () => {
    renderLlmPerf();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
