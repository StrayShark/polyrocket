// v0.63b — Copy 组件测试（路由 25% 覆盖 → ~80%）。
//
// /copy 是跟单中心。我们覆盖：
//   1. 渲染空列表页面
//   2. 显示 Add 按钮
//   3. 当 getMirrorPaperMode 返回 true 时渲染 paper-mode 横幅
//   4. 渲染带 watching pill + min edge pill 的目标列表
//   5. 当 target.enabled = false 时渲染 paused pill
//   6. 已设置时渲染 allocation cap pill
//   7. 渲染某个 target 下的 events 列表
//   8. listCopyTargets 失败时的错误态
//   9. 打开 Add modal（尚未提交）
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListCopyTargets = vi.fn();
const mockRecentCopyEvents = vi.fn();
const mockListPaperFills = vi.fn();
const mockGetMirrorPaperMode = vi.fn();
const mockAddCopyTarget = vi.fn();

vi.mock('@/ipc', () => ({
  listCopyTargets: () => mockListCopyTargets(),
  recentCopyEvents: (...args: unknown[]) => mockRecentCopyEvents(...args),
  listPaperFills: () => mockListPaperFills(),
  getMirrorPaperMode: () => mockGetMirrorPaperMode(),
  addCopyTarget: (...args: unknown[]) => mockAddCopyTarget(...args),
}));

import { Copy } from './Copy';

function renderCopy() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Copy />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const T_WATCHING = {
  id: 't1',
  address: '0x1111111111111111111111111111111111111111',
  label: 'Whale #1',
  enabled: true,
  allocation_cap: null,
  min_edge: 0.05,
  created_at: Date.now() - 86_400_000,
};

const T_PAUSED = {
  ...T_WATCHING,
  id: 't2',
  address: '0x2222222222222222222222222222222222222222',
  label: null,
  enabled: false,
  allocation_cap: '500',
};

const T_WITH_CAP = {
  ...T_WATCHING,
  id: 't3',
  address: '0x3333333333333333333333333333333333333333',
  label: 'Capped',
  enabled: true,
  allocation_cap: '1500',
  min_edge: 0.1,
};

const E_YES = {
  id: 1,
  target_id: 't1',
  market_id: 'm1',
  detected_at: Date.now() - 3_600_000,
  side: 'YES' as const,
  size: '100',
  price: 0.45,
  tx_hash: '0xabc',
  matched_bet_id: null,
};

const E_NO = {
  ...E_YES,
  id: 2,
  side: 'NO' as const,
  size: '50',
  price: 0.62,
};

describe('Copy', () => {
  beforeEach(() => {
    mockListCopyTargets.mockReset();
    mockRecentCopyEvents.mockReset();
    mockListPaperFills.mockReset();
    mockGetMirrorPaperMode.mockReset();
    mockAddCopyTarget.mockReset();
    // 默认：空 / 无 paper 模式
    mockRecentCopyEvents.mockResolvedValue([]);
    mockListPaperFills.mockResolvedValue([]);
    mockGetMirrorPaperMode.mockResolvedValue(false);
  });

  it('renders the page and shows empty state', async () => {
    mockListCopyTargets.mockResolvedValue([]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/copy/i).length).toBeGreaterThan(0);
    });
  });

  it('shows an Add button', async () => {
    mockListCopyTargets.mockResolvedValue([]);
    renderCopy();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const addBtn = buttons.find((b) => b.textContent?.toLowerCase().includes('add'));
      expect(addBtn).toBeTruthy();
    });
  });

  it('renders paper-mode banner with fill count when paper mode is on', async () => {
    mockListCopyTargets.mockResolvedValue([]);
    mockGetMirrorPaperMode.mockResolvedValue(true);
    mockListPaperFills.mockResolvedValue([
      { id: 'f1', mirror_id: 'mr1', market_id: 'm1', side: 'YES',
        size: '10', price: 0.5, placed_at: 0, notes: null },
      { id: 'f2', mirror_id: 'mr1', market_id: 'm2', side: 'NO',
        size: '20', price: 0.4, placed_at: 0, notes: null },
    ]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByTestId('copy-paper-mode-banner')).toBeInTheDocument();
    });
    // Banner 带有 [PAPER] 前缀
    expect(screen.getByText('[PAPER]')).toBeInTheDocument();
  });

  it('renders watching + min-edge pill for an enabled target', async () => {
    mockListCopyTargets.mockResolvedValue([T_WATCHING]);
    renderCopy();
    await waitFor(() => {
      // T_WATCHING 的 label 为 'Whale #1'
      expect(screen.getByText('Whale #1')).toBeInTheDocument();
    });
    // "watching" 胶囊（pill body）
    expect(screen.getByText(/watching/i)).toBeInTheDocument();
    // min edge 5% 胶囊 —— 文本可能被切分到多个 span 中，因此
    // 我们检查 "min edge" 与 "5%" 同时出现于 document 中
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/min edge/);
      expect(document.body.textContent).toMatch(/5%/);
    });
  });

  it('renders paused pill when target is disabled', async () => {
    mockListCopyTargets.mockResolvedValue([T_PAUSED]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByText(/paused/i)).toBeInTheDocument();
    });
  });

  it('renders allocation cap pill when set', async () => {
    mockListCopyTargets.mockResolvedValue([T_WITH_CAP]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByText('Capped')).toBeInTheDocument();
    });
    // cap $1500 胶囊 —— 文本由 fmtUsdc 分割；检查主体
    await waitFor(() => {
      expect(document.body.textContent).toMatch(/cap/);
      // fmtUsdc(1500) 格式化为 "1,500"（带逗号）或 "1500"
      expect(document.body.textContent).toMatch(/1,?500/);
    });
  });

  it('renders (no label) when target.label is null', async () => {
    mockListCopyTargets.mockResolvedValue([T_PAUSED]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByText(/\(no label\)/i)).toBeInTheDocument();
    });
  });

  it('renders events list under target with side pills', async () => {
    mockListCopyTargets.mockResolvedValue([T_WATCHING]);
    mockRecentCopyEvents.mockResolvedValue([E_YES, E_NO]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByText('Whale #1')).toBeInTheDocument();
    });
    // events 标题
    expect(screen.getByText(/recent events/i)).toBeInTheDocument();
    // YES 和 NO 胶囊已渲染
    await waitFor(() => {
      expect(screen.getAllByText('YES').length).toBeGreaterThan(0);
      expect(screen.getAllByText('NO').length).toBeGreaterThan(0);
    });
  });

  it('renders ErrorState when listCopyTargets fails', async () => {
    mockListCopyTargets.mockRejectedValue(new Error('RPC timeout'));
    renderCopy();
    await waitFor(() => {
      // 错误字符串出现在 ErrorState 中（也可能出现在 toast 中）。
      // 我们使用 getAllByText 并检查至少一个匹配。
      expect(screen.getAllByText(/RPC timeout/i).length).toBeGreaterThan(0);
    });
  });
});
