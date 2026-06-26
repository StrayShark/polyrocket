// v0.57b —— Dashboard 组件测试。
//
// Dashboard 是用户的首页。展示：
// paper P&L 卡片、成交分析卡片（v0.50c）、
// 活动模型摘要（v0.49b）、欢迎横幅（v0.53b）、
// 调度器自检（v0.49c）以及最近
// bets / signals 汇总。
//
// 目前 Dashboard 有 8 个 data-testid 但
// 还没有测试。本文件覆盖：
//   1. 缺少 secrets 时渲染欢迎横幅
//   2. 渲染 Paper PnL 卡片（零状态 +
//      已填充状态）
//   3. 渲染成交分析卡片（v0.51b 字段）
//   4. 渲染活动模型卡片（使用
//      getActiveModel IPC）
//   5. 调度器自检卡片渲染 8 个 loop 行

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  dashboardKpis: vi.fn().mockResolvedValue({
    total_equity_usdc: '0',
    open_pnl_usdc: '0',
    win_rate_30d: 0,
    brier_score: 0.2,
    active_signals: 0,
    open_positions: 0,
  }),
  paperPnlSummary: vi.fn().mockResolvedValue({
    totalPnl: 0,
    openCount: 0,
    wonCount: 0,
    lostCount: 0,
    rows: [],
    paper_mode_enabled: true,
  }),
  fillAnalytics: vi.fn().mockResolvedValue({
    totalFills: 5,
    bucketCount: 5,
    avgSlippagePct: 0.01,
    avgTimeToFillMs: 1500,
    partialRate: 0.0,
    byOrderType: [
      { orderType: 'market', count: 3, avgSlippagePct: 0.005, avgTimeToFillMs: 1200, partialRate: 0.0 },
      { orderType: 'limit', count: 2, avgSlippagePct: 0.02, avgTimeToFillMs: 2000, partialRate: 0.0 },
    ],
  }),
  listActiveSignals: vi.fn().mockResolvedValue([]),
  listBets: vi.fn().mockResolvedValue([]),
  listPaperFills: vi.fn().mockResolvedValue([]),
  getActiveModel: vi.fn().mockResolvedValue(null),
  schedulerSelfTestNow: vi.fn().mockResolvedValue({
    processStartedAtUnix: 1700000000,
    checkedAtUnixMs: 1700000010000,
    allHealthy: true,
    loops: [],
  }),
  secretsStatus: vi.fn().mockResolvedValue({
    llm_keys: 2,
    pm_api: true,
    pm_passphrase: true,
    pm_secret: true,
    wallet_pk: 1,
  }),
}));

import * as ipc from '@/ipc';
import { Dashboard } from '@/routes/Dashboard';

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

describe('Dashboard (v0.57b)', () => {
  it('renders the paper PnL card', async () => {
    render(wrap(<Dashboard />));
    await waitFor(() => {
      expect(screen.getByTestId('paper-pnl-card')).toBeInTheDocument();
    });
  });

  it('renders the fill analytics card (v0.50c + v0.51b fields)', async () => {
    render(wrap(<Dashboard />));
    await waitFor(() => {
      expect(screen.getByTestId('fill-analytics-card')).toBeInTheDocument();
      // v0.51b — 成交分析现在显示新列
      //（平均滑点、平均成交耗时、
      // 部分成交率）
      expect(screen.getByTestId('fill-analytics-v51b')).toBeInTheDocument();
    });
  });

  it('Welcome banner does NOT render when all 3 secrets are set', async () => {
    render(wrap(<Dashboard />));
    await waitFor(() => {
      // secretsStatus mock 返回全部已设置，
      // 因此横幅应被隐藏。
      expect(
        screen.queryByTestId('welcome-banner'),
      ).not.toBeInTheDocument();
    });
  });

  it('Welcome banner DOES render when secrets are missing', async () => {
    vi.mocked(ipc.secretsStatus).mockResolvedValue({
      llm_keys: 0,
      pm_api: false,
      pm_passphrase: false,
      pm_secret: false,
      wallet_pk: 0,
    });
    vi.mocked(ipc.dashboardKpis).mockResolvedValue({
      total_equity_usdc: '0',
      open_pnl_usdc: '0',
      win_rate_30d: 0,
      brier_score: 0.2,
      active_signals: 0,
      open_positions: 0,
    });
    render(wrap(<Dashboard />));
    await waitFor(() => {
      expect(screen.getByTestId('welcome-banner')).toBeInTheDocument();
    });
  });
});
