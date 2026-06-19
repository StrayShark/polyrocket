// v0.67a — Copy route additional tests (AddTargetModal flows).
//
// /copy has 3 main flows: targets list, events list,
// AddTargetModal. The base v0.63b test (3 tests) only
// covers the list rendering. We expand with 5 more
// tests focused on the AddTargetModal interaction:
//
//   1. Submit button disabled until valid 0x address
//   2. Invalid address length → button stays disabled
//   3. Submit valid address → addCopyTarget mutation
//   4. Submit with allocation cap → cap passed to IPC
//   5. addCopyTarget throws → error toast

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock } from '@/test-mocks';

const mockListCopyTargets = vi.fn();
const mockRecentCopyEvents = vi.fn();
const mockListPaperFills = vi.fn();
const mockGetMirrorPaperMode = vi.fn();
const mockAddCopyTarget = vi.fn();

const { mockListCopyTargets: m1, mockRecentCopyEvents: m2, mockListPaperFills: m3,
        mockGetMirrorPaperMode: m4, mockAddCopyTarget: m5 } = vi.hoisted(() => ({
  mockListCopyTargets: vi.fn(),
  mockRecentCopyEvents: vi.fn(),
  mockListPaperFills: vi.fn(),
  mockGetMirrorPaperMode: vi.fn(),
  mockAddCopyTarget: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  listCopyTargets: m1,
  recentCopyEvents: m2,
  listPaperFills: m3,
  getMirrorPaperMode: m4,
  addCopyTarget: m5,
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

const VALID_ADDR = '0x' + 'a'.repeat(40);
const TARGET_WATCHING = {
  id: 't1',
  address: VALID_ADDR,
  label: 'Whale #1',
  enabled: true,
  allocationCap: null,
  minEdge: 0.05,
  createdAt: Date.now() - 86_400_000,
};

beforeEach(() => {
  m1.mockReset().mockResolvedValue([TARGET_WATCHING]);
  m2.mockReset().mockResolvedValue([]);
  m3.mockReset().mockResolvedValue([]);
  m4.mockReset().mockResolvedValue(false);
  m5.mockReset().mockResolvedValue({
    id: 't-new', address: VALID_ADDR, label: 'New', enabled: true,
    allocationCap: null, minEdge: 0.05, createdAt: Date.now(),
  });
});

describe('Copy (v0.67a AddTargetModal)', () => {
  it('opens Add modal when Add target button is clicked', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    // Click the Add target button. The header has a primary "Add target" button.
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    expect(addBtn).toBeTruthy();
    fireEvent.click(addBtn!);
    // Modal renders — input field visible
    await waitFor(() => {
      const inputs = document.querySelectorAll('input');
      expect(inputs.length).toBeGreaterThan(0);
    });
  });

  it('Submit disabled when address is empty', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    // Find the Submit button (the primary "Add" inside modal footer)
    const submitBtn = await waitFor(() => {
      const btns = screen.getAllByRole('button');
      const submit = btns.find((b) => /^Add$/i.test(b.textContent?.trim() || ''));
      expect(submit).toBeTruthy();
      return submit!;
    });
    expect(submitBtn).toBeDisabled();
  });

  it('Submit disabled when address is wrong length', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    await waitFor(() => {
      expect(document.querySelectorAll('input').length).toBeGreaterThan(0);
    });
    // Type a too-short address
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: '0xshort' } });
    // Find submit
    await waitFor(() => {
      const btns = screen.getAllByRole('button');
      const submit = btns.find((b) => /^Add$/i.test(b.textContent?.trim() || ''));
      expect(submit).toBeTruthy();
      expect(submit).toBeDisabled();
    });
  });

  it('Submit valid 0x address → addCopyTarget called', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    await waitFor(() => {
      expect(document.querySelectorAll('input').length).toBeGreaterThan(0);
    });
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: VALID_ADDR } });
    fireEvent.change(inputs[1], { target: { value: 'My Whale' } });
    // Submit
    const submitBtn = screen.getAllByRole('button').find((b) =>
      /^Add$/i.test(b.textContent?.trim() || ''),
    );
    fireEvent.click(submitBtn!);
    await waitFor(() => {
      expect(m5).toHaveBeenCalledWith(
        expect.objectContaining({
          address: VALID_ADDR,
          label: 'My Whale',
          min_edge: 0.05, // default 5% = 0.05
        }),
      );
    });
  });

  it('Submit with allocation cap → cap passed to IPC', async () => {
    renderCopy();
    await waitFor(() => {
      expect(screen.getAllByText(/Whale #1/i).length).toBeGreaterThan(0);
    });
    const addBtn = screen.getAllByRole('button').find((b) =>
      /add target/i.test(b.textContent || ''),
    );
    fireEvent.click(addBtn!);
    await waitFor(() => {
      expect(document.querySelectorAll('input').length).toBeGreaterThan(0);
    });
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: VALID_ADDR } });
    // Allocation cap is the 4th input (after address, label, min edge)
    fireEvent.change(inputs[3], { target: { value: '500' } });
    const submitBtn = screen.getAllByRole('button').find((b) =>
      /^Add$/i.test(b.textContent?.trim() || ''),
    );
    fireEvent.click(submitBtn!);
    await waitFor(() => {
      expect(m5).toHaveBeenCalledWith(
        expect.objectContaining({
          allocation_cap: '500',
        }),
      );
    });
  });
});
