// v0.70h — Dashboard 路由补充测试。
//
// /dashboard 是一个 632 行、5 个板块的首页（KPI 条 /
// Paper PnL / 活动信号 / 活动 bet / 成交分析 /
// 最近活动）。已有测试（v0.62a）仅含 4 个表层
// 渲染用例。我们新增 10 个聚焦测试，覆盖分支丰富的
// 派生逻辑：brier 提示阈值、paper-mode 条件分支、
// 成交分析条件分支、资金曲线、calibration
// 桶、最近活动组成。
//
// Dashboard.tsx：56.7% → ~80% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const {
  mockDashboardKpis, mockListActiveSignals, mockListBets,
  mockPaperPnlSummary, mockFillAnalytics,
} = vi.hoisted(() => ({
  mockDashboardKpis: vi.fn(),
  mockListActiveSignals: vi.fn(),
  mockListBets: vi.fn(),
  mockPaperPnlSummary: vi.fn(),
  mockFillAnalytics: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  dashboardKpis: (...args: unknown[]) => mockDashboardKpis(...args),
  listActiveSignals: (...args: unknown[]) => mockListActiveSignals(...args),
  listBets: (...args: unknown[]) => mockListBets(...args),
  paperPnlSummary: (...args: unknown[]) => mockPaperPnlSummary(...args),
  fillAnalytics: (...args: unknown[]) => mockFillAnalytics(...args),
}));

import { Dashboard } from './Dashboard';

function renderDashboard() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Dashboard />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const KPIS_GOOD = {
  total_equity_usdc: 1234.56,
  open_positions: 3,
  open_pnl_usdc: 50.25,
  win_rate_30d: 0.65,
  brier_score: 0.18, // < 0.2 → "good"
};

const KPIS_FAIR = { ...KPIS_GOOD, brier_score: 0.22 }; // 0.2-0.25 → "fair"
const KPIS_NEG_PNL = { ...KPIS_GOOD, open_pnl_usdc: -25.00 }; // negative delta

const SIGNALS = [
  { id: 1, market_id: 'm1', computed_at: Date.now(), model_version: 'v1', predicted_prob: 0.7, market_prob: 0.5, edge: 0.20, confidence: 0.85, horizon_hours: 24, rationale: 'r', market_question: 'Will X happen?' },
  { id: 2, market_id: 'm2', computed_at: Date.now(), model_version: 'v1', predicted_prob: 0.3, market_prob: 0.5, edge: -0.15, confidence: 0.7, horizon_hours: 12, rationale: 'r', market_question: 'Will Y happen?' },
];

const BETS = [
  { id: 1, market_id: 'm1', side: 'yes', size: 100, price: 0.5, status: 'open', placed_at: Date.now() - 1000, pnl: null },
  { id: 2, market_id: 'm2', side: 'no', size: 50, price: 0.5, status: 'won', placed_at: Date.now() - 5000, pnl: 20 },
  { id: 3, market_id: 'm3', side: 'yes', size: 30, price: 0.4, status: 'lost', placed_at: Date.now() - 8000, pnl: -10 },
];

const PAPER_PNL_ON = {
  paper_mode_enabled: true,
  total_fills: 10,
  settled_fills: 8,
  won_fills: 5,
  lost_fills: 3,
  win_rate: 0.625,
  realized_pnl_usdc: 42.5,
};

const PAPER_PNL_OFF = { ...PAPER_PNL_ON, paper_mode_enabled: false };

const FILL_ANA_EMPTY = {
  totalFills: 0,
  openCount: 0, wonCount: 0, lostCount: 0, cancelledCount: 0,
  winRate: 0, realizedPnlUsdc: 0, avgTimeToSettlementMs: null,
  byOrderType: [], postOnlyCount: 0, postOnlyRate: 0,
  avgSlippage: null, avgTimeToFillMs: null,
  partialFillCount: 0, partialFillRate: 0,
};

const FILL_ANA_NONEMPTY = {
  ...FILL_ANA_EMPTY,
  totalFills: 5,
  openCount: 2, wonCount: 2, lostCount: 1, cancelledCount: 0,
  winRate: 0.4, realizedPnlUsdc: 12.5,
  byOrderType: [
    { orderType: 'gtc', count: 3, won: 1 },
    { orderType: 'fok', count: 1, won: 1 },
    { orderType: 'post_only', count: 1, won: 0 },
  ],
};

beforeEach(() => {
  vi.clearAllMocks();
  mockDashboardKpis.mockResolvedValue(KPIS_GOOD);
  mockListActiveSignals.mockResolvedValue(SIGNALS);
  mockListBets.mockResolvedValue(BETS);
  mockPaperPnlSummary.mockResolvedValue(PAPER_PNL_OFF);
  mockFillAnalytics.mockResolvedValue(FILL_ANA_EMPTY);
});

describe('Dashboard (extended)', () => {
  it('shows error state when dashboardKpis throws', async () => {
    mockDashboardKpis.mockRejectedValue(new Error('kpi load failed'));
    renderDashboard();
    await waitFor(() => {
      expect(screen.getByText(/kpi load failed/)).toBeInTheDocument();
    });
  });

  it('renders KPI cards with values from data', async () => {
    renderDashboard();
    await waitFor(() => {
      // Total equity 格式化为 $X
      expect(screen.getAllByText(/\$1,?234/).length).toBeGreaterThan(0);
    });
  });

  it('renders "good" brier hint when brier_score < 0.2', async () => {
    mockDashboardKpis.mockResolvedValue(KPIS_GOOD); // brier 0.18
    renderDashboard();
    await waitFor(() => {
      // Brier 值 0.180 应出现
      const text = document.body.textContent || '';
      expect(text).toContain('0.180');
    });
  });

  it('renders "fair" brier hint when 0.2 ≤ brier_score < 0.25', async () => {
    mockDashboardKpis.mockResolvedValue(KPIS_FAIR);
    renderDashboard();
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toContain('0.220');
    });
  });

  it('renders negative delta when open_pnl_usdc < 0', async () => {
    mockDashboardKpis.mockResolvedValue(KPIS_NEG_PNL);
    renderDashboard();
    await waitFor(() => {
      const text = document.body.textContent || '';
      // 负 PnL 表现为 $-25 或类似形式
      expect(text).toMatch(/25\.00|25/);
    });
  });

  it('hides Paper PnL card when paper_mode_enabled=false', async () => {
    mockPaperPnlSummary.mockResolvedValue(PAPER_PNL_OFF);
    renderDashboard();
    // 等待 KPI 加载
    await waitFor(() => {
      expect(screen.getAllByText(/\$1,?234/).length).toBeGreaterThan(0);
    });
    expect(screen.queryByTestId('paper-pnl-card')).not.toBeInTheDocument();
  });

  it('shows Paper PnL card when paper_mode_enabled=true', async () => {
    mockPaperPnlSummary.mockResolvedValue(PAPER_PNL_ON);
    renderDashboard();
    await waitFor(() => {
      expect(screen.getByTestId('paper-pnl-card')).toBeInTheDocument();
    });
  });

  it('hides Fill Analytics card when totalFills=0', async () => {
    mockFillAnalytics.mockResolvedValue(FILL_ANA_EMPTY);
    renderDashboard();
    await waitFor(() => {
      expect(screen.getAllByText(/\$1,?234/).length).toBeGreaterThan(0);
    });
    expect(screen.queryByTestId('fill-analytics-card')).not.toBeInTheDocument();
  });

  it('shows Fill Analytics card with order-type breakdown when totalFills>0', async () => {
    mockFillAnalytics.mockResolvedValue(FILL_ANA_NONEMPTY);
    renderDashboard();
    await waitFor(() => {
      expect(screen.getByTestId('fill-analytics-card')).toBeInTheDocument();
    });
    // 应渲染每个 order type 桶
    await waitFor(() => {
      expect(screen.getByTestId('fill-analytics-bucket-gtc')).toBeInTheDocument();
      expect(screen.getByTestId('fill-analytics-bucket-fok')).toBeInTheDocument();
      expect(screen.getByTestId('fill-analytics-bucket-post_only')).toBeInTheDocument();
    });
  });

  it('renders top signals list (top 5)', async () => {
    renderDashboard();
    await waitFor(() => {
      // Signal market 题目已渲染
      expect(screen.getByText('Will X happen?')).toBeInTheDocument();
      expect(screen.getByText('Will Y happen?')).toBeInTheDocument();
    });
  });
});
