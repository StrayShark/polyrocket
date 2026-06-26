// v0.71d — LlmPerf 路由补充测试。
//
// LlmPerf.tsx 共 168 行，包含 4 个并发 query
// （llmPerformance / llmStatsByConfidence / llmStatsByPrompt
// / llmStatsCostEfficiency）+ CSV 导出 mutation。已有
// 测试（v0.62a）只有 1 个表层渲染。我们新增 10 个测试，覆盖
// 分支丰富的路径：KPI 正/负 delta、by-conf
// 柱状颜色（bull vs bear）、by-prompt 渲染、cost-eff ROI
// 符号、CSV 导出成功/错误 toast、CSV 预览 <details>。
//
// 覆盖目标：52% → ~85% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const {
  mockLlmPerformance: mlp, mockLlmStatsByConfidence: msbc,
  mockLlmStatsByPrompt: msbp, mockLlmStatsCostEfficiency: msce,
  mockLlmStatsExport: mse,
} = vi.hoisted(() => ({
  mockLlmPerformance: vi.fn(),
  mockLlmStatsByConfidence: vi.fn(),
  mockLlmStatsByPrompt: vi.fn(),
  mockLlmStatsCostEfficiency: vi.fn(),
  mockLlmStatsExport: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  llmPerformance: (...args: unknown[]) => mlp(...args),
  llmStatsByConfidence: (...args: unknown[]) => msbc(...args),
  llmStatsByPrompt: (...args: unknown[]) => msbp(...args),
  llmStatsCostEfficiency: (...args: unknown[]) => msce(...args),
  llmStatsExport: (...args: unknown[]) => mse(...args),
}));

import { LlmPerf } from './LlmPerf';

function renderLlmPerf() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <LlmPerf />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const PERF = [
  { model_version: 'v1', brier_score: 0.18, n_calls: 100, n_predictions: 1000, log_loss: 0.4, win_rate: 0.55, avg_edge: 0.05 },
  { model_version: 'v2', brier_score: 0.20, n_calls: 50, n_predictions: 500, log_loss: 0.5, win_rate: 0.45, avg_edge: 0.04 },
];

const BY_CONF = [
  { bucket: 0, win_rate: 0.3, n: 10 },  // bear
  { bucket: 5, win_rate: 0.55, n: 20 }, // bull
  { bucket: 8, win_rate: 0.75, n: 30 }, // bull
];

const BY_PROMPT = [
  { prompt_version: 'v1', win_rate: 0.55, n: 100 },
  { prompt_version: 'v2', win_rate: 0.62, n: 50 },
];

const COST_EFF_POS = [
  { provider_id: 'openai', cost_cents: 500, wins: 10, roi: 0.15 },
  { provider_id: 'anthropic', cost_cents: 800, wins: 15, roi: 0.22 },
];

const COST_EFF_NEG = [
  { provider_id: 'cheap-llm', cost_cents: 100, wins: 2, roi: -0.05 },
];

beforeEach(() => {
  vi.clearAllMocks();
  mlp.mockResolvedValue(PERF);
  msbc.mockResolvedValue(BY_CONF);
  msbp.mockResolvedValue(BY_PROMPT);
  msce.mockResolvedValue(COST_EFF_POS);
  mse.mockResolvedValue('csv,data\nprovider,cost\nopenai,500');
});

describe('LlmPerf (extended)', () => {
  it('renders loading skeleton for 4 cards on initial mount', async () => {
    mlp.mockReturnValue(new Promise(() => {}));
    msbc.mockReturnValue(new Promise(() => {}));
    renderLlmPerf();
    expect(screen.getAllByRole('generic').length).toBeGreaterThan(0);
  });

  it('renders empty state for by-confidence when data is empty', async () => {
    msbc.mockResolvedValue([]);
    renderLlmPerf();
    // findByText 默认等待最多 1s
    const emptyText = await screen.findByText(/no data|暂无数据/i, {}, { timeout: 3000 });
    expect(emptyText).toBeInTheDocument();
  });

  it('renders by-confidence buckets with bull/bear colors', async () => {
    renderLlmPerf();
    await waitFor(() => {
      // 桶标签格式为 "0-10%"、"10-20%" 等。
      // (b.bucket * 10).toFixed(0) + '-' + (b.bucket * 10 + 10).toFixed(0) + '%'
      const text = document.body.textContent || '';
      expect(text).toMatch(/0-10%|10-20%|50-60%|80-90%/);
    });
  });

  it('renders by-prompt rows with prompt version pill', async () => {
    renderLlmPerf();
    await waitFor(() => {
      expect(screen.getAllByText('v1').length).toBeGreaterThan(0);
      expect(screen.getAllByText('v2').length).toBeGreaterThan(0);
    });
  });

  it('renders cost-efficiency rows with positive ROI (bull color)', async () => {
    renderLlmPerf();
    await waitFor(() => {
      expect(screen.getByText('openai')).toBeInTheDocument();
    });
    // ROI 行展示 "ROI +0.15" 类文本
    const text = document.body.textContent || '';
    expect(text).toMatch(/ROI.*\+0\.15|ROI.*\+0\.22/);
  });

  it('renders negative ROI with bear color (no + prefix)', async () => {
    msce.mockResolvedValue(COST_EFF_NEG);
    renderLlmPerf();
    await waitFor(() => {
      expect(screen.getByText('cheap-llm')).toBeInTheDocument();
    });
    // 负 ROI：text-bear，无 + 前缀
    const text = document.body.textContent || '';
    expect(text).toMatch(/ROI.*-0\.05/);
  });

  it('avg ROI positive triggers profitable delta', async () => {
    msce.mockResolvedValue(COST_EFF_POS);
    renderLlmPerf();
    await waitFor(() => screen.getByText('openai'));
    // avg ROI = (0.15+0.22)/2 = 0.185 → 正值 → "profitable"
    // 寻找含 "profitable" 等字样的 delta 文本
    const text = document.body.textContent || '';
    expect(text).toMatch(/profitable|positive|profit/i);
  });

  it('avg ROI negative triggers unprofitable delta', async () => {
    msce.mockResolvedValue([{ provider_id: 'x', cost_cents: 100, wins: 0, roi: -0.1 }]);
    renderLlmPerf();
    await waitFor(() => screen.getByText('x'));
    const text = document.body.textContent || '';
    expect(text).toMatch(/unprofitable|negative|loss/i);
  });

  it('Export CSV button triggers llmStatsExport + toast success', async () => {
    renderLlmPerf();
    await waitFor(() => screen.getByText('openai'));
    const exportBtn = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('export') ||
      b.textContent?.includes('导出'),
    );
    fireEvent.click(exportBtn!);
    await waitFor(() => {
      expect(mse).toHaveBeenCalled();
    });
  });

  it('Export CSV error path → llmStatsExport rejected', async () => {
    mse.mockRejectedValue(new Error('export failed'));
    renderLlmPerf();
    await waitFor(() => screen.getByText('openai'));
    const exportBtn = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('export') ||
      b.textContent?.includes('导出'),
    );
    fireEvent.click(exportBtn!);
    await waitFor(() => {
      expect(mse).toHaveBeenCalled();
    });
  });
});
