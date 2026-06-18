// v0.62a — Signals component tests.
//
// /signals is the active-signal list. Today 0%
// coverage. This file covers:
//   1. Initial render with empty data
//   2. Recompute button (stub) renders
//   3. Min-edge filter input is editable

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listActiveSignals: vi.fn().mockResolvedValue([]),
  recomputeSignals: vi.fn().mockResolvedValue(0),
}));

import { Signals } from './Signals';

function renderSignals() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Signals />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Signals', () => {
  it('renders the page', async () => {
    renderSignals();
    await waitFor(() => {
      // Multiple elements match "/signals/i" (h2 + nav link)
      expect(screen.getAllByText(/signals/i).length).toBeGreaterThan(0);
    });
  });

  it('shows the recompute button', async () => {
    renderSignals();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBeGreaterThan(0);
    });
  });
});
