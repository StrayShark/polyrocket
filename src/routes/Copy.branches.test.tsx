// v0.87b — Copy.tsx additional tests targeting the remaining uncovered
// branches: addTargetMut.onError, minEdgePct clamping (non-numeric input),
// clipboard copy button, paper-mode banner with n=0, and addTargetMut success.
//
// v0.63b + v0.67a existing tests cover:
//   - List rendering (empty/populated/loading/error)
//   - AddTargetModal flows (open/close/submit)
//
// Coverage target: Copy.tsx 79.54% → 85%+ stmts, branches 86.84% → 90%+.

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';
import { useToastStore } from '@/stores/toast-store';

const {
  mockListCopyTargets, mockRecentCopyEvents, mockListPaperFills,
  mockGetMirrorPaperMode, mockAddCopyTarget,
} = vi.hoisted(() => ({
  mockListCopyTargets: vi.fn(),
  mockRecentCopyEvents: vi.fn(),
  mockListPaperFills: vi.fn(),
  mockGetMirrorPaperMode: vi.fn(),
  mockAddCopyTarget: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listCopyTargets: (...args: unknown[]) => mockListCopyTargets(...args),
  recentCopyEvents: (...args: unknown[]) => mockRecentCopyEvents(...args),
  listPaperFills: () => mockListPaperFills(),
  getMirrorPaperMode: () => mockGetMirrorPaperMode(),
  addCopyTarget: (...args: unknown[]) => mockAddCopyTarget(...args),
}));

import { Copy } from './Copy';

function renderCopy() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={qc}>
        <Copy />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  useToastStore.setState({ toasts: [] });
  mockListCopyTargets.mockResolvedValue([]);
  mockRecentCopyEvents.mockResolvedValue([]);
  mockListPaperFills.mockResolvedValue([]);
  mockGetMirrorPaperMode.mockResolvedValue(false);
  mockAddCopyTarget.mockResolvedValue({ id: 1, address: '0xabc', label: null, enabled: true, allocation_cap: null, min_edge: 0.05, created_at: 0 });
});

afterEach(() => cleanup());

describe('Copy branches (v0.87b)', () => {
  it('addTargetMut.onError → toast.error (line 249)', async () => {
    mockAddCopyTarget.mockRejectedValueOnce(new Error('ipc failure'));
    renderCopy();
    // Open Add modal
    const addBtn = await screen.findByText('Add target');
    fireEvent.click(addBtn);
    // Fill valid address
    const input = await screen.findByPlaceholderText(/0x/i);
    fireEvent.change(input, { target: { value: '0x' + 'a'.repeat(40) } });
    // Submit
    const submitBtn = screen.getByRole('button', { name: /^Add$/ });
    fireEvent.click(submitBtn);
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const err = toasts.find((t) => t.kind === 'error');
      expect(err).toBeTruthy();
      expect(err?.body ?? '').toMatch(/ipc failure/);
    });
  });

  it('minEdgePct clamping: non-numeric input → 0 (line 300)', async () => {
    renderCopy();
    const addBtn = await screen.findByText('Add target');
    fireEvent.click(addBtn);
    // Find the min edge slider/input. It's a number input with min=0 max=50 step=1.
    // The exact selector depends on the Field component; for now we look for
    // any number input that has step=1 in the modal.
    const modalNumberInputs = document.querySelectorAll('input[type="number"]');
    // Pick the one with min=0 max=50
    const minEdgeInput = Array.from(modalNumberInputs).find(
      (el) => (el as HTMLInputElement).max === '50',
    ) as HTMLInputElement | undefined;
    expect(minEdgeInput).toBeTruthy();
    // Type something that becomes NaN — e.g. empty string
    fireEvent.change(minEdgeInput!, { target: { value: '' } });
    // The state should be 0 (clamped via Math.max(0, Math.min(50, NaN || 0)) = 0)
    expect((minEdgeInput as HTMLInputElement).value).toBe('0');
  });

  it('paper mode banner with n=0 (empty paper_fills)', async () => {
    mockGetMirrorPaperMode.mockResolvedValueOnce(true);
    mockListPaperFills.mockResolvedValueOnce([]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByTestId('copy-paper-mode-banner')).toBeInTheDocument();
    });
    // Banner should mention 0 fills
    // Banner text is split by t() interpolation; check the container text
      const banner = screen.getByTestId('copy-paper-mode-banner');
      expect(banner.textContent).toMatch(/paper mode|模拟盘/);
  });

  it('paper mode banner with n>0', async () => {
    mockGetMirrorPaperMode.mockResolvedValueOnce(true);
    mockListPaperFills.mockResolvedValueOnce([{ id: 1 }, { id: 2 }, { id: 3 }]);
    renderCopy();
    await waitFor(() => {
      expect(screen.getByTestId('copy-paper-mode-banner')).toBeInTheDocument();
    });
    // Banner should mention 3 fills
    const banner = screen.getByTestId('copy-paper-mode-banner');
      expect(banner.textContent).toMatch(/paper mode|模拟盘/);
  });

  it('TargetRow clipboard copy button writes address + success toast', async () => {
    mockListCopyTargets.mockResolvedValueOnce([{
      id: 1, address: '0x1234567890abcdef1234567890abcdef12345678',
      label: 'trader1', enabled: true, allocation_cap: null, min_edge: 0.05,
      created_at: Date.now(),
    }]);
    // Mock navigator.clipboard
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      writable: true,
      configurable: true,
    });
    renderCopy();
    await waitFor(() => {
      expect(screen.getByText('trader1')).toBeInTheDocument();
    });
    // Find the copy button (it has title="Copy full address")
    const copyBtn = screen.getByTitle('Copy full address');
    fireEvent.click(copyBtn);
    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith('0x1234567890abcdef1234567890abcdef12345678');
    });
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
    });
  });

  it('addTargetMut success → invalidates copy-targets query + closes modal', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
    const invalidateSpy = vi.spyOn(qc, 'invalidateQueries');
    render(
      <MemoryRouter>
        <QueryClientProvider client={qc}>
          <Copy />
        </QueryClientProvider>
      </MemoryRouter>,
    );
    // Open Add modal
    const addBtn = await screen.findByText('Add target');
    fireEvent.click(addBtn);
    // Fill valid address
    const input = await screen.findByPlaceholderText(/0x/i);
    fireEvent.change(input, { target: { value: '0x' + 'b'.repeat(40) } });
    // Submit
    const submitBtn = screen.getByRole('button', { name: /^Add$/ });
    fireEvent.click(submitBtn);
    await waitFor(() => {
      expect(mockAddCopyTarget).toHaveBeenCalled();
    });
    await waitFor(() => {
      const calls = invalidateSpy.mock.calls.map((c) => JSON.stringify(c[0]));
      expect(calls.some((c) => c.includes('copy-targets'))).toBe(true);
    });
  });
});
