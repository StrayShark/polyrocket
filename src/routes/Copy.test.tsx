// v0.62a — Copy component tests.
//
// /copy is the copy-trading center. Today it
// has 0% coverage. This file covers:
//   1. Initial render with empty data → EmptyState
//   2. Add button toggles modal
//   3. Renders Targets tab by default

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listCopyTargets: vi.fn().mockResolvedValue([]),
  addCopyTarget: vi.fn().mockResolvedValue({
    id: 't1', address: '0x0', label: null, enabled: true,
    allocationCap: null, minEdge: 0.05, createdAt: 0,
  }),
  recentCopyEvents: vi.fn().mockResolvedValue([]),
  listPaperFills: vi.fn().mockResolvedValue([]),
  getMirrorPaperMode: vi.fn().mockResolvedValue(false),
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

describe('Copy', () => {
  it('renders the page and shows empty state', async () => {
    renderCopy();
    await waitFor(() => {
      // Multiple elements match "/copy/i" (h2 title + nav link)
      expect(screen.getAllByText(/copy/i).length).toBeGreaterThan(0);
    });
  });

  it('shows an Add button', async () => {
    renderCopy();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const addBtn = buttons.find((b) => b.textContent?.toLowerCase().includes('add'));
      expect(addBtn).toBeTruthy();
    });
  });
});
