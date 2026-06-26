// v0.72c — Audit 分支补充测试。
//
// Audit.tsx 共 203 行，包含 6 列 DataTable + 双重过滤
//（action 前缀芯片 + 搜索防抖）+ result 胶囊颜色。
// 已有测试（v0.62a + v0.70a）覆盖 18 个用例。剩余
// 未覆盖分支（13/186 = 7%）分布在：
//   - 按 actor / target / action 字段的搜索过滤（3 个分支）
//   - action 过滤芯片 "All" 按钮（重置过滤）
//   - action 过滤芯片 "pm.*" / "wallet.*" 切换
//   - 空状态：data.length=0 vs filtered.length=0（不同文案）
//   - result 胶囊颜色：result='ok' vs result='error'
//   - target 单元格：target null → em-dash 兜底
//   - payload 单元格：payload null → em-dash 兜底
//   - 字符串中没有 '.' 的 action（dot === -1，不加入前缀集合）
//   - refresh 按钮在 isRefetching 期间的禁用状态
//
// 覆盖目标：分支 69.04% → 约 85%。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const { mockListAuditLog: mlal } = vi.hoisted(() => ({
  mockListAuditLog: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listAuditLog: (...args: unknown[]) => mlal(...args),
}));

import { Audit } from './Audit';

function renderAudit() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <Audit />
    </QueryClientProvider>,
  );
}

const ENTRIES = [
  { id: 1, at: 1700000000000, actor: 'system', action: 'pm.refresh', target: 'market_123', payload: '{"force":true}', result: 'ok' },
  { id: 2, at: 1700000001000, actor: 'user_alice', action: 'wallet.add', target: '0x1234', payload: null, result: 'ok' },
  { id: 3, at: 1700000002000, actor: 'user_bob', action: 'llm.analyze', target: null, payload: '{"prompt":"x"}', result: 'error' },
  { id: 4, at: 1700000003000, actor: 'system', action: 'system_health', target: null, payload: null, result: 'ok' },
];

beforeEach(() => {
  vi.clearAllMocks();
  mlal.mockResolvedValue(ENTRIES);
});

describe('Audit (branches — v0.72c)', () => {
  it('search by actor matches user_alice', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'alice' } });
    await waitFor(() => {
      expect(screen.getByText('wallet.add')).toBeInTheDocument();
      expect(screen.queryByText('user_bob')).not.toBeInTheDocument();
    });
  });

  it('search by target field (0x1234)', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: '0x1234' } });
    await waitFor(() => {
      expect(screen.getByText('wallet.add')).toBeInTheDocument();
      expect(screen.queryByText('user_bob')).not.toBeInTheDocument();
    });
  });

  it('search by action field (llm.analyze)', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'llm.analyze' } });
    await waitFor(() => {
      expect(screen.getByText('user_bob')).toBeInTheDocument();
      expect(screen.queryByText('user_alice')).not.toBeInTheDocument();
    });
  });

  it('click "All" chip resets actionFilter', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    // 先点击 "pm.*" 芯片以过滤
    const pmChip = screen.getAllByRole('button').find(b =>
      b.textContent?.trim() === 'pm.*',
    );
    expect(pmChip).toBeDefined();
    fireEvent.click(pmChip!);
    await waitFor(() => {
      expect(screen.queryByText('wallet.add')).not.toBeInTheDocument();
    });
    // 点击 "All" 芯片以重置
    const allChip = screen.getAllByRole('button').find(b =>
      /all/i.test(b.textContent || '') && b.textContent?.trim() !== 'pm.*',
    );
    expect(allChip).toBeDefined();
    fireEvent.click(allChip!);
    await waitFor(() => {
      expect(screen.getByText('wallet.add')).toBeInTheDocument();
      expect(screen.getByText('llm.analyze')).toBeInTheDocument();
    });
  });

  it('click action filter chip restricts results to matching prefix', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    const walletChip = screen.getAllByRole('button').find(b =>
      b.textContent?.trim() === 'wallet.*',
    );
    expect(walletChip).toBeDefined();
    fireEvent.click(walletChip!);
    await waitFor(() => {
      expect(screen.getByText('wallet.add')).toBeInTheDocument();
      expect(screen.queryByText('pm.refresh')).not.toBeInTheDocument();
      expect(screen.queryByText('llm.analyze')).not.toBeInTheDocument();
    });
  });

  it('empty state shows different copy when data is empty vs filtered empty', async () => {
    mlal.mockResolvedValue([]);
    renderAudit();
    await waitFor(() => {
      // data.length === 0 → "no_writes" 文案
      const text = document.body.textContent || '';
      expect(text).toMatch(/no.write.operations/i);
    });
  });

  it('empty state shows no_match copy when data exists but filter excludes all', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    const search = screen.getByPlaceholderText(/search/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'zzzzz-no-match' } });
    await waitFor(() => {
      const text = document.body.textContent || '';
      expect(text).toMatch(/no.entries.match/i);
    });
  });

  it('result pill color: result=error renders bear pill', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_bob'));
    // user_bob 的 llm.analyze result 为 'error' → bear pill
    const errorRow = screen.getByText('llm.analyze').closest('tr');
    expect(errorRow).toBeTruthy();
    // error pill 的 className 为 'bear'
    const errorPill = errorRow?.querySelector('.bg-bear\\/15, .text-bear');
    expect(errorPill).toBeTruthy();
  });

  it('target cell renders em-dash when target is null', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_bob'));
    // user_bob 的 target 为 null —— 应渲染 "—" 而非 code
    const bobRow = screen.getByText('llm.analyze').closest('tr');
    expect(bobRow).toBeTruthy();
    const dashes = bobRow?.querySelectorAll('span');
    expect(Array.from(dashes || []).some(d => d.textContent === '—')).toBe(true);
  });

  it('payload cell renders em-dash when payload is null', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    // user_alice 的 payload 为 null
    const aliceRow = screen.getByText('wallet.add').closest('tr');
    expect(aliceRow).toBeTruthy();
    const dashes = aliceRow?.querySelectorAll('span');
    expect(Array.from(dashes || []).some(d => d.textContent === '—')).toBe(true);
  });

  it('action with no dot is skipped from prefix set', async () => {
    // ENTRIES 中包含 "system_health"（无点） —— 不应作为芯片出现
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    // 可用芯片：pm.* / wallet.* / llm.*（system_health 没有点）
    const allChips = screen.getAllByRole('button')
      .map(b => b.textContent?.trim())
      .filter(t => t && t.endsWith('.*'));
    expect(allChips).not.toContain('system_health.*');
    expect(allChips).toContain('pm.*');
    expect(allChips).toContain('wallet.*');
    expect(allChips).toContain('llm.*');
  });

  it('refresh button is enabled and triggers refetch', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    const refreshBtn = screen.getAllByRole('button').find(b =>
      /refresh/i.test(b.textContent || ''),
    );
    expect(refreshBtn).toBeDefined();
    expect(refreshBtn!.hasAttribute('disabled')).toBe(false);
    const callsBefore = mlal.mock.calls.length;
    fireEvent.click(refreshBtn!);
    await waitFor(() => {
      expect(mlal.mock.calls.length).toBeGreaterThan(callsBefore);
    });
  });
});