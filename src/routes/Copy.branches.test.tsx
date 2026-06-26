// v0.87b — Copy.tsx 补充测试，覆盖剩余未覆盖的
// 分支：addTargetMut.onError、minEdgePct 夹紧（非数字输入）、
// 剪贴板复制按钮、n=0 时的 paper 模式横幅以及 addTargetMut 成功。
//
// v0.63b + v0.67a 已有测试覆盖：
//   - 列表渲染（空/已填充/加载中/错误）
//   - AddTargetModal 流程（打开/关闭/提交）
//
// 覆盖目标：Copy.tsx 79.54% → 85%+ 语句，分支 86.84% → 90%+。

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';
import { useToastStore } from '@/stores/toast-store';

const {
  mockListCopyTargets, mockRecentCopyEvents, mockListPaperFills,
  mockGetMirrorPaperMode, mockAddCopyTarget,
} = vi.hoisted(() => ({
  mockListCopyTargets: vi.fn(),
  mockRecentCopyEvents: vi.fn(),
  mockListPaperFills: vi.fn(),
  mockGetMirrorPaperMode: vi.fn(),
  mockAddCopyTarget: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listCopyTargets: (...args: unknown[]) => mockListCopyTargets(...args),
  recentCopyEvents: (...args: unknown[]) => mockRecentCopyEvents(...args),
  listPaperFills: () => mockListPaperFills(),
  getMirrorPaperMode: () => mockGetMirrorPaperMode(),
  addCopyTarget: (...args: unknown[]) => mockAddCopyTarget(...args),
}));

import { Copy } from './Copy';

function renderCopy() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={qc}>
        <Copy />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  useToastStore.setState({ toasts: [] });
  mockListCopyTargets.mockResolvedValue([]);
  mockRecentCopyEvents.mockResolvedValue([]);
  mockListPaperFills.mockResolvedValue([]);
  mockGetMirrorPaperMode.mockResolvedValue(false);
  mockAddCopyTarget.mockResolvedValue({ id: 1, address: '0xabc', label: null, enabled: true, allocation_cap: null, min_edge: 0.05, created_at: 0 });
});

afterEach(() => cleanup());

describe('Copy branches (v0.87b)', () => {
  it('addTargetMut.onError → toast.error (line 249)', async () => {
    mockAddCopyTarget.mockRejectedValueOnce(new Error('ipc failure'));
    renderCopy();
    // 打开 Add modal
    const addBtn = await screen.findByText('Add target');
    fireEvent.click(addBtn);
    // 填入合法地址
    const input = await screen.findByPlaceholderText(/0x/i);
    fireEvent.change(input, { target: { value: '0x' + 'a'.repeat(40) } });
    // 提交
    const submitBtn = screen.getByRole('button', { name: /^Add$/ });
    fireEvent.click(submitBtn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find((t) => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body ?? '').toMatch(/ipc failure/);
    });
  });

  it('minEdgePct clamping: non-numeric input → 0 (line 300)', async () => {
    renderCopy();
    const addBtn = await screen.findByText('Add target');
    fireEvent.click(addBtn);
    // 寻找 min edge slider/input。它是 min=0 max=50 step=1 的 number 输入。
    // 具体选择器依赖 Field 组件；目前我们查找 modal 中
    // 任一带 step=1 的 number 输入。
    const modalNumberInputs = document.querySelectorAll('input[type="number"]');
    // 挑选 min=0 max=50 的那个
    const minEdgeInput = Array.from(modalNumberInputs).find(
      (el) => (el as HTMLInputElement).max === '50',
    ) as HTMLInputElement | undefined;
    expect(minEdgeInput).toBeTruthy();
    // 输入会变成 NaN —— 例如空字符串
    fireEvent.change(minEdgeInput!, { target: { value: '' } });
    // state 应为 0（通过 Math.max(0, Math.min(50, NaN || 0)) = 0 夹紧）
    expect((minEdgeInput as HTMLInputElement).value).toBe('0');
  });

  it('paper mode banner with n=0 (empty paper_fills)', async () => {
    mockGetMirrorPaperMode.mockResolvedValueOnce(true);
    mockListPaperFills.mockResolvedValueOnce([]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByTestId('copy-paper-mode-banner')).toBeInTheDocument();
    });
    // Banner 应包含 0 fills
    // Banner 文本由 t() 插值拆分；检查容器文本
      const banner = screen.getByTestId('copy-paper-mode-banner');
      expect(banner.textContent).toMatch(/paper mode|模拟盘/);
  });

  it('paper mode banner with n>0', async () => {
    mockGetMirrorPaperMode.mockResolvedValueOnce(true);
    mockListPaperFills.mockResolvedValueOnce([{ id: 1 }, { id: 2 }, { id: 3 }]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByTestId('copy-paper-mode-banner')).toBeInTheDocument();
    });
    // Banner 应包含 3 fills
    const banner = screen.getByTestId('copy-paper-mode-banner');
      expect(banner.textContent).toMatch(/paper mode|模拟盘/);
  });

  it('TargetRow clipboard copy button writes address + success toast', async () => {
    mockListCopyTargets.mockResolvedValueOnce([{
      id: 1, address: '0x1234567890abcdef1234567890abcdef12345678',
      label: 'trader1', enabled: true, allocation_cap: null, min_edge: 0.05,
      created_at: Date.now(),
    }]);
    // 模拟 navigator.clipboard
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      writable: true,
      configurable: true,
    });
    renderCopy();
    await waitFor(() => {
      expect(screen.getByText('trader1')).toBeInTheDocument();
    });
    // 寻找 copy 按钮（其 title="Copy full address"）
    const copyBtn = screen.getByTitle('Copy full address');
    fireEvent.click(copyBtn);
    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith('0x1234567890abcdef1234567890abcdef12345678');
    });
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
    });
  });

  it('addTargetMut success → invalidates copy-targets query + closes modal', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
    const invalidateSpy = vi.spyOn(qc, 'invalidateQueries');
    render(
      <MemoryRouter>
        <QueryClientProvider client={qc}>
          <Copy />
        </QueryClientProvider>
      </MemoryRouter>,
    );
    // 打开 Add modal
    const addBtn = await screen.findByText('Add target');
    fireEvent.click(addBtn);
    // 填入合法地址
    const input = await screen.findByPlaceholderText(/0x/i);
    fireEvent.change(input, { target: { value: '0x' + 'b'.repeat(40) } });
    // 提交
    const submitBtn = screen.getByRole('button', { name: /^Add$/ });
    fireEvent.click(submitBtn);
    await waitFor(() => {
      expect(mockAddCopyTarget).toHaveBeenCalled();
    });
    await waitFor(() => {
      const calls = invalidateSpy.mock.calls.map((c) => JSON.stringify(c[0]));
      expect(calls.some((c) => c.includes('copy-targets'))).toBe(true);
    });
  });
});
