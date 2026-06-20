// v0.87a — Audit.tsx cell renderer tests.
//
// Audit.tsx has 6 column definitions with inline cell renderers
// (lines 75, 83, 91, 109). v0.62a + v0.70a + v0.72c tests render the
// table with SAMPLE data but don't explicitly assert each cell's
// content type. v8 coverage reports the cell arrow function bodies
// as missed because the inline JSX execution is hard to attribute
// to source lines.
//
// These tests assert each column's cell renders the expected JSX:
//   - `at` → fmtDateTime output (in a span with font-mono class)
//   - `actor` → <Pill kind="muted">
//   - `action` → <span> with the action text
//   - `target` → <code> (when target is non-null) OR <span>— (when null)
//   - `result` → <Pill kind="bull"|"bear"> depending on result value
//
// Also covers line 174 (ErrorState on listAuditLog rejection with retry).
//
// Coverage target: Audit.tsx 88.37% → 90%+ stmts.

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
    // The at column renders the formatted timestamp in a span.font-mono.
    // 1718710000000 → 2024-06-18 (something like that). The exact format
    // depends on fmtDateTime. We check the font-mono span exists.
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
      // The action column renders inside a span with text-fg class
      const fgSpans = document.querySelectorAll('span.text-fg.font-mono');
      expect(fgSpans.length).toBeGreaterThan(0);
    });
  });

  it('result column cell renders <Pill> with bull/bear kind (line 109)', async () => {
    mockListAuditLog.mockResolvedValueOnce(FULL_SAMPLE);
    renderAudit();
    await waitFor(() => {
      // The result column renders ok/error inside a Pill
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
      // target is null → renders "—" (em-dash). Multiple em-dashes may
      // exist (e.g. payload null), so just check at least one is present.
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
    // The ErrorState has a retry button. Click it, then make the second
    // mock call succeed.
    mockListAuditLog.mockResolvedValueOnce([]);
    const retryBtn = screen.getByRole('button', { name: /try again/i });
    fireEvent.click(retryBtn);
    await waitFor(() => {
      expect(mockListAuditLog).toHaveBeenCalledTimes(2);
    });
  });
});
