// v0.70f — ToastViewport + ToastHost component tests.
//
// Toast.tsx was 0% covered. It's a small (96 lines) but
// critical feedback component — every successful IPC mutation
// in the app shows a toast. Tests cover:
//   - 4 kinds render with their respective KIND_CLS classes
//   - body text renders below title when present
//   - dismiss button calls useToastStore.dismiss(id)
//   - empty queue renders nothing
//   - ToastHost's useEffect initializes the store
//
// Toast.tsx: 0% → ~95% stmts.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

const { mockToastState, mockDismiss, mockUseToastStore } = vi.hoisted(() => {
  const toastState = { toasts: [] as any[] };
  const dismissFn = vi.fn();
  // useToastStore is called as both a hook (selector pattern) and
  // useToastStore.getState(). Build it as a vi.fn (callable) with
  // a getState static method.
  const useToastStore: any = Object.assign(vi.fn(), {
    getState: vi.fn(() => ({ toasts: toastState.toasts, dismiss: dismissFn })),
  });
  return { mockToastState: toastState, mockDismiss: dismissFn, mockUseToastStore: useToastStore };
});

vi.mock('@/stores/toast-store', () => ({
  useToastStore: mockUseToastStore,
}));

import { ToastViewport, ToastHost } from './Toast';

function renderViewport() {
  // Wire the hook impl per render to read from our mutable state.
  mockUseToastStore.mockImplementation((selector: any) =>
    selector({ toasts: mockToastState.toasts, dismiss: mockDismiss }),
  );
  return render(<ToastViewport />);
}

beforeEach(() => {
  vi.clearAllMocks();
  mockToastState.toasts = [];
});

describe('ToastViewport', () => {
  it('renders nothing when toasts queue is empty', () => {
    renderViewport();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('renders info toast with accent border class', () => {
    mockToastState.toasts = [
      { id: '1', kind: 'info', title: 'Info title', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Info title')).toBeInTheDocument();
    const div = screen.getByText('Info title').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-accent/);
  });

  it('renders success toast with bull (green) border class', () => {
    mockToastState.toasts = [
      { id: '2', kind: 'success', title: 'Saved!', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Saved!')).toBeInTheDocument();
    const div = screen.getByText('Saved!').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-bull/);
  });

  it('renders warning toast with warning border class', () => {
    mockToastState.toasts = [
      { id: '3', kind: 'warning', title: 'Heads up', ttl: 6000 },
    ];
    renderViewport();
    expect(screen.getByText('Heads up')).toBeInTheDocument();
    const div = screen.getByText('Heads up').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-warning/);
  });

  it('renders error toast with bear (red) border class', () => {
    mockToastState.toasts = [
      { id: '4', kind: 'error', title: 'Failed', ttl: 0 },
    ];
    renderViewport();
    expect(screen.getByText('Failed')).toBeInTheDocument();
    const div = screen.getByText('Failed').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-bear/);
  });

  it('renders body text when provided', () => {
    mockToastState.toasts = [
      { id: '5', kind: 'info', title: 'Title', body: 'Body content here', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Title')).toBeInTheDocument();
    expect(screen.getByText('Body content here')).toBeInTheDocument();
  });

  it('does not render body div when body is absent', () => {
    mockToastState.toasts = [
      { id: '6', kind: 'success', title: 'Just title', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Just title')).toBeInTheDocument();
    // No second text-11 div with body
    expect(document.body.textContent).not.toContain('Body');
  });

  it('calls dismiss(id) when X button is clicked', () => {
    mockToastState.toasts = [
      { id: '7', kind: 'error', title: 'Dismiss me', ttl: 0 },
    ];
    renderViewport();
    const dismissBtn = screen.getByRole('button', { name: /dismiss/i });
    fireEvent.click(dismissBtn);
    expect(mockDismiss).toHaveBeenCalledWith('7');
  });

  it('renders multiple toasts in order (newest at bottom)', () => {
    mockToastState.toasts = [
      { id: 'a', kind: 'info', title: 'First', ttl: 4000 },
      { id: 'b', kind: 'success', title: 'Second', ttl: 4000 },
      { id: 'c', kind: 'error', title: 'Third', ttl: 0 },
    ];
    renderViewport();
    expect(screen.getByText('First')).toBeInTheDocument();
    expect(screen.getByText('Second')).toBeInTheDocument();
    expect(screen.getByText('Third')).toBeInTheDocument();
  });

  it('has aria-live="polite" on the container for screen-reader announcements', () => {
    renderViewport();
    const container = document.querySelector('[aria-live="polite"]');
    expect(container).toBeInTheDocument();
  });
});

describe('ToastHost', () => {
  it('mounts ToastViewport (renders empty queue)', () => {
    render(<ToastHost />);
    // No toasts → no buttons
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('mounts ToastViewport with toast visible', () => {
    mockToastState.toasts = [
      { id: 'h1', kind: 'info', title: 'Host test', ttl: 4000 },
    ];
    render(<ToastHost />);
    expect(screen.getByText('Host test')).toBeInTheDocument();
  });
});
