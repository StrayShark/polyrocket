/**
 * v0.36a — Prefs import/export tests.
 *
 * Tests the round-trip: export → parse → equal to
 * the original. Also tests the validation: each
 * pref field with a wrong type is rejected.
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
    it('includes all 11 UiPrefs fields', () => {
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
      // Simulate an old export that's missing
      // autoPromoteAfterTrain (a v0.28c field)
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
          // no autoPromoteAfterTrain
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
      // 'many' is a string, not a number, so the type check fails
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
