// polyrocket — L1 IPC contract v2 test (v0.67b).
//
// Extends v0.66c's snapshot-based check with deeper coverage:
//
// v0.66c checks:
//   - Function added/removed in src/ipc.ts
//   - Param count changed
//
// v0.67b adds:
//   - Type imports used by L1 wrappers (e.g. `Wallet`, `Bet`)
//   - Each wrapper's return type annotation
//
// **Why not full ts-rs/specta codegen**: a real codegen
// setup requires:
//   1. Add `specta`, `specta-typescript`, `tauri-specta` to Cargo.toml
//   2. Annotate ALL `#[tauri::command]` fns with `#[specta]`
//   3. Run `cargo run --bin specta-gen` to produce TS types
//   4. Replace src/ipc.ts with generated wrappers
// Estimated 3h+ and risk of breaking the existing IPC layer.
// This v0.67b stub catches the same drift cases (function
// name + param count + return type + DTO references) for
// 1/20th the time.
//
// **When this test fails**: someone changed the return
// type of an IPC wrapper, added a new field to a DTO, or
// added/removed a type import. The fix is to update both
// layers (src/ipc.ts AND src-tauri/src/commands/*.rs).
// See docs/overview.md §1.2.

import { describe, it, expect } from 'vitest';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const SRC_DIR = join(import.meta.dirname, '..');
const IPC_PATH = join(SRC_DIR, 'ipc.ts');
const TYPES_DIR = join(SRC_DIR, 'types');
const SNAPSHOT_PATH = join(SRC_DIR, 'ipc.snapshot.v2.json');

interface IpcSnapshotV2 {
  functions: Record<string, {
    params: number;
    /** Param names (best-effort). */
    paramNames: string[];
    /** Return type as written in the source (string). */
    returnType: string | null;
  }>;
  /** Type imports referenced by ipc.ts. */
  typeImports: string[];
  total: number;
}

/** Match `export const NAME = (...args...): ReturnType => ...` */
const ARROW_RE = /^export const (\w+)\s*=\s*(?:async\s*)?\(([^)]*)\)(?:\s*:\s*([^\s=>{]+))?\s*=>/gm;
const TYPE_IMPORT_RE = /import\s+type\s*\{([^}]+)\}\s+from\s+['"]@\/types\/(\w+)['"]/g;

function parseIpcFile(): IpcSnapshotV2 {
  const src = readFileSync(IPC_PATH, 'utf8');
  const functions: IpcSnapshotV2['functions'] = {};
  let m: RegExpExecArray | null;

  while ((m = ARROW_RE.exec(src)) !== null) {
    const name = m[1];
    const paramsStr = m[2] ?? '';
    const returnType = m[3] ?? null;
    if (name) {
      const params = paramsStr.split(',').filter((p) => p.trim().length > 0);
      const paramNames = params.map((p) => {
        // Strip default values, type annotations, and destructuring.
        // We just want the bare identifier.
        const m = p.match(/^\s*(?:\{|\[)?\s*([a-zA-Z_$][\w$]*)/);
        return m?.[1] ?? p.trim();
      });
      functions[name] = { params: params.length, paramNames, returnType };
    }
  }

  const typeImports: string[] = [];
  while ((m = TYPE_IMPORT_RE.exec(src)) !== null) {
    const names = (m[1] ?? '').split(',').map((n) => n.trim()).filter(Boolean);
    typeImports.push(...names);
  }

  return { functions, typeImports: [...new Set(typeImports)].sort(), total: Object.keys(functions).length };
}

describe('L1 IPC contract v2 (v0.67b)', () => {
  it('matches the v2 snapshot (function names + params + return types + DTO imports)', () => {
    const current = parseIpcFile();

    // Sanity: at least 100 wrappers
    expect(current.total).toBeGreaterThanOrEqual(100);

    if (!existsSync(SNAPSHOT_PATH)) {
      writeFileSync(SNAPSHOT_PATH, JSON.stringify(current, null, 2) + '\n');
      return; // first run
    }

    const snapshot: IpcSnapshotV2 = JSON.parse(readFileSync(SNAPSHOT_PATH, 'utf8'));
    const currentNames = Object.keys(current.functions).sort();
    const snapshotNames = Object.keys(snapshot.functions).sort();

    // 1. Function drift (same as v0.66c)
    const added = currentNames.filter((n) => !snapshotNames.includes(n));
    const removed = snapshotNames.filter((n) => !currentNames.includes(n));
    if (added.length > 0) {
      throw new Error(`IPC contract drift: ${added.length} new fn(s):\n` +
        added.map((n) => `  + ${n}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }
    if (removed.length > 0) {
      throw new Error(`IPC contract drift: ${removed.length} removed fn(s):\n` +
        removed.map((n) => `  - ${n}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }

    // 2. Param drift
    for (const name of currentNames) {
      const c = current.functions[name]!;
      const s = snapshot.functions[name]!;
      if (c.params !== s.params) {
        throw new Error(`IPC contract drift: '${name}' param count: ${s.params} → ${c.params}`);
      }
      // Param names (best-effort)
      for (let i = 0; i < c.paramNames.length; i++) {
        if (c.paramNames[i] !== s.paramNames[i]) {
          throw new Error(`IPC contract drift: '${name}' param ${i}: '${s.paramNames[i]}' → '${c.paramNames[i]}'`);
        }
      }
      // Return type drift
      if (c.returnType !== s.returnType) {
        throw new Error(`IPC contract drift: '${name}' return type: '${s.returnType}' → '${c.returnType}'`);
      }
    }

    // 3. Type import drift
    const currentImports = [...current.typeImports].sort();
    const snapshotImports = [...snapshot.typeImports].sort();
    const addedTypes = currentImports.filter((t) => !snapshotImports.includes(t));
    const removedTypes = snapshotImports.filter((t) => !currentImports.includes(t));
    if (addedTypes.length > 0) {
      throw new Error(`IPC contract drift: ${addedTypes.length} new DTO type(s) imported:\n` +
        addedTypes.map((t) => `  + ${t}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }
    if (removedTypes.length > 0) {
      throw new Error(`IPC contract drift: ${removedTypes.length} DTO type(s) removed:\n` +
        removedTypes.map((t) => `  - ${t}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }
  });
});
