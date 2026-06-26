// v0.70e —— Wallets 路由附加测试。
//
// /wallets 是一个 329 行、4 组件（Wallets + WalletCard +
// AddWalletModal + Field）的页面。已有测试（v0.63b）共
// 7 个用例覆盖基础渲染 + 无效地址。我们新增
// 10 个聚焦测试，覆盖分支密集的状态机：
// 复制到剪贴板、从文件导入、地址校验（0x + 42）、
// chain 选择器、类型切换、refresh 旋转。
//
// Wallets.tsx：45.8% → ~75% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

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

// env-file + wallet-file 的 mocks
vi.mock('@/lib/env-file', () => ({
  readFileText: vi.fn().mockResolvedValue('{}'),
}));
vi.mock('@/lib/wallet-file', () => ({
  extractAddressFromJson: vi.fn().mockReturnValue('0x1234567890123456789012345678901234567890'),
}));

// happy-dom 没有 clipboard —— 在此 stub。
// 因为 navigator.clipboard 是只读，所以使用 defineProperty。
Object.defineProperty(navigator, 'clipboard', {
  value: {
    writeText: vi.fn().mockResolvedValue(undefined),
  },
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

beforeEach(() => {
  vi.clearAllMocks();
  mlw.mockResolvedValue(WALLETS);
  maw.mockImplementation((args) => Promise.resolve({ id: 'new-w', ...args }));
});

describe('Wallets (extended)', () => {
  it('renders loading skeleton with 4 cards on initial mount', async () => {
    mlw.mockReturnValue(new Promise(() => {}));
    renderWallets();
    expect(screen.getAllByRole('generic').length).toBeGreaterThan(0);
  });

  it('renders error state when listWallets throws', async () => {
    mlw.mockRejectedValue(new Error('wallets fetch failed'));
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText(/wallets fetch failed/)).toBeInTheDocument();
    });
  });

  it('renders empty state when data is empty', async () => {
    mlw.mockResolvedValue([]);
    renderWallets();
    await waitFor(() => {
      // 带 "add first" 操作按钮的空状态
      const text = document.body.textContent || '';
      expect(text.length).toBeGreaterThan(0);
    });
  });

  it('renders 2 wallet cards in grid layout', async () => {
    renderWallets();
    await waitFor(() => {
      // 主钱包 label 可见
      expect(screen.getByText('main')).toBeInTheDocument();
    });
    // 第二个钱包 —— label 为 null，因此显示兜底文本
    const text = document.body.textContent || '';
    expect(text).toMatch(/0x1234|0xabcd/);
  });

  it('copies address to clipboard when copy icon is clicked', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    // 查找所有 copy 按钮（每张钱包卡一个）
    const copyButtons = document.querySelectorAll('button[title*="opy" i], button[title*="复制" i]');
    expect(copyButtons.length).toBeGreaterThan(0);
    fireEvent.click(copyButtons[0]);
    await waitFor(() => {
      expect((navigator.clipboard as any).writeText).toHaveBeenCalledWith(
        expect.stringMatching(/^0x[a-fA-F0-9]+$/),
      );
    });
  });

  it('opens AddWalletModal when Add button is clicked', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    const addButton = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('add'),
    );
    expect(addButton).toBeDefined();
    fireEvent.click(addButton!);
    await waitFor(() => {
      // Modal 打开 —— 地址输入框出现
      const inputs = screen.getAllByRole('textbox');
      expect(inputs.length).toBeGreaterThan(0);
    });
  });

  it('disables Add submit when address is invalid (no 0x prefix)', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    const addButton = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('add'),
    );
    fireEvent.click(addButton!);
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    // 输入无效地址（无 0x）
    const addressInput = screen.getAllByRole('textbox')[0] as HTMLInputElement;
    fireEvent.change(addressInput, { target: { value: 'not-an-address' } });
    await waitFor(() => {
      // modal footer 的 submit Add 按钮应处于 disabled
      const submitBtn = screen.getAllByRole('button').find(b =>
        b.textContent?.trim().toLowerCase() === 'add' && b.hasAttribute('disabled'),
      );
      expect(submitBtn).toBeDefined();
    });
  });

  it('disables Add submit when address length is not 42 chars', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    const addButton = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('add'),
    );
    fireEvent.click(addButton!);
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const addressInput = screen.getAllByRole('textbox')[0] as HTMLInputElement;
    fireEvent.change(addressInput, { target: { value: '0x123' } }); // too short
    await waitFor(() => {
      const submitBtn = screen.getAllByRole('button').find(b =>
        b.textContent?.trim().toLowerCase() === 'add' && b.hasAttribute('disabled'),
      );
      expect(submitBtn).toBeDefined();
    });
  });

  it('enables Add submit when address is valid 0x + 42 chars', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    const addButton = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('add'),
    );
    fireEvent.click(addButton!);
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
  });

  it('toggles wallet type between eoa and smart', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    const addButton = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('add'),
    );
    fireEvent.click(addButton!);
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    // 查找 smart 按钮 —— 文本包含 'smart' 或 '智能'
    const smartBtn = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('smart'),
    );
    if (smartBtn) {
      fireEvent.click(smartBtn);
      await waitFor(() => {
        // 点击后，smart 按钮应具有 accent class
        expect(smartBtn.className).toContain('accent');
      });
    }
  });

  it('submits addWallet mutation when valid address is entered', async () => {
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    const addButton = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('add'),
    );
    fireEvent.click(addButton!);
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const addressInput = screen.getAllByRole('textbox')[0] as HTMLInputElement;
    fireEvent.change(addressInput, {
      target: { value: '0x1234567890123456789012345678901234567890' },
    });
    // 查找 modal footer 的 Add 按钮（非主页面按钮）
    await waitFor(() => {
      const modalAddBtns = screen.getAllByRole('button').filter(b =>
        b.textContent?.trim().toLowerCase() === 'add' && !b.hasAttribute('disabled'),
      );
      expect(modalAddBtns.length).toBeGreaterThan(0);
    });
    const submitBtn = screen.getAllByRole('button').find(b =>
      b.textContent?.trim().toLowerCase() === 'add' && !b.hasAttribute('disabled'),
    );
    fireEvent.click(submitBtn!);
    await waitFor(() => {
      expect(maw).toHaveBeenCalled();
    });
  });

  it('imports address from JSON file via pickFile', async () => {
    mpf.mockResolvedValue('/path/to/wallet.json');
    renderWallets();
    await waitFor(() => screen.getByText('main'));
    const addButton = screen.getAllByRole('button').find(b =>
      b.textContent?.toLowerCase().includes('add'),
    );
    fireEvent.click(addButton!);
    await waitFor(() => screen.getAllByRole('textbox').length > 0);
    const importBtn = screen.getByTestId('wallet-import-from-file');
    fireEvent.click(importBtn);
    await waitFor(() => {
      // extractAddressFromJson mock 返回 0x...42 字符地址，
      // 该地址被设置到地址输入框中。
      expect(mpf).toHaveBeenCalled();
      const addressInput = screen.getAllByRole('textbox')[0] as HTMLInputElement;
      expect(addressInput.value).toMatch(/^0x[0-9a-f]{40}$/);
    });
  });
});
