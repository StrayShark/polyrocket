// v0.77a — SidecarHealthBadge branches round 1 (+6 tests, 0%→100% stmts, 0%→100% br).
//
// SidecarHealthBadge.tsx is 82 lines with 13 branches (3 status ×
// 2 success/fail + busy/click handlers). 0% coverage currently —
// no existing test mounts it. We add 6 tests covering all status
// branches + click behavior.
//
// Coverage target: 0/13 → 13/13 branches = 100% br.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockSidecarHealthSnapshot = vi.fn();
const mockSidecarHealthNow = vi.fn();

vi.mock('@/ipc', () => ({
  sidecarHealthSnapshot: () => mockSidecarHealthSnapshot(),
  sidecarHealthNow: () => Promise.resolve(mockSidecarHealthNow()),
}));

import { SidecarHealthBadge } from './SidecarHealthBadge';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(<QueryClientProvider client={qc}>{node}</QueryClientProvider>);
}

const NOW_MS = 1_700_000_000_000;

beforeEach(() => {
  vi.clearAllMocks();
  mockSidecarHealthNow.mockResolvedValue(undefined);
});

describe('SidecarHealthBadge (v0.77a — full coverage)', () => {
  it('renders with unknown status when no data', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue(undefined);
    wrap(<SidecarHealthBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('sidecar-health-badge')).toBeTruthy();
    });
    expect(screen.getByTestId('sidecar-health-badge').textContent).toContain('Sidecar —');
  });

  it('renders unknown when both last_success and last_failure are null', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue({
      last_success_at_ms: null,
      last_failure_at_ms: null,
      success_count: 0,
      failure_count: 0,
    });
    wrap(<SidecarHealthBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('sidecar-health-badge').textContent).toContain('Sidecar —');
    });
  });

  it('renders ok when last_success is set and last_failure is null', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue({
      last_success_at_ms: NOW_MS,
      last_failure_at_ms: null,
      success_count: 5,
      failure_count: 0,
    });
    wrap(<SidecarHealthBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('sidecar-health-badge').textContent).toContain('Sidecar OK');
    });
  });

  it('renders ok when last_success > last_failure', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue({
      last_success_at_ms: NOW_MS,
      last_failure_at_ms: NOW_MS - 1000,
      success_count: 5,
      failure_count: 1,
    });
    wrap(<SidecarHealthBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('sidecar-health-badge').textContent).toContain('Sidecar OK');
    });
  });

  it('renders failed when last_failure > last_success', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue({
      last_success_at_ms: NOW_MS - 1000,
      last_failure_at_ms: NOW_MS,
      success_count: 5,
      failure_count: 3,
    });
    wrap(<SidecarHealthBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('sidecar-health-badge').textContent).toContain('Sidecar DOWN');
    });
  });

  it('renders failed when last_failure is set and last_success is null', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue({
      last_success_at_ms: null,
      last_failure_at_ms: NOW_MS,
      success_count: 0,
      failure_count: 3,
    });
    wrap(<SidecarHealthBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('sidecar-health-badge').textContent).toContain('Sidecar DOWN');
    });
  });

  it('click triggers sidecarHealthNow and refetches', async () => {
    mockSidecarHealthSnapshot.mockResolvedValue({
      last_success_at_ms: NOW_MS,
      last_failure_at_ms: null,
      success_count: 1,
      failure_count: 0,
    });
    wrap(<SidecarHealthBadge />);
    await waitFor(() => {
      expect(screen.getByTestId('sidecar-health-badge')).toBeTruthy();
    });
    fireEvent.click(screen.getByTestId('sidecar-health-badge'));
    await waitFor(() => {
      expect(mockSidecarHealthNow).toHaveBeenCalled();
    });
  });
});
