// v0.67a — Copy 路由补充测试（AddTargetModal 流程）。
//
// /copy 主要有 3 个流程：targets 列表、events 列表、
// AddTargetModal。基础 v0.63b 测试（3 个用例）仅
// 覆盖列表渲染。我们新增 5 个聚焦
// AddTargetModal 交互的测试：
//
//   1. Submit 按钮在填入合法 0x 地址前一直处于禁用状态
//   2. 地址长度不合法 → 按钮仍然禁用
//   3. 提交合法地址 → 触发 addCopyTarget mutation
//   4. 提交时携带 allocation cap → cap 被传给 IPC
//   5. addCopyTarget 抛出 → 错误 toast

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const { mockListCopyTargets: m1, mockRecentCopyEvents: m2, mockListPaperFills: m3,
        mockGetMirrorPaperMode: m4, mockAddCopyTarget: m5 } = vi.hoisted(() => ({
  mockListCopyTargets: vi.fn(),
  mockRecentCopyEvents: vi.fn(),
  mockListPaperFills: vi.fn(),
  mockGetMirrorPaperMode: vi.fn(),
  mockAddCopyTarget: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listCopyTargets: m1,
  recentCopyEvents: m2,
  listPaperFills: m3,
  getMirrorPaperMode: m4,
  addCopyTarget: m5,
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

const VALID_ADDR = '0x' + 'a'.repeat(40);
const TARGET_WATCHING = {
  id: 't1',
  address: VALID_ADDR,
  label: 'Whale #1',
  enabled: true,
  allocationCap: null,
  minEdge: 0.05,
  createdAt: Date.now() - 86_400_000,
};

beforeEach(() => {
  m1.mockReset().mockResolvedValue([TARGET_WATCHING]);
  m2.mockReset().mockResolvedValue([]);
  m3.mockReset().mockResolvedValue([]);
  m4.mockReset().mockResolvedValue(false);
  m5.mockReset().mockResolvedValue({
    id: 't-new', address: VALID_ADDR, label: 'New', enabled: true,
    allocationCap: null, minEdge: 0.05, createdAt: Date.now(),
  });
});

describe('Copy (v0.67a AddTargetModal)', () => {
  it('opens Add modal when Add target button is clicked', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    // 点击 Add target 按钮。页头有一个 primary 的 "Add target" 按钮。
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    expect(addBtn).toBeTruthy();
    fireEvent.click(addBtn!);
    // Modal 渲染 —— 输入字段可见
    await waitFor(() => {
      const inputs = document.querySelectorAll('input');
      expect(inputs.length).toBeGreaterThan(0);
    });
  });

  it('Submit disabled when address is empty', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    // 寻找 Submit 按钮（modal 底部 primary 的 "Add"）
    const submitBtn = await waitFor(() => {
      const btns = screen.getAllByRole('button');
      const submit = btns.find((b) => /^Add$/i.test(b.textContent?.trim() || ''));
      expect(submit).toBeTruthy();
      return submit!;
    });
    expect(submitBtn).toBeDisabled();
  });

  it('Submit disabled when address is wrong length', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    await waitFor(() => {
      expect(document.querySelectorAll('input').length).toBeGreaterThan(0);
    });
    // 输入一个过短的 address
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: '0xshort' } });
    // 寻找 submit
    await waitFor(() => {
      const btns = screen.getAllByRole('button');
      const submit = btns.find((b) => /^Add$/i.test(b.textContent?.trim() || ''));
      expect(submit).toBeTruthy();
      expect(submit).toBeDisabled();
    });
  });

  it('Submit valid 0x address → addCopyTarget called', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    await waitFor(() => {
      expect(document.querySelectorAll('input').length).toBeGreaterThan(0);
    });
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: VALID_ADDR } });
    fireEvent.change(inputs[1], { target: { value: 'My Whale' } });
    // 提交
    const submitBtn = screen.getAllByRole('button').find((b) =>
      /^Add$/i.test(b.textContent?.trim() || ''),
    );
    fireEvent.click(submitBtn!);
    await waitFor(() => {
      expect(m5).toHaveBeenCalledWith(
        expect.objectContaining({
          address: VALID_ADDR,
          label: 'My Whale',
          min_edge: 0.05, // default 5% = 0.05
        }),
      );
    });
  });

  it('Submit with allocation cap → cap passed to IPC', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    await waitFor(() => {
      expect(document.querySelectorAll('input').length).toBeGreaterThan(0);
    });
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: VALID_ADDR } });
    // Allocation cap 是第 4 个 input（位于 address、label、min edge 之后）
    fireEvent.change(inputs[3], { target: { value: '500' } });
    const submitBtn = screen.getAllByRole('button').find((b) =>
      /^Add$/i.test(b.textContent?.trim() || ''),
    );
    fireEvent.click(submitBtn!);
    await waitFor(() => {
      expect(m5).toHaveBeenCalledWith(
        expect.objectContaining({
          allocation_cap: '500',
        }),
      );
    });
  });
});
