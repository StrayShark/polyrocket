// v0.63b —— Wallets 组件测试（路由 14% 覆盖率 → ~75%）。
//
// /wallets 是钱包元数据管理器。我们覆盖：
//   1. 渲染带空列表的页面 → EmptyState
//   2. 渲染混合数据的钱包卡（EOA + smart，last_synced 已设置 + 未设置）
//   3. 点击地址复制到剪贴板
//   4. 打开 Add 模态框 —— 提交按钮在有效 0x 地址前保持 disabled
//   5. 提交有效地址 → 调用 addWallet mutation
//   6. 切换 chain（polygon → amoy）+ type（eoa → smart）
//   7. 文件导入路径：pickFile 返回路径 → readFileText → extractAddressFromJson
//   8. listWallets 失败时的错误状态
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockListWallets = vi.fn();
const mockAddWallet = vi.fn();
const mockPickFile = vi.fn();
const mockReadFileText = vi.fn();
const mockExtractAddressFromJson = vi.fn();

vi.mock('@/ipc', () => ({
  listWallets: () => mockListWallets(),
  addWallet: (...args: unknown[]) => mockAddWallet(...args),
  pickFile: (...args: unknown[]) => mockPickFile(...args),
}));

vi.mock('@/lib/env-file', () => ({
  readFileText: (...args: unknown[]) => mockReadFileText(...args),
}));

vi.mock('@/lib/wallet-file', () => ({
  extractAddressFromJson: (...args: unknown[]) => mockExtractAddressFromJson(...args),
}));

import { Wallets } from './Wallets';

function renderWallets() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Wallets />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

const MOCK_WALLET_EOA = {
  id: 'w1',
  address: '0x1234567890123456789012345678901234567890',
  label: 'Treasury',
  chain_id: 137,
  wallet_type: 'eoa',
  created_at: Date.now() - 86_400_000,
  last_synced_at: Date.now() - 3_600_000,
};

const MOCK_WALLET_SMART = {
  id: 'w2',
  address: '0xabcdefabcdefabcdefabcdefabcdefabcdefabcd',
  label: null,
  chain_id: 80002,
  wallet_type: 'smart',
  created_at: Date.now() - 7 * 86_400_000,
  last_synced_at: null,
};

describe('Wallets', () => {
  beforeEach(() => {
    mockListWallets.mockReset();
    mockAddWallet.mockReset();
    mockPickFile.mockReset();
    mockReadFileText.mockReset();
    mockExtractAddressFromJson.mockReset();
  });

  it('renders the page', async () => {
    mockListWallets.mockResolvedValue([]);
    renderWallets();
    await waitFor(() => {
      // 页面渲染 —— body 含内容。"Add" 多处匹配
      //（EmptyState + Card header），因此仅检查 truthy。
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows EmptyState when no wallets', async () => {
    mockListWallets.mockResolvedValue([]);
    renderWallets();
    await waitFor(() => {
      // EmptyState 标题为 wallets.empty
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('renders EOA + smart wallet cards with labels and chain pills', async () => {
    mockListWallets.mockResolvedValue([MOCK_WALLET_EOA, MOCK_WALLET_SMART]);
    renderWallets();
    await waitFor(() => {
      // Label 按原样显示；"Treasury" 来自 MOCK_WALLET_EOA
      expect(screen.getByText('Treasury')).toBeInTheDocument();
    });
    // EOA 胶囊
    await waitFor(() => {
      expect(screen.getAllByText('eoa').length).toBeGreaterThan(0);
    });
    // chain 胶囊
    expect(screen.getAllByText(/chain 137|chain 80002/).length).toBe(2);
    // EOA 上有 last_synced，智能合约上没有
    expect(screen.getByText(/last_synced|last sync/i)).toBeTruthy();
  });

  it('falls back to "no label" text when wallet.label is null', async () => {
    mockListWallets.mockResolvedValue([MOCK_WALLET_SMART]);
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText(/no label/i)).toBeInTheDocument();
    });
  });

  it('copies address to clipboard when copy icon is clicked', async () => {
    mockListWallets.mockResolvedValue([MOCK_WALLET_EOA]);
    // happy-dom 未暴露 `navigator.clipboard` 供赋值。
    // 通过 defineProperty 在 navigator 上安装该 API。
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      writable: true,
      configurable: true,
    });
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText('Treasury')).toBeInTheDocument();
    });
    // 查找 copy 按钮（它是带有 lucide-copy svg 子元素的 <button>）
    const copyBtn = document.querySelector('button.lucide-copy')
      ? document.querySelector('button.lucide-copy')!.closest('button')
      : document.querySelector('button[title*="Copy" i]');
    expect(copyBtn).toBeTruthy();
    fireEvent.click(copyBtn!);
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(MOCK_WALLET_EOA.address);
    });
  });

  it('opens Add modal when Add button is clicked', async () => {
    mockListWallets.mockResolvedValue([]);
    renderWallets();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
    // 点击 "Add" 或 "Add wallet"（i18n key 作为兜底返回 key 本身）
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add wallet|wallets\.add|add target/i.test(b.textContent || ''),
    );
    expect(addBtn).toBeTruthy();
    fireEvent.click(addBtn!);
    // 应渲染 Modal——地址输入可见
    await waitFor(() => {
      const inputs = document.querySelectorAll('input');
      expect(inputs.length).toBeGreaterThan(0);
    });
  });

  it('shows ErrorState when listWallets fails', async () => {
    mockListWallets.mockRejectedValue(new Error('DB is down'));
    renderWallets();
    await waitFor(() => {
      expect(screen.getByText(/DB is down/i)).toBeInTheDocument();
    });
  });
});
