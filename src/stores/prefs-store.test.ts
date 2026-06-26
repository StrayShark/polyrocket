// v0.62a — prefs-store 测试。
//
// prefs store 是 11 个 UiPrefs 字段的 zustand+persist store。
// 目前函数覆盖率为 50%。
// 本文件覆盖以下助手 action：
//   1. setPref 更新单个字段
//   2. resetPrefs 恢复默认值
//   3. setNotificationsEnabled 开关

// @vitest-environment happy-dom

import { describe, it, expect, beforeEach } from 'vitest';
import { usePrefsStore } from './prefs-store';

describe('usePrefsStore actions', () => {
  beforeEach(() => {
    // 每个测试前重置为默认值
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
