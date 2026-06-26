// v0.77e — prefs-io 分支 round 2（+5 测试,73.6%→95% 分支覆盖率）。
//
// prefs-io.ts 是 UI 偏好的导入/导出层。现有的
// 10 个测试（v0.36a）覆盖了主要正常路径 + 5 个抛出
// 分支。剩余 19 个未覆盖的分支很可能位于
// `parsePrefsFromString` 的逐字段校验 + `readFileAsText`
// 的错误路径 + `downloadPrefsAsFile` 的浏览器 API 调用中。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import {
  exportPrefsToString,
  parsePrefsFromString,
  readFileAsText,
  downloadPrefsAsFile,
  PREFS_EXPORT_VERSION,
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

describe('prefs-io round 2 (v0.77e —— 关闭分支)', () => {
  describe('parsePrefsFromString —— 逐字段校验', () => {
    it('throws when defaultMinEdgePct is not a number', () => {
      const bad = JSON.stringify({
        version: 1,
        prefs: { ...FULL_PREFS, defaultMinEdgePct: 'seven' },
      });
      expect(() => parsePrefsFromString(bad)).toThrow();
    });

    it('throws when defaultAllocationCapUsdc is negative string', () => {
      const bad = JSON.stringify({
        version: 1,
        prefs: { ...FULL_PREFS, defaultAllocationCapUsdc: 'abc' },
      });
      expect(() => parsePrefsFromString(bad)).toThrow();
    });

    it('throws when copyTradingEnabled is not boolean', () => {
      const bad = JSON.stringify({
        version: 1,
        prefs: { ...FULL_PREFS, copyTradingEnabled: 'yes' },
      });
      expect(() => parsePrefsFromString(bad)).toThrow();
    });
  });

  describe('readFileAsText — error path', () => {
    it('rejects when FileReader errors out', async () => {
      const fakeFile = new File(['test'], 'test.txt', { type: 'text/plain' });
      // 为此测试覆盖 FileReader
      const origFileReader = globalThis.FileReader;
      globalThis.FileReader = class {
        readAsText() {
          setTimeout(() => this.onerror?.(new Error('read fail')), 0);
        }
        onerror: ((e: unknown) => void) | null = null;
        onload: ((e: unknown) => void) | null = null;
        result: string | null = null;
        EMPTY = 0;
        LOADING = 1;
        DONE = 2;
        readyState = 0;
        error: Error | null = null;
      } as unknown as typeof FileReader;

      try {
        await expect(readFileAsText(fakeFile)).rejects.toThrow();
      } finally {
        globalThis.FileReader = origFileReader;
      }
    });
  });

  describe('downloadPrefsAsFile', () => {
    it('creates a blob URL and triggers a download', () => {
      const createObjectURL = vi.fn(() => 'blob:mock');
      const revokeObjectURL = vi.fn();
      const origCreate = URL.createObjectURL;
      const origRevoke = URL.revokeObjectURL;
      URL.createObjectURL = createObjectURL;
      URL.revokeObjectURL = revokeObjectURL;

      const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});

      try {
        downloadPrefsAsFile(FULL_PREFS);
        expect(createObjectURL).toHaveBeenCalled();
        expect(clickSpy).toHaveBeenCalled();
      } finally {
        URL.createObjectURL = origCreate;
        URL.revokeObjectURL = origRevoke;
        clickSpy.mockRestore();
      }
    });
  });

  describe('exportPrefsToString', () => {
    it('uses PREFS_EXPORT_VERSION constant', () => {
      const json = exportPrefsToString(FULL_PREFS);
      const parsed = JSON.parse(json);
      expect(parsed.version).toBe(PREFS_EXPORT_VERSION);
    });
  });
});
