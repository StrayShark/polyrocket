/**
 * L1 — Prefs import/export (v0.36a).
 *
 * The user can back up their UI prefs (auto-promote
 * margin, after-train toggle, etc.) to a JSON file
 * and restore it later. Useful for:
 *   - Sharing a preferred config with other users
 *   - Backup before a re-install
 *   - Replicating the same config across multiple
 *     machines
 *
 * The wire format is the UiPrefs shape with a
 * version field at the top:
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
 * The version field is for future migrations. If we
 * add a new pref in v1.5, an old export (v1) would
 * be missing it; the importer would use the default
 * for the new pref.
 *
 * The exporter only includes the 7 UiPrefs fields,
 * NOT the internal store state (setPref / reset
 * functions). Those are not serializable.
 */
import type { UiPrefs } from '@/stores/prefs-store';

export const PREFS_EXPORT_VERSION = 1;

export interface PrefsExport {
  /** v0.36a — format version. Always 1 for now. */
  version: number;
  /** v0.36a — when the export was created (unix millis). */
  exported_at_ms: number;
  /** v0.36a — the actual prefs. All 7 UiPrefs fields. */
  prefs: UiPrefs;
}

/** Default values for each pref. Used when an import
 *  is missing a field (e.g. an old export with a
 *  newer field that didn't exist when the export
 *  was made — or vice versa). */
const PREF_DEFAULTS: UiPrefs = {
  defaultMinEdgePct: 5,
  defaultAllocationCapUsdc: 100,
  copyTradingEnabled: false,
  notificationsEnabled: true,
  advancedStats: false,
  autoPromoteBrierMargin: 0.005,
  autoPromoteAfterTrain: false,
};

/**
 * v0.36a — export prefs to a JSON string.
 * The output is the full PrefsExport envelope.
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
 * v0.36a — parse a JSON string into a validated
 * UiPrefs. Throws on invalid input (unknown version,
 * missing required fields, wrong types). The user-
 * facing error message is friendly; the developer-
 * facing error message includes the field name.
 *
 * Strategy:
 *  1. JSON.parse → unknown
 *  2. Validate it's an object with version + prefs
 *  3. For each known pref, validate the type
 *     and copy the value. Missing fields fall
 *     back to defaults (forward-compat).
 *  4. Reject unknown version (backward-incompat)
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
  // Validate each known field, fall back to defaults
  // for missing ones. Reject wrong types.
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
  return result;
}

/**
 * v0.36a — trigger a browser download of the prefs
 * as a JSON file. Creates a Blob, an anchor, clicks
 * it, then cleans up. The filename is
 * `polyrocket-prefs-YYYYMMDD.json`.
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
  // Revoke the URL on the next tick (the click is
  // already in flight; revoking now is too early)
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

/**
 * v0.36a — read a File object and return its text
 * contents. Used by the Import button. Rejects
 * non-text files (the export is always JSON which
 * is text).
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
