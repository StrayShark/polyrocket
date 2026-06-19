// v0.72c — Audit branches additional tests.
//
// Audit.tsx is 203 lines with 6-column DataTable + dual filter
// (action prefix chip + search debounce) + result pill color.
// Existing tests (v0.62a + v0.70a) cover 18 cases. The remaining
// uncovered branches (13/186 = 7%) are split across:
//   - search filter by actor / target / action fields (3 branches)
//   - action filter chip "All" button (resets filter)
//   - action filter chip "pm.*" / "wallet.*" toggle
//   - empty state: data.length=0 vs filtered.length=0 (different copy)
//   - result pill color: result='ok' vs result='error'
//   - target cell: target null → em-dash fallback
//   - payload cell: payload null → em-dash fallback
//   - action with no '.' in string (dot === -1, not added to prefix set)
//   - refresh button disabled state during isRefetching
//
// Coverage target: branches 69.04% → ~85%.
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
    // Click "pm.*" chip first to filter
    const pmChip = screen.getAllByRole('button').find(b =>
      b.textContent?.trim() === 'pm.*',
    );
    expect(pmChip).toBeDefined();
    fireEvent.click(pmChip!);
    await waitFor(() => {
      expect(screen.queryByText('wallet.add')).not.toBeInTheDocument();
    });
    // Click "All" chip to reset
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
      // data.length === 0 → "no_writes" copy
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
    // user_bob's llm.analyze has result='error' → bear pill
    const errorRow = screen.getByText('llm.analyze').closest('tr');
    expect(errorRow).toBeTruthy();
    // error pill has 'bear' className
    const errorPill = errorRow?.querySelector('.bg-bear\\/15, .text-bear');
    expect(errorPill).toBeTruthy();
  });

  it('target cell renders em-dash when target is null', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_bob'));
    // user_bob's target is null — should render "—" instead of code
    const bobRow = screen.getByText('llm.analyze').closest('tr');
    expect(bobRow).toBeTruthy();
    const dashes = bobRow?.querySelectorAll('span');
    expect(Array.from(dashes || []).some(d => d.textContent === '—')).toBe(true);
  });

  it('payload cell renders em-dash when payload is null', async () => {
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    // user_alice's payload is null
    const aliceRow = screen.getByText('wallet.add').closest('tr');
    expect(aliceRow).toBeTruthy();
    const dashes = aliceRow?.querySelectorAll('span');
    expect(Array.from(dashes || []).some(d => d.textContent === '—')).toBe(true);
  });

  it('action with no dot is skipped from prefix set', async () => {
    // ENTRIES has "system_health" (no dot) — should NOT appear as a chip
    renderAudit();
    await waitFor(() => screen.getByText('user_alice'));
    // Available chips: pm.* / wallet.* / llm.* (system_health has no dot)
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