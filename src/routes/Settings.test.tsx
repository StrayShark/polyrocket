/**
 * v0.28d — AutoPromoteCard behavior tests.
 *
 * Tests the new "Auto-run after train" toggle and
 * the new setAutoPromoteConfig push behavior on
 * margin save.
 *
 * v0.28c added:
 *  - A Toggle for `autoPromoteAfterTrain` in the
 *    AutoPromoteCard. Toggling it pushes the new
 *    value to Rust via `setAutoPromoteConfig({enabled})`.
 *  - The existing margin Save button also pushes
 *    `{brier_margin}` to Rust in addition to
 *    updating the zustand store.
 *
 * These tests mock `@/ipc` and `@/stores/prefs-store`
 * to verify the IPC is called with the right args.
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

// Mock the IPC layer. We assert on the auto-promote
// config push; the other IPCs used by Settings are
// stubbed to prevent network calls.
vi.mock('@/ipc', () => ({
  getAuditRetention: vi.fn(),
  setAuditRetention: vi.fn(),
  purgeAuditLogNow: vi.fn(),
  setAutoPromoteConfig: vi.fn(),
  setTelemetryEnabled: vi.fn().mockResolvedValue(true),
  getTelemetryEnabled: vi.fn().mockResolvedValue(false),
  setMirrorPaperMode: vi.fn().mockResolvedValue(true),
  getMirrorPaperMode: vi.fn().mockResolvedValue(false),
}));

// Mock the prefs store with a controllable in-memory
// value. The real store uses zustand+localStorage,
// which is annoying to reset between tests. We
// support both `usePrefsStore()` (no selector —
// returns the whole state) and `usePrefsStore(sel)`
// (with selector).
const mockSetPref = vi.fn();
const mockPrefsState = {
  defaultMinEdgePct: 5,
  defaultAllocationCapUsdc: 100,
  copyTradingEnabled: false,
  notificationsEnabled: true,
  advancedStats: false,
  autoPromoteBrierMargin: 0.005,
  autoPromoteAfterTrain: false,
  autoPromoteNotify: true,
  // v0.48b — model degradation alert
  degradationAlertNotify: true,
  setPref: mockSetPref,
  reset: vi.fn(),
};
vi.mock('@/stores/prefs-store', () => ({
  usePrefsStore: (selector?: (s: typeof mockPrefsState) => unknown) => {
    if (typeof selector === 'function') return selector(mockPrefsState);
    return mockPrefsState;
  },
}));

// Mock the toast store to avoid side effects
vi.mock('@/stores/toast-store', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

// Minimal i18n shim — the real useT pulls from a
// Zustand store. The test only checks the
// behavior, not the strings.
vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

import { setAutoPromoteConfig } from '@/ipc';
import { Settings } from '@/routes/Settings';

describe('AutoPromoteCard — v0.28c toggle + save (v0.28d)', () => {
  beforeEach(() => {
    vi.mocked(setAutoPromoteConfig).mockReset();
    vi.mocked(setAutoPromoteConfig).mockResolvedValue({
      enabled: false,
      brier_margin: 0.005,
    });
    mockSetPref.mockReset();
  });

  it('renders the "Auto-run after train" toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(
        screen.getByTestId('auto-promote-after-train-toggle'),
      ).toBeInTheDocument();
    });
  });

  it('pushes { enabled: true } when the user toggles ON', async () => {
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId(
      'auto-promote-after-train-toggle',
    );
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: true }),
      );
    });
    // Also updates the local zustand store
    expect(mockSetPref).toHaveBeenCalledWith(
      'autoPromoteAfterTrain',
      true,
    );
  });

  it('pushes { enabled: false } when the user toggles OFF', async () => {
    // Re-mock the store to start with afterTrain=true
    mockPrefsState.autoPromoteAfterTrain = true;
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId(
      'auto-promote-after-train-toggle',
    );
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalledWith(
        expect.objectContaining({ enabled: false }),
      );
    });
    expect(mockSetPref).toHaveBeenCalledWith(
      'autoPromoteAfterTrain',
      false,
    );
    mockPrefsState.autoPromoteAfterTrain = false; // reset
  });

  it('Save button pushes { brier_margin } to Rust', async () => {
    render(wrap(<Settings />));
    const saveBtn = await screen.findByTestId('auto-promote-save-btn');
    fireEvent.click(saveBtn);
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalledWith(
        expect.objectContaining({ brier_margin: 0.005 }),
      );
    });
    // Also updates the local zustand store
    expect(mockSetPref).toHaveBeenCalledWith(
      'autoPromoteBrierMargin',
      0.005,
    );
  });

  it('mounts push the current config to Rust (one-time)', async () => {
    render(wrap(<Settings />));
    // The useEffect on mount calls setAutoPromoteConfig
    // with the current persisted values. Wait for it.
    await waitFor(() => {
      expect(setAutoPromoteConfig).toHaveBeenCalled();
    });
    // Check it was called with both fields at least once
    const calls = vi.mocked(setAutoPromoteConfig).mock.calls;
    const fullPush = calls.find(
      (c) =>
        typeof c[0]?.enabled === 'boolean' &&
        typeof c[0]?.brier_margin === 'number',
    );
    expect(fullPush).toBeDefined();
  });
});

// =================================================================
// =================== v0.36b — Backup & restore card =================
// =================================================================

describe('Backup & restore card (v0.36b)', () => {
  beforeEach(() => {
    vi.mocked(setAutoPromoteConfig).mockReset();
    vi.mocked(setAutoPromoteConfig).mockResolvedValue({
      enabled: false,
      brier_margin: 0.005,
    });
    mockSetPref.mockReset();
  });

  it('renders the Backup & restore card with Export and Import buttons', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('backup-restore-card')).toBeInTheDocument();
    });
    expect(screen.getByTestId('backup-export-btn')).toBeInTheDocument();
    expect(screen.getByTestId('backup-import-btn')).toBeInTheDocument();
  });

  it('Export button triggers a download (spy on anchor click)', async () => {
    // Spy on document.createElement to capture the
    // anchor element used for the download
    const realCreate = document.createElement.bind(document);
    let capturedAnchor: HTMLAnchorElement | null = null;
    vi.spyOn(document, 'createElement').mockImplementation((tag) => {
      const el = realCreate(tag);
      if (tag === 'a') capturedAnchor = el as HTMLAnchorElement;
      return el;
    });
    render(wrap(<Settings />));
    const exportBtn = await screen.findByTestId('backup-export-btn');
    fireEvent.click(exportBtn);
    expect(capturedAnchor).not.toBeNull();
    expect(capturedAnchor!.download).toMatch(/^polyrocket-prefs-\d{8}\.json$/);
    // Restore
    vi.mocked(document.createElement).mockRestore();
  });

  // v0.39b — the auto-promote desktop-notification toggle
  it('renders the auto-promote-notify toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('auto-promote-notify-toggle')).toBeInTheDocument();
    });
  });

  it('toggling auto-promote-notify updates the prefs store', async () => {
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('auto-promote-notify-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(mockSetPref).toHaveBeenCalledWith('autoPromoteNotify', false);
    });
  });

  // v0.42c — telemetry opt-in toggle
  it('renders the telemetry card with a toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('telemetry-toggle')).toBeInTheDocument();
    });
  });

  it('toggling telemetry ON pushes { enabled: true } to Rust', async () => {
    const { setTelemetryEnabled } = await import('@/ipc');
    vi.mocked(setTelemetryEnabled).mockClear();
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('telemetry-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setTelemetryEnabled).toHaveBeenCalledWith({ enabled: true });
    });
  });

  it('mount pushes the current effective state from Rust (sync path)', async () => {
    const { getTelemetryEnabled } = await import('@/ipc');
    vi.mocked(getTelemetryEnabled).mockClear();
    vi.mocked(getTelemetryEnabled).mockResolvedValue(true);
    render(wrap(<Settings />));
    // The mount effect calls getTelemetryEnabled
    // and pushes the value into the prefs store if
    // it differs.
    await waitFor(() => {
      expect(getTelemetryEnabled).toHaveBeenCalled();
    });
  });

  // v0.44c — paper mode toggle
  it('renders the mirror-paper-mode toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('mirror-paper-mode-toggle')).toBeInTheDocument();
    });
  });

  it('toggling paper mode ON pushes { enabled: true } to Rust', async () => {
    const { setMirrorPaperMode } = await import('@/ipc');
    vi.mocked(setMirrorPaperMode).mockClear();
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('mirror-paper-mode-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(setMirrorPaperMode).toHaveBeenCalledWith({ enabled: true });
    });
  });

  // v0.42e-2 — opt-in OS notification for the
  // skipped auto-promote branch.
  it('renders the auto-promote-notify-skipped toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(
        screen.getByTestId('auto-promote-notify-skipped-toggle'),
      ).toBeInTheDocument();
    });
  });

  it('toggling auto-promote-notify-skipped updates the prefs store', async () => {
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('auto-promote-notify-skipped-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(mockSetPref).toHaveBeenCalledWith('autoPromoteSkippedNotify', true);
    });
  });

  // v0.48b — model degradation alert toggle
  it('renders the degradation-alert toggle', async () => {
    render(wrap(<Settings />));
    await waitFor(() => {
      expect(screen.getByTestId('degradation-alert-toggle')).toBeInTheDocument();
    });
  });

  it('toggling degradation alert updates the prefs store', async () => {
    mockSetPref.mockClear();
    render(wrap(<Settings />));
    const toggle = await screen.findByTestId('degradation-alert-toggle');
    fireEvent.click(toggle);
    await waitFor(() => {
      expect(mockSetPref).toHaveBeenCalledWith('degradationAlertNotify', false);
    });
  });
});
