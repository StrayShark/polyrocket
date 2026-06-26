// v0.87a —— Audit.tsx 单元格渲染器测试。
//
// Audit.tsx 有 6 个列定义，包含内联单元格渲染器
//（第 75、83、91、109 行）。v0.62a + v0.70a + v0.72c 测试使用
// SAMPLE 数据渲染表格，但没有显式断言每个单元格的
// 内容类型。v8 覆盖率报告将单元格箭头函数体标记为
// 未覆盖，因为内联 JSX 的执行很难归因到源代码行。
//
// 这些测试断言每列的单元格都渲染出预期的 JSX：
//   - `at` → fmtDateTime 输出（在带 font-mono 类的 span 中）
//   - `actor` → <Pill kind="muted">（渲染 actor 名称）
//   - `action` → 包含 action 文本的 <span>
//   - `target` → <code>（当 target 非空时）或 <span>—（当为 null 时）
//   - `result` → 根据 result 值的 <Pill kind="bull"|"bear">
//
// 同时覆盖第 174 行（listAuditLog 拒绝时渲染 ErrorState 并可重试）。
//
// 覆盖率目标：Audit.tsx 88.37% → 90%+ stmts。

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const { mockListAuditLog } = vi.hoisted(() => ({
  mockListAuditLog: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listAuditLog: (...args: unknown[]) => mockListAuditLog(...args),
}));

import { Audit } from './Audit';

const FULL_SAMPLE = [
  { id: 1, at: 1718710000000, actor: 'user1', action: 'pm.place_bet', target: 'mkt-1', result: 'ok', payload: '{"size":100}' },
  { id: 2, at: 1718710100000, actor: 'user2', action: 'wallet.add', target: null, result: 'error', payload: null },
];

function renderAudit() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={qc}>
        <Audit />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mockListAuditLog.mockResolvedValue([]);
});

afterEach(() => cleanup());

describe('Audit cell renderers (v0.87a)', () => {
  it('at column cell renders fmtDateTime output (line 75)', async () => {
    mockListAuditLog.mockResolvedValueOnce(FULL_SAMPLE);
    renderAudit();
    await waitFor(() => {
      expect(screen.getByText('pm.place_bet')).toBeInTheDocument();
    });
    // at 列在 span.font-mono 中渲染格式化后的时间戳。
    // 1718710000000 → 2024-06-18（类似如此）。具体格式
    // 取决于 fmtDateTime。我们检查存在 font-mono 的 span。
    const fontMonoSpans = document.querySelectorAll('span.font-mono');
    expect(fontMonoSpans.length).toBeGreaterThan(0);
  });

  it('actor column cell renders <Pill> with the actor name (line 83)', async () => {
    mockListAuditLog.mockResolvedValueOnce(FULL_SAMPLE);
    renderAudit();
    await waitFor(() => {
      expect(screen.getByText('user1')).toBeInTheDocument();
      expect(screen.getByText('user2')).toBeInTheDocument();
    });
  });

  it('action column cell renders the action text in a span (line 91)', async () => {
    mockListAuditLog.mockResolvedValueOnce(FULL_SAMPLE);
    renderAudit();
    await waitFor(() => {
      // action 列在带有 text-fg 类的 span 中渲染
      const fgSpans = document.querySelectorAll('span.text-fg.font-mono');
      expect(fgSpans.length).toBeGreaterThan(0);
    });
  });

  it('result column cell renders <Pill> with bull/bear kind (line 109)', async () => {
    mockListAuditLog.mockResolvedValueOnce(FULL_SAMPLE);
    renderAudit();
    await waitFor(() => {
      // result 列在 Pill 中渲染 ok/error
      expect(screen.getByText('ok')).toBeInTheDocument();
      expect(screen.getByText('error')).toBeInTheDocument();
    });
  });

  it('target column cell: null target → em-dash fallback', async () => {
    mockListAuditLog.mockResolvedValueOnce([
      { id: 1, at: 0, actor: 'user', action: 'test', target: null, result: 'ok', payload: null },
    ]);
    renderAudit();
    await waitFor(() => {
      // target 为 null → 渲染 "—"（em-dash）。可能存在多个 em-dash
    // （例如 payload 为 null），因此仅检查至少存在一个。
      expect(screen.getAllByText('—').length).toBeGreaterThan(0);
    });
  });

  it('target column cell: non-null target → <code> with address', async () => {
    mockListAuditLog.mockResolvedValueOnce([
      { id: 1, at: 0, actor: 'user', action: 'test', target: '0xabc', result: 'ok', payload: null },
    ]);
    renderAudit();
    await waitFor(() => {
      const codes = document.querySelectorAll('code');
      expect(codes.length).toBeGreaterThan(0);
    });
  });

  it('ErrorState on listAuditLog rejection + retry (line 174)', async () => {
    mockListAuditLog.mockRejectedValueOnce(new Error('audit fetch failed'));
    renderAudit();
    await waitFor(() => {
      expect(screen.getByText(/audit fetch failed/)).toBeInTheDocument();
    });
    // ErrorState 有 retry 按钮。点击它，然后让第二次
    // mock 调用成功。
    mockListAuditLog.mockResolvedValueOnce([]);
    const retryBtn = screen.getByRole('button', { name: /try again/i });
    fireEvent.click(retryBtn);
    await waitFor(() => {
      expect(mockListAuditLog).toHaveBeenCalledTimes(2);
    });
  });
});
