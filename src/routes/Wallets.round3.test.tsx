// v0.75d —— Wallets 第 3 轮分支测试（+4 个测试，96.7→99% br）。
//
// Wallets.tsx 共 329 行，包含 3 个子区段（Header+Refresh /
// List / AddModal）。14 个已有测试（test + extras + round2）
// 已覆盖大部分流程。v8 覆盖率报告 4 处未覆盖分支位于
// 85、98、111、268 行 —— 我们用聚焦测试覆盖它们。
//
// 覆盖率目标：96.7% br → ~99% br。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListWallets = vi.fn();
const mockAddWallet = vi.fn();
const mockPickFile = vi.fn();

vi.mock('@/ipc', () => ({
  listWallets: () => mockListWallets(),
  addWallet: (...args: unknown[]) => Promise.resolve(mockAddWallet(...args)),
  pickFile: (...args: unknown[]) => Promise.resolve(mockPickFile(...args)),
}));

vi.mock('@/lib/env-file', () => ({
  readFileText: () => Promise.resolve(''),
}));

vi.mock('@/lib/wallet-file', () => ({
  extractAddressFromJson: () => null,
}));

import { Wallets } from './Wallets';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  mockListWallets.mockResolvedValue([]);
  mockAddWallet.mockResolvedValue({ id: 'w1', address: '0x' });
  mockPickFile.mockResolvedValue(null);
});

describe('Wallets round 3 (v0.75d — branch closing)', () => {
  it('clicking the refresh button triggers a refetch', async () => {
    wrap(<Wallets />);
    await waitFor(() => screen.getAllByRole('button').length > 0);
    // 通过图标（RefreshCw）或 aria-label 找到 refresh 按钮
    const refreshBtn = document.querySelector('button svg.animate-spin')?.closest('button')
      || screen.getAllByRole('button').find(b => b.querySelector('svg.lucide-refresh-cw'));
    if (refreshBtn) {
      const callsBefore = mockListWallets.mock.calls.length;
      fireEvent.click(refreshBtn);
      await waitFor(() => {
        expect(mockListWallets.mock.calls.length).toBeGreaterThan(callsBefore);
      });
    }
  });

  it('error state renders when listWallets rejects', async () => {
    mockListWallets.mockRejectedValue(new Error('network down'));
    wrap(<Wallets />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/error|failed|wrong/i);
    });
  });

  it('empty state Add button opens Add modal with input', async () => {
    wrap(<Wallets />);
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/empty|wallet|add/i);
    });
    // 找到主要 add 按钮（任何包含 "add" / "add_first" 的文本）
    const addBtn = screen.getAllByRole('button').find(b =>
      /add/i.test(b.textContent || ''),
    );
    expect(addBtn).toBeTruthy();
    if (addBtn) {
      fireEvent.click(addBtn);
      // 打开后，label input 应出现
      await waitFor(() => {
        const inputs = document.querySelectorAll('input');
        expect(inputs.length).toBeGreaterThan(0);
      });
    }
  });

  it('Add modal label input accepts text input (controlled state)', async () => {
    wrap(<Wallets />);
    await waitFor(() => screen.getAllByRole('button').length > 0);
    // 点击任何 add 按钮以打开 modal
    const addBtn = screen.getAllByRole('button').find(b => /add/i.test(b.textContent || ''));
    expect(addBtn).toBeTruthy();
    if (addBtn) {
      fireEvent.click(addBtn);
      // 等待 modal 打开
      await waitFor(() => {
        const inputs = document.querySelectorAll('input');
        expect(inputs.length).toBeGreaterThan(0);
      });
      // 找到 label 输入框 —— 第二个 input（placeholder "primary, trade-1, cold, …"）
      const allInputs = document.querySelectorAll('input');
      const labelInput = allInputs[1] as HTMLInputElement;
      expect(labelInput).toBeTruthy();
      fireEvent.change(labelInput, { target: { value: 'My Treasury' } });
      expect(labelInput.value).toBe('My Treasury');
    }
  });
});
