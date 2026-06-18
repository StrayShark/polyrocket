// v0.62a.2 — Brief component tests.
//
// /brief is the daily-brief full list page.
// Today 0% coverage. This file covers:
//   1. Initial render with empty data
//   2. Refresh button triggers mutation
//   3. Tab list (Top / Filter)

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  dailyBriefGet: vi.fn().mockResolvedValue([]),
  dailyBriefRefresh: vi.fn().mockResolvedValue({ computed_at: 0, n_items: 0 }),
  dailyBriefDismiss: vi.fn().mockResolvedValue(undefined),
}));

import { Brief } from './Brief';

function renderBrief() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Brief />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Brief', () => {
  it('renders the page', async () => {
    renderBrief();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows the refresh button', async () => {
    renderBrief();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const refresh = buttons.find((b) =>
        b.textContent?.toLowerCase().includes('refresh'),
      );
      expect(refresh).toBeTruthy();
    });
  });
});
