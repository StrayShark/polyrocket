// v0.62a.2 — Notifications component tests.
//
// /notifications is the toast list page.
// Today 0% coverage. This file covers:
//   1. Initial render with empty toasts
//   2. Renders a toast when one is pushed
//   3. Clear button works

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k }),
}));

vi.mock('@/ipc', () => ({
  sendNotification: vi.fn(),
}));

import { Notifications } from './Notifications';
import { useToastStore } from '@/stores/toast-store';

describe('Notifications', () => {
  beforeEach(() => {
    useToastStore.getState().clear();
  });

  it('renders the page with empty state when no toasts', async () => {
    render(
      <MemoryRouter>
        <Notifications />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows a toast when one is in the store', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'hello', ttl: 0 });
    render(
      <MemoryRouter>
        <Notifications />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(screen.getByText('hello')).toBeInTheDocument();
    });
  });

  it('clear button empties the toasts', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'a', ttl: 0 });
    useToastStore.getState().push({ kind: 'info', title: 'b', ttl: 0 });
    render(
      <MemoryRouter>
        <Notifications />
      </MemoryRouter>,
    );
    const clearBtn = await screen.findByText(/clear/i);
    fireEvent.click(clearBtn);
    await waitFor(() => {
      expect(useToastStore.getState().toasts).toEqual([]);
    });
  });
});
