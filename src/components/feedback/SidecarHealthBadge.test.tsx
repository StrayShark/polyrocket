// v0.77a — SidecarHealthBadge 分支第 1 轮 (+6 tests, 0%→100% stmts, 0%→100% br)。
//
// SidecarHealthBadge.tsx 共 82 行、13 个分支 (3 个状态 ×
// 2 个成功/失败 + busy/click 处理函数)。当前 0% 覆盖率 —
// 现有测试均未挂载它。我们新增 6 个测试
// 覆盖所有状态分支 + click 行为。
//
// 覆盖率目标: 0/13 → 13/13 分支 = 100% br。
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
