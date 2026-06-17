#!/usr/bin/env node
/**
 * Standalone test for scripts/check-l1-tauri.mjs.
 *
 * Run with: `node scripts/check-l1-tauri.test.mjs`
 *
 * Exits 0 on success, 1 on failure.
 *
 * Tests:
 *   1. The script passes on the real repo (integration test)
 *   2. The script's "X" (failure) code path is exercised by
 *      manually corrupting the source files, running the
 *      script, and restoring (skipped here; covered by
 *      manual CI runs)
 */

import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const REPO = process.cwd();
const SCRIPT = join(REPO, 'scripts/check-l1-tauri.mjs');

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`  ✓ ${name}`);
    passed++;
  } catch (e) {
    console.error(`  ✗ ${name}`);
    console.error(`    ${e.message}`);
    failed++;
  }
}

// =========================================================================
// Test 1: passes on the real repo (integration test)
// =========================================================================
test('passes on the real repo', () => {
  const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
  if (r.status !== 0) {
    throw new Error(`exit ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
  }
  if (!r.stdout.match(/✓ L1↔Tauri OK/)) {
    throw new Error(`unexpected output: ${r.stdout}`);
  }
  // Sanity: the output should mention all 6 sidecar L1 wrappers
  if (!r.stdout.match(/6 sidecar L1 wrappers/)) {
    throw new Error(`expected 6 sidecar L1 wrappers in output: ${r.stdout}`);
  }
});

// =========================================================================
// Test 2: output mentions skipped non-sidecar wrappers
// =========================================================================
test('reports non-sidecar wrappers are skipped', () => {
  const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
  if (r.status !== 0) {
    throw new Error(`exit ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
  }
  if (!r.stdout.match(/non-sidecar skipped/)) {
    throw new Error(`expected "non-sidecar skipped" in output: ${r.stdout}`);
  }
});

// =========================================================================
// Test 3: fails when a Tauri command is missing
// =========================================================================
test('fails when a Tauri command is missing', () => {
  // Make a backup, remove one Tauri command, run the script,
  // restore. This is the exact failure case the guard is
  // designed to catch.
  const sidecarPath = 'src-tauri/src/commands/sidecar.rs';
  const original = readFileSync(sidecarPath, 'utf8');
  try {
    // Remove the `#[tauri::command]` attribute for
    // auto_promote_if_better. We delete the attribute
    // line; the function still exists, but it's no
    // longer a Tauri command.
    const lines = original.split('\n');
    let removed = false;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i] === '#[tauri::command]' && lines[i + 1] && lines[i + 1].includes('auto_promote_if_better')) {
        lines.splice(i, 1);
        removed = true;
        break;
      }
    }
    if (!removed) throw new Error('could not find #[tauri::command] before auto_promote_if_better');
    writeFileSync(sidecarPath, lines.join('\n'));

    const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
    if (r.status === 0) {
      throw new Error(`expected non-zero exit, got 0\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
    }
    // The script writes to stderr (via console.error)
    const output = r.stderr || r.stdout;
    if (!output.match(/L1↔Tauri mismatch/)) {
      throw new Error(`expected "L1↔Tauri mismatch" in output: ${output}\nfull: ${JSON.stringify(r)}`);
    }
    if (!output.match(/autoPromoteIfBetter/)) {
      throw new Error(`expected "autoPromoteIfBetter" in error message: ${output}`);
    }
  } finally {
    // Always restore, even on test failure
    writeFileSync(sidecarPath, original);
  }
});

// =========================================================================
// Summary
// =========================================================================
console.log('');
console.log(`Results: ${passed} passed, ${failed} failed`);
process.exit(failed === 0 ? 0 : 1);
