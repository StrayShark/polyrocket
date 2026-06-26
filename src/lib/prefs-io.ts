/**
 * L1 —— Prefs 导入/导出（v0.36a）。
 *
 * 用户可以将其 UI prefs（auto-promote
 * margin、after-train toggle 等）备份为
 * JSON 文件以备后续恢复。适用于：
 *   - 与其他用户共享首选配置
 *   - 重新安装前进行备份
 *   - 在多台机器之间复制相同配置
 *
 * 线格式是带有顶层 version 字段的
 * UiPrefs 结构：
 * ```
 * {
 *   "version": 1,
 *   "exported_at_ms": 1700000000000,
 *   "prefs": {
 *     "defaultMinEdgePct": 5,
 *     "defaultAllocationCapUsdc": 100,
 *     ...
 *   }
 * }
 * ```
 *
 * version 字段用于未来的迁移。如果
 * 在 v1.5 中新增一个 pref，旧的（v1）
 * 导出会缺少它；导入器将使用新 pref
 * 的默认值。
 *
 * 导出器只包含 7 个 UiPrefs 字段，
 * **不**包含内部 store 状态（setPref /
 * reset 函数）。这些不可序列化。
 */
import type { UiPrefs } from '@/stores/prefs-store';

export const PREFS_EXPORT_VERSION = 1;

export interface PrefsExport {
  /** v0.36a —— 格式版本。目前始终为 1。 */
  version: number;
  /** v0.36a —— 导出的创建时间（unix 毫秒）。 */
  exported_at_ms: number;
  /** v0.36a —— 实际的 prefs。全部 11 个
   *  UiPrefs 字段（v0.44c 之前是 10 个，
   *  新增了 mirrorPaperMode）。 */
  prefs: UiPrefs;
}

/** 每个 pref 的默认值。当导入中缺少
 *  某字段时使用（例如旧导出中包含
 *  导出时还不存在的新字段 —— 或反之）。 */
const PREF_DEFAULTS: UiPrefs = {
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

/**
 * v0.36a —— 将 prefs 导出为 JSON 字符串。
 * 输出是完整的 PrefsExport 封装。
 */
export function exportPrefsToString(prefs: UiPrefs): string {
  const envelope: PrefsExport = {
    version: PREFS_EXPORT_VERSION,
    exported_at_ms: Date.now(),
    prefs: { ...prefs },
  };
  return JSON.stringify(envelope, null, 2);
}

/**
 * v0.36a —— 将 JSON 字符串解析为已校验的
 * UiPrefs。输入无效（未知版本、缺失
 * 必填字段、类型错误）时抛出错误。
 * 面向用户的错误消息是友好的；
 * 面向开发者的错误消息包含字段名。
 *
 * 策略：
 *  1. JSON.parse → unknown
 *  2. 校验它是包含 version + prefs 的对象
 *  3. 对每个已知 pref，校验类型
 *     并复制值。缺失的字段回退
 *     到默认值（向前兼容）。
 *  4. 拒绝未知版本（向后不兼容）
 */
export function parsePrefsFromString(json: string): UiPrefs {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch (e) {
    throw new Error(
      `Invalid JSON: ${e instanceof Error ? e.message : String(e)}`,
    );
  }
  if (typeof parsed !== 'object' || parsed === null) {
    throw new Error('Invalid export: top-level value must be an object');
  }
  const obj = parsed as Record<string, unknown>;
  if (typeof obj.version !== 'number') {
    throw new Error('Invalid export: missing or invalid "version" field');
  }
  if (obj.version !== PREFS_EXPORT_VERSION) {
    throw new Error(
      `Unsupported version: got ${obj.version}, expected ${PREFS_EXPORT_VERSION}`,
    );
  }
  if (typeof obj.prefs !== 'object' || obj.prefs === null) {
    throw new Error('Invalid export: missing or invalid "prefs" field');
  }
  const rawPrefs = obj.prefs as Record<string, unknown>;
  // 校验每个已知字段，缺失时
  // 回退到默认值；类型错误则拒绝。
  const result: UiPrefs = { ...PREF_DEFAULTS };
  if ('defaultMinEdgePct' in rawPrefs) {
    const v = rawPrefs.defaultMinEdgePct;
    if (typeof v !== 'number' || !Number.isFinite(v)) {
      throw new Error('Invalid export: defaultMinEdgePct must be a finite number');
    }
    result.defaultMinEdgePct = v;
  }
  if ('defaultAllocationCapUsdc' in rawPrefs) {
    const v = rawPrefs.defaultAllocationCapUsdc;
    if (typeof v !== 'number' || !Number.isFinite(v)) {
      throw new Error('Invalid export: defaultAllocationCapUsdc must be a finite number');
    }
    result.defaultAllocationCapUsdc = v;
  }
  if ('copyTradingEnabled' in rawPrefs) {
    const v = rawPrefs.copyTradingEnabled;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: copyTradingEnabled must be a boolean');
    }
    result.copyTradingEnabled = v;
  }
  if ('notificationsEnabled' in rawPrefs) {
    const v = rawPrefs.notificationsEnabled;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: notificationsEnabled must be a boolean');
    }
    result.notificationsEnabled = v;
  }
  if ('advancedStats' in rawPrefs) {
    const v = rawPrefs.advancedStats;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: advancedStats must be a boolean');
    }
    result.advancedStats = v;
  }
  if ('autoPromoteBrierMargin' in rawPrefs) {
    const v = rawPrefs.autoPromoteBrierMargin;
    if (typeof v !== 'number' || !Number.isFinite(v)) {
      throw new Error('Invalid export: autoPromoteBrierMargin must be a finite number');
    }
    result.autoPromoteBrierMargin = v;
  }
  if ('autoPromoteAfterTrain' in rawPrefs) {
    const v = rawPrefs.autoPromoteAfterTrain;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: autoPromoteAfterTrain must be a boolean');
    }
    result.autoPromoteAfterTrain = v;
  }
  // v0.39b — autoPromoteNotify。v0.39 引入;旧
  // 导出不含此字段（向前兼容：默认
  // 为 true）。
  if ('autoPromoteNotify' in rawPrefs) {
    const v = rawPrefs.autoPromoteNotify;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: autoPromoteNotify must be a boolean');
    }
    result.autoPromoteNotify = v;
  }
  // v0.42c — telemetryEnabled。v0.42 引入;旧
  // 导出不含此字段（向前兼容：默认
  // 为 false）。
  if ('telemetryEnabled' in rawPrefs) {
    const v = rawPrefs.telemetryEnabled;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: telemetryEnabled must be a boolean');
    }
    result.telemetryEnabled = v;
  }
  // v0.42e-2 — autoPromoteSkippedNotify。v0.42 引入;
  // 旧导出不含此字段（向前兼容：
  // 默认为 false）。
  if ('autoPromoteSkippedNotify' in rawPrefs) {
    const v = rawPrefs.autoPromoteSkippedNotify;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: autoPromoteSkippedNotify must be a boolean');
    }
    result.autoPromoteSkippedNotify = v;
  }
  // v0.44c — mirrorPaperMode。v0.44 引入;旧
  // 导出不含此字段（向前兼容：
  // 默认为 false）。
  if ('mirrorPaperMode' in rawPrefs) {
    const v = rawPrefs.mirrorPaperMode;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: mirrorPaperMode must be a boolean');
    }
    result.mirrorPaperMode = v;
  }
  // v0.48b — degradationAlertNotify。v0.48 引入;
  // 旧导出不含此字段
  // （向前兼容：默认为 true）。
  if ('degradationAlertNotify' in rawPrefs) {
    const v = rawPrefs.degradationAlertNotify;
    if (typeof v !== 'boolean') {
      throw new Error('Invalid export: degradationAlertNotify must be a boolean');
    }
    result.degradationAlertNotify = v;
  }
  return result;
}

/**
 * v0.36a —— 触发浏览器将 prefs 下载为
 * JSON 文件。创建 Blob、anchor，点击它，
 * 然后清理。文件名为
 * `polyrocket-prefs-YYYYMMDD.json`。
 */
export function downloadPrefsAsFile(prefs: UiPrefs): void {
  const json = exportPrefsToString(prefs);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const date = new Date();
  const yyyy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, '0');
  const dd = String(date.getDate()).padStart(2, '0');
  const filename = `polyrocket-prefs-${yyyy}${mm}${dd}.json`;
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  // 在下一个 tick 撤销 URL（点击
  // 已经发出，现在撤销太早）
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

/**
 * v0.36a —— 读取一个 File 对象并返回其文本
 * 内容。供 Import 按钮使用。拒绝
 * 非文本文件（导出始终是 JSON 即
 * 文本）。
 */
export function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result === 'string') {
        resolve(result);
      } else {
        reject(new Error('File contents are not text'));
      }
    };
    reader.onerror = () => reject(reader.error ?? new Error('Unknown read error'));
    reader.readAsText(file);
  });
}
