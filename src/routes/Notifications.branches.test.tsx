// v0.89d — Notifications.tsx branches round (+7 tests, br 66.66→100%).
//
// The 4 kind branches (info/success/warning/error) and the body
// present/absent branch in src/routes/Notifications.tsx:78-86 are
// the gaps. Existing Notifications.test.tsx covers only 'info'
// kind without body. This file adds:
//   - success / warning / error / info kind rendering (Pill class)
//   - body present branch (renders body text)
//   - body absent branch (no .text-fg-secondary element)
//   - dismiss button removes single toast

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

describe('Notifications — v0.89d branches', () => {
  beforeEach(() => {
    useToastStore.getState().clear();
  });

  it('renders success toast with bull background (KIND_CLS[success] branch)', async () => {
    useToastStore.getState().push({ kind: 'success', title: 'win', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('win')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-bull\\/10')).toBeInTheDocument();
  });

  it('renders error toast with bear background (KIND_CLS[error] branch)', async () => {
    useToastStore.getState().push({ kind: 'error', title: 'fail', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('fail')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-bear\\/10')).toBeInTheDocument();
  });

  it('renders warning toast with warning background (KIND_CLS[warning] branch)', async () => {
    useToastStore.getState().push({ kind: 'warning', title: 'careful', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('careful')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-warning\\/10')).toBeInTheDocument();
  });

  it('renders info toast with accent background (KIND_CLS[info] branch)', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'fyi', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('fyi')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-accent\\/10')).toBeInTheDocument();
  });

  it('renders toast body when body is present (t2.body truthy branch)', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'with body', body: 'extra details', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('extra details')).toBeInTheDocument();
    });
  });

  it('omits body element when body is not provided (t2.body falsy branch)', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'no body here', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('no body here')).toBeInTheDocument();
    });
    // No .text-fg-secondary element should be rendered for body
    const bodies = document.querySelectorAll('.text-fg-secondary');
    expect(bodies.length).toBe(0);
  });

  it('dismiss button removes the corresponding toast', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'removable', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('removable')).toBeInTheDocument();
    });
    const dismissBtn = document.querySelector(
      '[aria-label="notifications.toast.dismiss_aria"]',
    ) as HTMLButtonElement;
    expect(dismissBtn).toBeTruthy();
    fireEvent.click(dismissBtn);
    await waitFor(() => {
      expect(screen.queryByText('removable')).not.toBeInTheDocument();
    });
  });
});
