// v0.62c — TelemetryLogList sub-component tests.
//
// `TelemetryLogList` is the v0.49a on-disk session
// viewer. Today 0% coverage (it's an internal
// sub-component of Settings.tsx). v0.62c exports
// it from Settings.tsx so we can test in isolation.
// This file covers:
//   1. Empty state when no logs
//   2. Renders a list of sessions with size + date
//   3. Summary line shows total bytes + count
//   4. Refresh button re-fetches the list
//   5. Purge button calls IPC and shows badge

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';

vi.mock('@/ipc', () => ({
  listTelemetryLogs: vi.fn(),
  purgeTelemetryLogs: vi.fn(),
}));

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k }),
}));

import { TelemetryLogList } from './Settings';
import { listTelemetryLogs, purgeTelemetryLogs } from '@/ipc';

describe('TelemetryLogList', () => {
  beforeEach(() => {
    vi.mocked(listTelemetryLogs).mockReset();
    vi.mocked(purgeTelemetryLogs).mockReset();
  });

  it('shows the empty state when there are no logs', async () => {
    vi.mocked(listTelemetryLogs).mockResolvedValue([]);
    render(<TelemetryLogList />);
    await waitFor(() => {
      expect(screen.getByTestId('telemetry-logs-summary').textContent).toMatch(/0 sessions/);
    });
  });

  it('renders each session with size + modified date', async () => {
    vi.mocked(listTelemetryLogs).mockResolvedValue([
      { name: 'session-100.jsonl', path: '/x/session-100.jsonl', sizeBytes: 1024,
        modifiedUnix: 1_700_000_000, isCurrent: false },
      { name: 'session-200.jsonl', path: '/x/session-200.jsonl', sizeBytes: 4096,
        modifiedUnix: 1_700_500_000, isCurrent: true },
    ]);
    render(<TelemetryLogList />);
    await waitFor(() => {
      // Both file names visible
      expect(screen.getByText('session-100.jsonl')).toBeInTheDocument();
      expect(screen.getByText('session-200.jsonl')).toBeInTheDocument();
    });
    // Current marker on the second
    const list = screen.getByTestId('telemetry-logs-list');
    expect(list.querySelectorAll('[data-testid="telemetry-log-current"]').length).toBe(1);
  });

  it('summary line shows count and total size', async () => {
    vi.mocked(listTelemetryLogs).mockResolvedValue([
      { name: 'a.jsonl', path: '/a', sizeBytes: 1024, modifiedUnix: 1, isCurrent: false },
      { name: 'b.jsonl', path: '/b', sizeBytes: 2048, modifiedUnix: 2, isCurrent: false },
    ]);
    render(<TelemetryLogList />);
    await waitFor(() => {
      const summary = screen.getByTestId('telemetry-logs-summary').textContent ?? '';
      expect(summary).toMatch(/2 sessions/);
      // 1024 + 2048 = 3072 bytes → ~3 KB
      expect(summary).toMatch(/KB total/);
    });
  });

  it('flags the current session in the summary', async () => {
    vi.mocked(listTelemetryLogs).mockResolvedValue([
      { name: 'old.jsonl', path: '/o', sizeBytes: 1024, modifiedUnix: 1, isCurrent: false },
      { name: 'new.jsonl', path: '/n', sizeBytes: 1024, modifiedUnix: 2, isCurrent: true },
    ]);
    render(<TelemetryLogList />);
    await waitFor(() => {
      const summary = screen.getByTestId('telemetry-logs-summary').textContent ?? '';
      expect(summary).toMatch(/1 current/);
    });
  });

  it('refresh button re-fetches the list', async () => {
    vi.mocked(listTelemetryLogs).mockResolvedValue([]);
    render(<TelemetryLogList />);
    const refreshBtn = await screen.findByTestId('telemetry-logs-refresh');
    fireEvent.click(refreshBtn);
    await waitFor(() => {
      expect(listTelemetryLogs).toHaveBeenCalledTimes(2); // mount + click
    });
  });

  it('purge button calls purge IPC and shows badge', async () => {
    vi.mocked(listTelemetryLogs).mockResolvedValue([
      { name: 'a.jsonl', path: '/a', sizeBytes: 1024, modifiedUnix: 1, isCurrent: false },
    ]);
    vi.mocked(purgeTelemetryLogs).mockResolvedValue(1);
    render(<TelemetryLogList />);
    const purgeBtn = await screen.findByTestId('telemetry-logs-purge');
    fireEvent.click(purgeBtn);
    await waitFor(() => {
      expect(purgeTelemetryLogs).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByTestId('telemetry-purged-badge')).toBeInTheDocument();
    });
  });

  it('purge button is disabled when there are no logs', async () => {
    vi.mocked(listTelemetryLogs).mockResolvedValue([]);
    render(<TelemetryLogList />);
    const purgeBtn = await screen.findByTestId('telemetry-logs-purge');
    expect(purgeBtn).toBeDisabled();
  });
});
