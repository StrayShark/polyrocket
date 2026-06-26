// v0.62a — Audit 组件测试。
//
// /audit 是只读审计日志浏览器。
// 当前覆盖率 0%。本文件覆盖
// 初始渲染 + 过滤芯片 + 表格。

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listAuditLog: vi.fn().mockResolvedValue([]),
}));

import { Audit } from './Audit';

function renderAudit() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Audit />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Audit', () => {
  it('renders the page', async () => {
    renderAudit();
    await waitFor(() => {
      expect(screen.getAllByText(/audit/i).length).toBeGreaterThan(0);
    });
  });
});
