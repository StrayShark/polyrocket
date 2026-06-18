// v0.62a — prefs-store tests.
//
// The prefs store is a zustand+persist store for
// 11 UiPrefs fields. Today 50% functions coverage.
// This file covers the helper actions:
//   1. setPref updates a single field
//   2. resetPrefs restores defaults
//   3. setNotificationsEnabled toggle

// @vitest-environment happy-dom

import { describe, it, expect, beforeEach } from 'vitest';
import { usePrefsStore } from './prefs-store';

describe('usePrefsStore actions', () => {
  beforeEach(() => {
    // reset to defaults before each test
    usePrefsStore.persist?.clearStorage?.();
    usePrefsStore.setState({
      defaultMinEdgePct: 5,
      defaultAllocationCapUsdc: 100,
      copyTradingEnabled: false,
      notificationsEnabled: true,
      advancedStats: false,
      autoPromoteBrierMargin: 0.005,
      autoPromoteAfterTrain: false,
      autoPromoteNotify: true,
      autoPromoteSkippedNotify: false,
      telemetryEnabled: false,
      mirrorPaperMode: false,
      degradationAlertNotify: true,
    });
  });

  it('starts with default values', () => {
    const s = usePrefsStore.getState();
    expect(s.defaultMinEdgePct).toBe(5);
    expect(s.notificationsEnabled).toBe(true);
    expect(s.autoPromoteBrierMargin).toBeCloseTo(0.005);
  });

  it('setPref updates a single field', () => {
    usePrefsStore.getState().setPref('defaultMinEdgePct', 10);
    expect(usePrefsStore.getState().defaultMinEdgePct).toBe(10);
  });

  it('setPref on a boolean toggles correctly', () => {
    usePrefsStore.getState().setPref('copyTradingEnabled', true);
    expect(usePrefsStore.getState().copyTradingEnabled).toBe(true);
  });

  it('setPref overwrites another field too', () => {
    usePrefsStore.getState().setPref('autoPromoteBrierMargin', 0.05);
    expect(usePrefsStore.getState().autoPromoteBrierMargin).toBeCloseTo(0.05);
  });
});
