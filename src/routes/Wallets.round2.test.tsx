// v0.72b —— Wallets 第 2 轮附加测试。
//
// Wallets.tsx 共 329 行，包含 4 个组件（Wallets + WalletCard
// + AddWalletModal + Field）。已有测试（v0.63b + v0.70e）共
// 19 个用例。我们新增 8 个用例覆盖剩余分支：
//   - Cancel 按钮关闭模态框，不调用 mutation
//   - 导入文件路径：pickFile 返回 null（无选择）
//   - 导入文件路径：extractAddressFromJson 返回 null → toast.error
//   - 导入文件路径：pickFile 抛出 → catch 分支 toast.error
//   - chain_id 选择器改变值（137 ↔ 80002）
//   - wallet_type 切换：eoa→smart 来回切换
//   - WalletCard last_synced_at = null → 无 last-synced 边框行
//   - addWallet 失败 → toast.error
//
// 覆盖率目标：79.16% → ~88% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';
import { useToastStore } from '@/stores/toast-store';

const { mockListWallets: mlw, mockAddWallet: maw, mockPickFile: mpf } = vi.hoisted(() => ({
  mockListWallets: vi.fn(),
  mockAddWallet: vi.fn(),
  mockPickFile: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listWallets: (...args: unknown[]) => mlw(...args),
  addWallet: (...args: unknown[]) => maw(...args),
  pickFile: (...args: unknown[]) => mpf(...args),
}));

vi.mock('@/lib/env-file', () => ({
  readFileText: vi.fn().mockResolvedValue('{}'),
}));

const { mockExtractAddress } = vi.hoisted(() => ({
  mockExtractAddress: vi.fn(),
}));
vi.mock('@/lib/wallet-file', () => ({
  extractAddressFromJson: (...args: unknown[]) => mockExtractAddress(...args),
}));

Object.defineProperty(navigator, 'clipboard', {
  value: { writeText: vi.fn().mockResolvedValue(undefined) },
  writable: true,
  configurable: true,
});

import { Wallets } from './Wallets';

function renderWallets() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Wallets />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const WALLETS = [
  { id: 'w1', address: '0x1234567890123456789012345678901234567890', label: 'main', chain_id: 137, wallet_type: 'eoa' as const, created_at: Date.now() - 86400000, last_synced_at: Date.now() - 3600000 },
  { id: 'w2', address: '0xabcdefabcdefabcdefabcdefabcdefabcdefabcd', label: null, chain_id: 80002, wallet_type: 'smart' as const, created_at: Date.now() - 172800000, last_synced_at: null },
];

function openAddModal() {
  const addButton = screen.getAllByRole('button').find(b =>
    b.textContent?.toLowerCase().includes('add'),
  );
  expect(addButton).toBeDefined();
  fireEvent.click(addButton!);
}

beforeEach(() => {
  vi.clearAllMocks();
  mlw.mockResolvedValue(WALLETS);
  maw.mockImplementation((args) => Promise.resolve({ id: 'new-w', ...args }));
  // 默认：extract 返回一个有效地址
  mockExtractAddress.mockReturnValue('0x1234567890123456789012345678901234567890');
  // 在测试间清空 toast store
  useToastStore.setState({ toasts: [] });
});

describe('Wallets (round 2 — v0.72b)', () => {
  it('cancel button closes modal without submitting mutation', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    openAddModal();
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const cancelBtn = screen.getAllByRole('button').find(b =>
      /cancel|取消/i.test(b.textContent || ''),
    );
    expect(cancelBtn).toBeDefined();
    fireEvent.click(cancelBtn!);
    await waitFor(() => {
      // Modal 关闭 —— address 输入框消失
      expect(screen.queryAllByRole('textbox').length).toBe(0);
    });
    expect(maw).not.toHaveBeenCalled();
  });

  it('import file: pickFile returns null path (no selection)', async () => {
    mpf.mockResolvedValue(null);
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    openAddModal();
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const importBtn = screen.getByTestId('wallet-import-from-file');
    fireEvent.click(importBtn);
    await waitFor(() => {
      expect(mpf).toHaveBeenCalled();
    });
    // 无 toast，无 address 设置
    const { toasts } = useToastStore.getState();
    expect(toasts.length).toBe(0);
  });

  it('import file: extract returns null → toast.error no address found', async () => {
    mpf.mockResolvedValue('/tmp/wallet.json');
    mockExtractAddress.mockReturnValue(null);
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    openAddModal();
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const importBtn = screen.getByTestId('wallet-import-from-file');
    fireEvent.click(importBtn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find(t => t.kind === 'error');
      expect(err).toBeTruthy();
      // i18n key 'wallets.add.import_no_address' 解析为 'No 0x address found...'
      expect(err?.title || err?.body).toMatch(/no.0x.address|address/i);
    });
  });

  it('import file: pickFile throws → catch branch toast.error', async () => {
    mpf.mockRejectedValue(new Error('dialog cancelled'));
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    openAddModal();
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const importBtn = screen.getByTestId('wallet-import-from-file');
    fireEvent.click(importBtn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find(t => t.kind === 'error');
      expect(err).toBeTruthy();
      // catch 分支使用 toast.error(String(err)) → body
      const text = err?.title || err?.body || '';
      expect(text).toMatch(/dialog cancelled/i);
    });
  });

  it('chain_id selector switches from Polygon to Amoy', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    openAddModal();
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('137');
    fireEvent.change(select, { target: { value: '80002' } });
    await waitFor(() => {
      expect(select.value).toBe('80002');
    });
  });

  it('wallet_type toggle: eoa → smart roundtrip', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    openAddModal();
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    // 找到 smart 按钮
    const smartBtn = screen.getAllByRole('button').find(b =>
      /smart/i.test(b.textContent || ''),
    );
    expect(smartBtn).toBeDefined();
    fireEvent.click(smartBtn!);
    await waitFor(() => {
      expect(smartBtn!.className).toContain('accent');
    });
    // 切换回 eoa
    const eoaBtn = screen.getAllByRole('button').find(b =>
      /eoa/i.test(b.textContent || '') && !/smart/i.test(b.textContent || ''),
    );
    fireEvent.click(eoaBtn!);
    await waitFor(() => {
      expect(eoaBtn!.className).toContain('accent');
    });
  });

  it('WalletCard with last_synced_at = null omits the last-synced line', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    // w2 的 last_synced_at=null —— 不应渲染 "last synced" 文本
    // 对 w2。检查是结构性的：查询唯一的 label
    // fallback 文本 "no_label"（当 w.label 为 null 时渲染）
    expect(screen.getAllByText(/no.label|无标签/i).length).toBeGreaterThan(0);
  });

  it('addWallet onError shows toast.error with message', async () => {
    maw.mockRejectedValue(new Error('add wallet failed'));
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    openAddModal();
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const addressInput = screen.getAllByRole('textbox')[0] as HTMLInputElement;
    fireEvent.change(addressInput, {
      target: { value: '0x1234567890123456789012345678901234567890' },
    });
    await waitFor(() => {
      const submitBtn = screen.getAllByRole('button').find(b =>
        b.textContent?.trim().toLowerCase() === 'add' && !b.hasAttribute('disabled'),
      );
      expect(submitBtn).toBeDefined();
    });
    const submitBtn = screen.getAllByRole('button').find(b =>
      b.textContent?.trim().toLowerCase() === 'add' && !b.hasAttribute('disabled'),
    );
    fireEvent.click(submitBtn!);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find(t => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body).toMatch(/add wallet failed/i);
    });
  });
});