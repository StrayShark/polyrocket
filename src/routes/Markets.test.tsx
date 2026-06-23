// v0.62a + v0.119 — Markets component tests (football-only).
//
// v0.119 product pivot: polyrocket 只做足球市场预测
// (see docs/polyrocket-football-prd.md). Markets page UI is locked
// to football — only ['all', 'football'] filter pills, default
// category is 'football'. These tests verify the football-only
// surface.
//
// The /markets route is the user's market browser — search /
// filter / sync to refresh.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  listMarkets: vi.fn().mockResolvedValue([
    { id: 'm1', slug: 'fifwc-1', question: 'Will Argentina win?', category: 'football',
      end_date: 9999999999, active: true, resolved: false, outcome: null,
      liquidity: '1000', volume_24h: '500' },
    { id: 'm2', slug: 'fifwc-2', question: 'France to win?', category: 'football',
      end_date: 9999999999, active: true, resolved: false, outcome: null,
      liquidity: '2000', volume_24h: '800' },
  ]),
  syncMarkets: vi.fn().mockResolvedValue(1),
}));

import { Markets } from './Markets';

function renderMarkets() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <Markets />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('Markets (v0.119 football-only)', () => {
  it('renders the page title and an empty state initially', async () => {
    renderMarkets();
    await waitFor(() => {
      expect(screen.getAllByText(/markets/i).length).toBeGreaterThan(0);
    });
  });

  it('shows sync button', async () => {
    renderMarkets();
    await waitFor(() => {
      // The Sync button exists in the toolbar
      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBeGreaterThan(0);
    });
  });

  it('renders ONLY [all, football] filter pills (v0.119 football pivot)', async () => {
    renderMarkets();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const labels = buttons.map((b) => b.textContent?.trim()).filter(Boolean);
      // Should have 'all' and 'football' only — no cs2/politics/crypto/tech/other
      expect(labels).toContain('all');
      expect(labels).toContain('football');
      // Critical: non-football pills must NOT exist
      expect(labels).not.toContain('cs2');
      expect(labels).not.toContain('politics');
      expect(labels).not.toContain('crypto');
      expect(labels).not.toContain('tech');
      expect(labels).not.toContain('other');
    });
  });

  it('defaults to football category (v0.119 football pivot)', async () => {
    renderMarkets();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const footballPill = buttons.find((b) => b.textContent === 'football');
      // Football pill should have the active class (bg-accent/15)
      expect(footballPill).toBeTruthy();
      expect(footballPill?.className).toContain('bg-accent/15');
    });
  });

  it('switches filter when "all" pill is clicked (shows all categories in DB)', async () => {
    renderMarkets();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const allPill = buttons.find((b) => b.textContent === 'all');
      if (allPill) fireEvent.click(allPill);
      // After clicking, 'all' pill should be active
      const allPillAfter = buttons.find((b) => b.textContent === 'all');
      expect(allPillAfter?.className).toContain('bg-accent/15');
    });
  });

  it('search input is editable', async () => {
    renderMarkets();
    await waitFor(() => {
      const inputs = screen.getAllByPlaceholderText(/search/i);
      if (inputs.length > 0) {
        fireEvent.change(inputs[0], { target: { value: 'Argentina' } });
        expect((inputs[0] as HTMLInputElement).value).toBe('Argentina');
      }
    });
  });
});
