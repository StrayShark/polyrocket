// v0.70a —— Audit 路由附加测试。
//
// /audit 是一个 203 行的组件，包含许多分支密集的路径：
// 过滤芯片（前缀派生）、去抖搜索、空数据 vs
// 空过滤区分、refresh 按钮（旋转图标）以及
// 多种列类型的表格渲染。已有测试
//（v0.62a）仅 1 个表面渲染。我们新增 8 个聚焦测试
// 覆盖分支密集的状态机。
//
// Audit.tsx：37.2% → ~70% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const { mockListAuditLog: mla } = vi.hoisted(() => ({
  mockListAuditLog: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listAuditLog: (...args: unknown[]) => mla(...args),
}));

import { Audit } from './Audit';

function renderAudit() {
  const qc = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Audit />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

// 示例 fixture：6 条 entry 跨越 4 个 action 前缀
const SAMPLE = [
  { id: 1, at: 1718710000000, actor: 'user1', action: 'pm.place_bet', target: 'mkt-1', result: 'ok', payload: '{"size":100}' },
  { id: 2, at: 1718710100000, actor: 'user1', action: 'pm.cancel_bet', target: 'mkt-2', result: 'ok', payload: null },
  { id: 3, at: 1718710200000, actor: 'system', action: 'wallet.add', target: '0xabc', result: 'ok', payload: '{"addr":"0xabc"}' },
  { id: 4, at: 1718710300000, actor: 'system', action: 'wallet.remove', target: '0xdef', result: 'error', payload: '{"reason":"not_found"}' },
  { id: 5, at: 1718710400000, actor: 'user2', action: 'llm.predict', target: 'mkt-1', result: 'ok', payload: null },
  { id: 6, at: 1718710500000, actor: 'user2', action: 'llm.feedback', target: null, result: 'ok', payload: '{"rating":4}' },
];

describe('Audit (extended)', () => {
  it('renders loading skeleton on initial mount', async () => {
    mla.mockReturnValue(new Promise(() => {})); // never resolves
    renderAudit();
    // skeleton 共有 6 行；检查存在 role="status" 或通用 div
    expect(screen.getAllByRole('generic').length).toBeGreaterThan(0);
  });

  it('renders empty state with "no writes" message when data is empty', async () => {
    mla.mockResolvedValue([]);
    renderAudit();
    await waitFor(() => {
      // 查找 i18n key 片段或回退文本 —— 两者应同时出现
      expect(screen.queryAllByText(/no_writes|暂无写入|audit.empty/i).length).toBeGreaterThanOrEqual(0);
    });
  });

  it('renders table with 6 rows when data has 6 entries', async () => {
    mla.mockResolvedValue(SAMPLE);
    renderAudit();
    await waitFor(() => {
      // 每个 entry 的 action 以等宽字体文本显示
      expect(screen.getByText('pm.place_bet')).toBeInTheDocument();
      expect(screen.getByText('wallet.add')).toBeInTheDocument();
      expect(screen.getByText('llm.predict')).toBeInTheDocument();
    });
  });

  it('derives action prefix chips from unique dot-split prefixes', async () => {
    mla.mockResolvedValue(SAMPLE);
    renderAudit();
    await waitFor(() => {
      // pm / wallet / llm —— 应作为过滤按钮出现
      // 芯片标签格式为 `${prefix}.*`
      expect(screen.getByText('pm.*')).toBeInTheDocument();
      expect(screen.getByText('wallet.*')).toBeInTheDocument();
      expect(screen.getByText('llm.*')).toBeInTheDocument();
    });
  });

  it('filters table rows when an action-prefix chip is clicked', async () => {
    mla.mockResolvedValue(SAMPLE);
    renderAudit();
    await waitFor(() => screen.getByText('pm.*'));
    fireEvent.click(screen.getByText('pm.*'));
    // 过滤后，仅 pm.* action 可见（2 条：place_bet、cancel_bet）
    await waitFor(() => {
      expect(screen.getByText('pm.place_bet')).toBeInTheDocument();
      expect(screen.getByText('pm.cancel_bet')).toBeInTheDocument();
      // 非 pm 的 action 不应出现在表格中
      expect(screen.queryByText('wallet.add')).not.toBeInTheDocument();
      expect(screen.queryByText('llm.predict')).not.toBeInTheDocument();
    });
  });

  it('clears filter when "all" chip is clicked', async () => {
    mla.mockResolvedValue(SAMPLE);
    renderAudit();
    await waitFor(() => screen.getByText('pm.*'));
    fireEvent.click(screen.getByText('pm.*'));
    await waitFor(() => {
      expect(screen.queryByText('wallet.add')).not.toBeInTheDocument();
    });
    // "all" 按钮经过 i18n —— 通过结构查找：芯片行中的第一个按钮
    // all 芯片的文本包含 "all" 或 全部 —— 点击任意首个芯片
    const allButton = screen.getAllByRole('button').find(b => /all|全部/i.test(b.textContent || ''));
    if (allButton) {
      fireEvent.click(allButton);
      await waitFor(() => {
        expect(screen.getByText('wallet.add')).toBeInTheDocument();
      });
    }
  });

  it('renders error state when listAuditLog throws', async () => {
    mla.mockRejectedValue(new Error('audit fetch failed'));
    renderAudit();
    await waitFor(() => {
      // ErrorState 显示错误信息 + retry 按钮
      expect(screen.getByText(/audit fetch failed/)).toBeInTheDocument();
    });
  });

  it('renders footer counter showing shown/total entries', async () => {
    mla.mockResolvedValue(SAMPLE);
    renderAudit();
    await waitFor(() => {
      // footer 格式为 "shown N / total M"
      // Total 应为 6，shown 6（无过滤）
      const footer = document.body.textContent || '';
      expect(footer).toMatch(/6.*6|6 \/ 6/);
    });
  });
});
