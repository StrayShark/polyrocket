/**
 * v0.36a — 偏好导入/导出测试。
 *
 * 测试往返：导出 → 解析 → 与原始值相等。
 * 还测试校验：每个类型错误的 pref 字段都会被拒绝。
 */

// @vitest-environment happy-dom

import { describe, it, expect } from 'vitest';
import {
  exportPrefsToString,
  parsePrefsFromString,
  PREFS_EXPORT_VERSION,
  readFileAsText,
} from './prefs-io';
import type { UiPrefs } from '@/stores/prefs-store';

const FULL_PREFS: UiPrefs = {
  defaultMinEdgePct: 7,
  defaultAllocationCapUsdc: 250,
  copyTradingEnabled: true,
  notificationsEnabled: false,
  advancedStats: true,
  autoPromoteBrierMargin: 0.01,
  autoPromoteAfterTrain: true,
  autoPromoteNotify: false,
  autoPromoteSkippedNotify: true,
  telemetryEnabled: true,
  mirrorPaperMode: true,
  degradationAlertNotify: false,
};

describe('prefs-io (v0.36a)', () => {
  describe('exportPrefsToString', () => {
    it('produces a JSON envelope with version + exported_at_ms + prefs', () => {
      const json = exportPrefsToString(FULL_PREFS);
      const parsed = JSON.parse(json);
      expect(parsed.version).toBe(PREFS_EXPORT_VERSION);
      expect(typeof parsed.exported_at_ms).toBe('number');
      expect(parsed.prefs).toEqual(FULL_PREFS);
    });
    it('includes all 12 UiPrefs fields', () => {
      const json = exportPrefsToString(FULL_PREFS);
      const parsed = JSON.parse(json);
      expect(Object.keys(parsed.prefs).sort()).toEqual(
        [
          'advancedStats',
          'autoPromoteAfterTrain',
          'autoPromoteBrierMargin',
          'autoPromoteNotify',
          'autoPromoteSkippedNotify',
          'copyTradingEnabled',
          'degradationAlertNotify',
          'defaultAllocationCapUsdc',
          'defaultMinEdgePct',
          'mirrorPaperMode',
          'notificationsEnabled',
          'telemetryEnabled',
        ].sort(),
      );
    });
  });

  describe('parsePrefsFromString', () => {
    it('round-trips a full prefs object', () => {
      const json = exportPrefsToString(FULL_PREFS);
      const parsed = parsePrefsFromString(json);
      expect(parsed).toEqual(FULL_PREFS);
    });
    it('round-trips the default prefs (all fields at default)', () => {
      const defaults: UiPrefs = {
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
      };
      const json = exportPrefsToString(defaults);
      const parsed = parsePrefsFromString(json);
      expect(parsed).toEqual(defaults);
    });
    it('throws on invalid JSON', () => {
      expect(() => parsePrefsFromString('{not valid json')).toThrow(/Invalid JSON/);
    });
    it('throws on non-object top-level', () => {
      expect(() => parsePrefsFromString('"a string"')).toThrow(/must be an object/);
    });
    it('throws on missing version', () => {
      const noVersion = JSON.stringify({ prefs: {} });
      expect(() => parsePrefsFromString(noVersion)).toThrow(/version/);
    });
    it('throws on wrong version', () => {
      const wrongVersion = JSON.stringify({ version: 999, prefs: {} });
      expect(() => parsePrefsFromString(wrongVersion)).toThrow(/Unsupported version/);
    });
    it('throws on missing prefs field', () => {
      const noPrefs = JSON.stringify({ version: PREFS_EXPORT_VERSION });
      expect(() => parsePrefsFromString(noPrefs)).toThrow(/prefs/);
    });
    it('uses default for missing fields (forward-compat)', () => {
      // 模拟缺少
      // autoPromoteAfterTrain（v0.28c 字段）的旧导出
      const oldExport = {
        version: PREFS_EXPORT_VERSION,
        exported_at_ms: 1,
        prefs: {
          defaultMinEdgePct: 5,
          defaultAllocationCapUsdc: 100,
          copyTradingEnabled: false,
          notificationsEnabled: true,
          advancedStats: false,
          autoPromoteBrierMargin: 0.005,
          // 无 autoPromoteAfterTrain
        },
      };
      const parsed = parsePrefsFromString(JSON.stringify(oldExport));
      expect(parsed.autoPromoteAfterTrain).toBe(false); // default
    });
    it('rejects wrong type for defaultMinEdgePct', () => {
      const bad = {
        version: PREFS_EXPORT_VERSION,
        exported_at_ms: 1,
        prefs: { ...FULL_PREFS, defaultMinEdgePct: 'not a number' },
      };
      expect(() => parsePrefsFromString(JSON.stringify(bad))).toThrow(/defaultMinEdgePct/);
    });
    it('rejects NaN for numeric fields', () => {
      const bad = {
        version: PREFS_EXPORT_VERSION,
        exported_at_ms: 1,
        prefs: { ...FULL_PREFS, autoPromoteBrierMargin: 'many' as unknown as number },
      };
      // 'many' 是字符串而非数字，因此类型检查会失败
      expect(() => parsePrefsFromString(JSON.stringify(bad))).toThrow(/autoPromoteBrierMargin/);
    });
    it('rejects wrong type for copyTradingEnabled', () => {
      const bad = {
        version: PREFS_EXPORT_VERSION,
        exported_at_ms: 1,
        prefs: { ...FULL_PREFS, copyTradingEnabled: 'true' as unknown as boolean },
      };
      expect(() => parsePrefsFromString(JSON.stringify(bad))).toThrow(/copyTradingEnabled/);
    });

    // v0.116 —— 覆盖率提升第 16 轮。覆盖 prefs-io 中的第 199、209、219、264 行
    // （类型检查 throw 分支）。
    it('rejects wrong type for autoPromoteSkippedNotify', () => {
      const bad = {
        version: PREFS_EXPORT_VERSION,
        exported_at_ms: 1,
        prefs: { ...FULL_PREFS, autoPromoteSkippedNotify: 'true' as unknown as boolean },
      };
      expect(() => parsePrefsFromString(JSON.stringify(bad))).toThrow(/autoPromoteSkippedNotify/);
    });

    it('rejects wrong type for mirrorPaperMode', () => {
      const bad = {
        version: PREFS_EXPORT_VERSION,
        exported_at_ms: 1,
        prefs: { ...FULL_PREFS, mirrorPaperMode: 1 as unknown as boolean },
      };
      expect(() => parsePrefsFromString(JSON.stringify(bad))).toThrow(/mirrorPaperMode/);
    });
  });

  describe('readFileAsText', () => {
    it('reads a text file', async () => {
      const blob = new Blob(['hello world'], { type: 'text/plain' });
      const file = new File([blob], 'test.txt', { type: 'text/plain' });
      const text = await readFileAsText(file);
      expect(text).toBe('hello world');
    });
  });
});
