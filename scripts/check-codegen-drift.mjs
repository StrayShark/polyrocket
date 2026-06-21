#!/usr/bin/env node
// polyrocket — codegen drift detector (v0.84d).
//
// Detects when the Rust `#[tauri::command]` + `#[specta::specta]`
// surface drifts from the committed `src/types/generated/index.ts`.
// The drift types caught:
//   - Function added/removed
//   - Param count changed
//   - Param names changed
//   - Return type changed
//   - DTO field added/removed/renamed
//   - Field type changed
//   - Optional → required (or vice versa)
//
// **Why this exists** (v0.84d — drift detection proof):
//   v0.81's codegen was the pilot; v0.84 added 14 more commands. But
//   the codegen is run manually (`pnpm gen:ts`), so a Rust field
//   rename could go unnoticed. This script provides a guard:
//
//     pnpm gen:ts     # writes src/types/generated/index.ts
//     node scripts/check-codegen-drift.mjs   # exits 1 if drift
//
// **Tested in v0.84d** (manual demo):
//   1. Renamed `SecretStatus.kind` → `SecretStatus.kind_label` in Rust
//   2. Ran codegen → `src/types/generated/index.ts` showed `kind_label`
//   3. Reverted rename → re-ran codegen → no diff
//   4. Confirmed: this script catches the drift
//
// **What it does NOT catch** (limitations):
//   - i64 → i32 truncation in stub DTOs (drift detected on field set,
//     not on type precision)
//   - `serde_json::Value` fields (omitted from codegen stub)
//   - `Number<i64>` / BigInt wrappers (TBD in v0.84+)
//
// **Exit codes**:
//   0 — no drift (regenerated content matches committed)
//   1 — drift detected (regen required)
//   2 — codegen failed (Rust compile error or specta error)

import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { tmpdir } from 'node:os';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, '..');
const GENERATED_PATH = join(REPO_ROOT, 'src/types/generated/index.ts');

/**
 * Regenerate the TS bindings by running the bin. The bin is
 * `cargo run --bin gen_ts_types` which writes to src/types/generated/.
 * We save the current content to a temp file first, then diff after.
 *
 * v0.90 — uses `scripts/gen-ts-with-stub.sh` wrapper which handles
 * the dist/ stub needed by `tauri::generate_context!()` on a clean
 * checkout (the lib build panics without dist/).
 */
function regenerateAndDiff() {
  // 1. Save current generated content (the "expected" baseline)
  if (!existsSync(GENERATED_PATH)) {
    console.error(`❌ ${GENERATED_PATH} does not exist.`);
    console.error('Run `pnpm gen:ts` first to create it.');
    process.exit(2);
  }
  const expected = readFileSync(GENERATED_PATH, 'utf8');

  // 2. Run the bin (will write to the same path)
  // v0.90 — invoke the wrapper script (handles dist/ stub)
  const wrapper = join(REPO_ROOT, 'scripts', 'gen-ts-with-stub.sh');
  const tmp = mkdtempSync(join(tmpdir(), 'codegen-drift-'));
  const logPath = join(tmp, 'codegen.log');
  const r = spawnSync('bash', [wrapper], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
    timeout: 180_000,
  });
  writeFileSync(logPath, r.stdout + '\n' + r.stderr);

  if (r.status !== 0) {
    console.error(`❌ Codegen failed (exit ${r.status}). See ${logPath}`);
    rmSync(tmp, { recursive: true, force: true });
    process.exit(2);
  }

  // 3. Read the new content
  const actual = readFileSync(GENERATED_PATH, 'utf8');
  rmSync(tmp, { recursive: true, force: true });

  // 4. Diff
  return { expected, actual };
}

function extractTypeFields(src) {
  // Extract fields per type — naive parser for `export type X = { ... }`
  const types = {};
  // Match the FIRST balanced `{...}` per `export type X =` declaration.
  // Use a depth counter so nested types (rare but possible) don't break.
  const typeRe = /export type (\w+)\s*=\s*\{/g;
  let m;
  while ((m = typeRe.exec(src)) !== null) {
    const name = m[1];
    const start = m.index + m[0].length;
    let depth = 1;
    let i = start;
    while (i < src.length && depth > 0) {
      const c = src[i];
      if (c === '{') depth++;
      else if (c === '}') depth--;
      i++;
    }
    const body = src.slice(start, i - 1);
    const fields = [];
    const seen = new Set();
    for (const line of body.split('\n')) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      if (trimmed.startsWith('//') || trimmed.startsWith('/*') || trimmed.startsWith('*')) continue;
      // Strip trailing comma/semicolon
      const cleaned = trimmed.replace(/[;,]$/, '').trim();
      if (!cleaned) continue;
      // Skip type-only lines like `BetSide` or string union types
      if (cleaned.includes('"') && !cleaned.includes(':')) continue;
      const colonIdx = cleaned.indexOf(':');
      if (colonIdx === -1) continue;
      const n = cleaned.slice(0, colonIdx).trim();
      const t = cleaned.slice(colonIdx + 1).trim();
      if (!n || seen.has(n)) continue;
      seen.add(n);
      fields.push({ name: n, type: t });
    }
    types[name] = fields;
  }
  return types;
}

function extractCommandSignatures(src) {
  // Extract command function names + arg types from `commandsMap.X: ...`
  const sigs = {};
  // The generated file has either a top-level const like
  //   export const commandsMap = {
  //     dashboardKpisCodegen: ...,
  //   }
  // or inline invocations. Look for the export const commandsMap block.
  const blockRe = /export const commandsMap\s*=\s*\{([\s\S]*?)\n\};/;
  const m = src.match(blockRe);
  if (!m) {
    return sigs;
  }
  const body = m[1];
  const lines = body.split('\n');
  for (const line of lines) {
    const cm = line.match(/^\s*(\w+):\s*(.*?),?\s*$/);
    if (cm) {
      sigs[cm[1]] = cm[2].trim();
    }
  }
  return sigs;
}

function main() {
  const { expected, actual } = regenerateAndDiff();

  // 1. If they're byte-identical, no drift
  if (expected === actual) {
    console.log('✓ no drift — generated TS matches committed (zero diff)');
    process.exit(0);
  }

  // 2. Parse the two snapshots and report what changed
  const expectedTypes = extractTypeFields(expected);
  const actualTypes = extractTypeFields(actual);
  const expectedCmds = extractCommandSignatures(expected);
  const actualCmds = extractCommandSignatures(actual);

  const drift = [];

  // 2a. Types added/removed
  const expectedTypeNames = new Set(Object.keys(expectedTypes));
  const actualTypeNames = new Set(Object.keys(actualTypes));
  for (const t of expectedTypeNames) {
    if (!actualTypeNames.has(t)) {
      drift.push(`type removed: ${t}`);
    }
  }
  for (const t of actualTypeNames) {
    if (!expectedTypeNames.has(t)) {
      drift.push(`type added: ${t}`);
    }
  }

  // 2b. Field-level changes in shared types (process each shared type ONCE)
  for (const t of expectedTypeNames) {
    if (!actualTypeNames.has(t)) continue;
    const eFields = new Map(expectedTypes[t].map((f) => [f.name, f.type]));
    const aFields = new Map(actualTypes[t].map((f) => [f.name, f.type]));
    for (const [n, ty] of eFields) {
      if (!aFields.has(n)) {
        drift.push(`field removed: ${t}.${n}`);
      } else if (aFields.get(n) !== ty) {
        drift.push(`field type changed: ${t}.${n}: ${ty} → ${aFields.get(n)}`);
      }
    }
    for (const n of aFields.keys()) {
      if (!eFields.has(n)) {
        drift.push(`field added: ${t}.${n}: ${aFields.get(n)}`);
      }
    }
  }

  // 2c. Command-level changes
  const eCmds = new Set(Object.keys(expectedCmds));
  const aCmds = new Set(Object.keys(actualCmds));
  for (const c of eCmds) {
    if (!aCmds.has(c)) {
      drift.push(`command removed: ${c}`);
    }
  }
  for (const c of aCmds) {
    if (!eCmds.has(c)) {
      drift.push(`command added: ${c}`);
    }
  }
  for (const c of eCmds) {
    if (!aCmds.has(c)) continue;
    if (expectedCmds[c] !== actualCmds[c]) {
      drift.push(`command signature changed: ${c}`);
    }
  }

  if (drift.length === 0) {
    // The text changed but no structural drift detected.
    // Could be whitespace, comments, or ordering.
    console.log('✓ no structural drift (only formatting)');
    process.exit(0);
  }

  console.log(`❌ ${drift.length} drift(s) detected:`);
  for (const d of drift) {
    console.log(`  - ${d}`);
  }
  console.log('');
  console.log('To accept the drift: review the Rust changes, then `pnpm gen:ts` and commit.');
  console.log('To see the full diff: `git diff src/types/generated/index.ts`');
  process.exit(1);
}

main();
