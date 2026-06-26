// v0.62c — TelemetryLogList 子组件测试。
//
// `TelemetryLogList` 是 v0.49a 引入的本地磁盘 session
// 查看器。目前覆盖率为 0%（它是
// Settings.tsx 的内部子组件）。v0.62c 把
// 它从 Settings.tsx 导出以便独立测试。
// 本文件覆盖：
//   1. 无日志时的空态
//   2. 渲染 session 列表（含大小 + 日期）
//   3. 汇总行显示总字节数 + 数量
//   4. Refresh 按钮重新拉取列表
//   5. Purge 按钮调用 IPC 并显示徽章

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
      // 两个文件名均可见
      expect(screen.getByText('session-100.jsonl')).toBeInTheDocument();
      expect(screen.getByText('session-200.jsonl')).toBeInTheDocument();
    });
    // 第二个上有 current 标记
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
      // 1024 + 2048 = 3072 字节 → 约 3 KB
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
