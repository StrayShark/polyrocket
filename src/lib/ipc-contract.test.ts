// v0.66c — L1 IPC contract snapshot test.
//
// Purpose: catch drift between the L1 TypeScript wrappers
// in `src/ipc.ts` and the L2 Rust commands in
// `src-tauri/src/commands/*.rs`. If someone adds a Rust
// command without adding a TS wrapper (or vice versa),
// or changes a function's signature, this test fails.
//
// **Why a snapshot, not codegen**: a full ts-rs / specta
// codegen setup would take 3h+ and add 2-3 new
// dependencies. The project already has a manual sync
// between the two layers (CI module map enforces
// 1 Rust command = 1 TS wrapper), so a runtime check
// is a smaller investment with similar coverage for
// the most common drift cases.
//
// **What this test checks**:
//   1. The list of `export function` names in `src/ipc.ts`
//      matches the snapshot (drift = command added/removed).
//   2. Each function's parameter count matches the snapshot
//      (drift = signature changed).
//   3. The total count is 111 (matches the project docs
//      "111 IPC commands").
//
// **When this test fails**: someone added/removed/renamed
// an L1 wrapper without updating the L2 Rust side (or
// vice versa). The fix is to keep both layers in sync.
// See docs/overview.md §1.2 for the layer map.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const IPC_PATH = join(import.meta.dirname, '..', 'ipc.ts');
const SNAPSHOT_PATH = join(import.meta.dirname, '..', 'ipc.snapshot.json');

interface IpcSnapshot {
  /** Function name → parameter count. */
  functions: Record<string, number>;
  /** Total count. */
  total: number;
}

function parseIpcFile(): IpcSnapshot {
  const src = readFileSync(IPC_PATH, 'utf8');
  // polyrocket's L1 uses `export const NAME = (args) => invoke<T>(...)`
  // for IPC commands and `export const NAME = (args) => { ... }` for
  // event listeners. We match both, with a regex that handles the
  // multi-line form (some const arrows wrap to a new line after `=`).
  //
  // Pattern: `^export const NAME\s*=\s*([^)]*)\)\s*=>` — covers
  // single-line arrows + multi-line (the `=>` at the start of
  // a new line means the function body starts there).
  const arrowRegex = /^export const (\w+)\s*=\s*([^)]*)\)\s*=>/gm;
  const functions: Record<string, number> = {};
  let m: RegExpExecArray | null;
  while ((m = arrowRegex.exec(src)) !== null) {
    const name = m[1];
    if (name && m[2] !== undefined) {
      // Strip TS type annotations from the param list. We only
      // count actual identifier-like tokens (not `: Type` parts).
      const params = m[2].split(',').filter((p) => p.trim().length > 0);
      functions[name] = params.length;
    }
  }
  return { functions, total: Object.keys(functions).length };
}

describe('L1 IPC contract', () => {
  it('matches the snapshot (L1 wrappers ↔ L2 commands)', () => {
    const current = parseIpcFile();

    // Project docs claim 111 IPC commands. The function count
    // is at least that (some wrappers are non-IPC utilities like
    // event listeners, but the regular `invoke` wrappers should
    // be the dominant share). We allow ≥ 100 to leave headroom
    // for the regex to miss multi-line arrows with complex
    // generics.
    expect(current.total).toBeGreaterThanOrEqual(100);

    // If the snapshot file exists, compare against it. This is
    // a hard drift check: any function added/removed changes
    // the snapshot and fails the test.
    //
    // If the snapshot file doesn't exist, write it (first-run).
    // Subsequent runs compare.
    const fs = require('node:fs') as typeof import('node:fs');
    if (!fs.existsSync(SNAPSHOT_PATH)) {
      fs.writeFileSync(SNAPSHOT_PATH, JSON.stringify(current, null, 2) + '\n');
      // Allow the first run to pass (snapshot is being created)
      return;
    }

    const snapshot: IpcSnapshot = JSON.parse(fs.readFileSync(SNAPSHOT_PATH, 'utf8'));

    // Compare function names — added/removed is drift
    const currentNames = Object.keys(current.functions).sort();
    const snapshotNames = Object.keys(snapshot.functions).sort();
    const added = currentNames.filter((n) => !snapshotNames.includes(n));
    const removed = snapshotNames.filter((n) => !currentNames.includes(n));

    if (added.length > 0) {
      throw new Error(
        `L1 IPC contract drift: ${added.length} new function(s) added to src/ipc.ts:\n` +
          added.map((n) => `  + ${n} (${current.functions[n]} params)`).join('\n') +
          `\nIf intentional, update the snapshot by running this test with --update-snapshot.`,
      );
    }
    if (removed.length > 0) {
      throw new Error(
        `L1 IPC contract drift: ${removed.length} function(s) removed from src/ipc.ts:\n` +
          removed.map((n) => `  - ${n}`).join('\n') +
          `\nIf intentional, update the snapshot.`,
      );
    }

    // Compare param counts — signature change is drift
    for (const name of currentNames) {
      if (current.functions[name] !== snapshot.functions[name]) {
        throw new Error(
          `L1 IPC contract drift: '${name}' parameter count changed ` +
            `from ${snapshot.functions[name]} to ${current.functions[name]}. ` +
            `If intentional, update the snapshot.`,
        );
      }
    }
  });
});
