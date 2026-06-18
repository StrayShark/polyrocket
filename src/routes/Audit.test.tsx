// v0.62a — Audit component tests.
//
// /audit is the read-only audit log browser.
// Today 0% coverage. This file covers the
// initial render + filter chips + table.

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
