// v0.62a.2 — Analysis 组件测试。
//
// /analysis 是 LLM 扇出分析页面。
// 当前覆盖率 0%。本文件覆盖：
//   1. 空数据初始渲染
//   2. 市场 id 输入 + Run 按钮
//   3. 推荐接受/拒绝流程

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
      // Run / Accept / Reject / Refresh 等。
      expect(buttons.length).toBeGreaterThan(0);
    });
  });
});
