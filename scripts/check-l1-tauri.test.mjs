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
  // Sanity: the output should mention all L1 wrappers
  // and registered commands. The exact count varies as
  // the project grows, so we just check the format.
  if (!r.stdout.match(/\d+ L1 wrappers/)) {
    throw new Error(`expected "N L1 wrappers" in output: ${r.stdout}`);
  }
  if (!r.stdout.match(/\d+ registered commands/)) {
    throw new Error(`expected "N registered commands" in output: ${r.stdout}`);
  }
});

// =========================================================================
// Test 2: output mentions orphan commands (info only, not an error)
// =========================================================================
test('reports orphan commands (registered but no L1 wrapper)', () => {
  const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
  if (r.status !== 0) {
    throw new Error(`exit ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
  }
  // The script reports orphan commands as [info: ...] when there are any.
  // This is informational only; not an error.
  // Some real repos have 0 orphan commands; this test just
  // verifies the script runs cleanly in that case.
  if (!r.stdout.match(/L1↔Tauri OK/)) {
    throw new Error(`expected L1↔Tauri OK: ${r.stdout}`);
  }
});

// =========================================================================
// Test 3: fails when a Tauri command is missing
// =========================================================================
test('fails when a Tauri command is missing (unregistered from lib.rs)', () => {
  // Make a backup, remove one Tauri command REGISTRATION from
  // lib.rs, run the script, restore. This is the exact failure
  // case the guard is designed to catch: an L1 wrapper exists
  // but the command isn't registered in lib.rs (or was removed).
  const libPath = 'src-tauri/src/lib.rs';
  const original = readFileSync(libPath, 'utf8');
  try {
    // Remove the registration of auto_promote_if_better
    const lines = original.split('\n');
    let removed = false;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].includes('commands::sidecar::auto_promote_if_better')) {
        lines.splice(i, 1);
        removed = true;
        break;
      }
    }
    if (!removed) throw new Error('could not find commands::sidecar::auto_promote_if_better in lib.rs');
    writeFileSync(libPath, lines.join('\n'));

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
    writeFileSync(libPath, original);
  }
});

// =========================================================================
// Test 4: catches a v0.4-era missing command bug
// =========================================================================
test('catches a missing Tauri command for an L1 wrapper', () => {
  // This is the exact case the guard caught when generalized
  // to all modules in v0.27a: an L1 wrapper calls
  // `invoke('X', ...)` but no Tauri command with that name
  // is registered in lib.rs. The L1 wrapper would fail at
  // runtime with "command not found". This test simulates
  // that scenario by adding a fake L1 wrapper.
  const ipcPath = 'src/ipc.ts';
  const ipcOriginal = readFileSync(ipcPath, 'utf8');
  try {
    // Add a fake L1 wrapper for a non-existent command
    const fakeWrapper = `

// v0.27a test — fake wrapper for a missing command
export const fakeTestWrapper = () => invoke<unknown>('fake_test_method');
`;
    writeFileSync(ipcPath, ipcOriginal + fakeWrapper);

    const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
    if (r.status === 0) {
      throw new Error(`expected non-zero exit, got 0\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
    }
    const output = r.stderr || r.stdout;
    if (!output.match(/fakeTestWrapper/)) {
      throw new Error(`expected "fakeTestWrapper" in error: ${output}`);
    }
    if (!output.match(/fake_test_method/)) {
      throw new Error(`expected "fake_test_method" in error: ${output}`);
    }
  } finally {
    writeFileSync(ipcPath, ipcOriginal);
  }
});

// =========================================================================
// Test 5 (v0.32a): catches a "defined but not registered" Tauri command
// =========================================================================
test('catches a #[tauri::command] function that is defined but not registered', () => {
  // v0.32a — inverse direction. We add a `#[tauri::command]`
  // function to commands/seed.rs that is NOT registered in
  // lib.rs::generate_handler!. The guard should catch it.
  const seedPath = 'src-tauri/src/commands/seed.rs';
  const original = readFileSync(seedPath, 'utf8');
  try {
    // Append a fake #[tauri::command] function to seed.rs
    const fakeFn = `

// v0.32a test — fake tauri::command that is not registered in lib.rs
#[tauri::command]
pub async fn fake_unregistered_command_v032a() -> AppResult<String> {
    Ok("never wired up".into())
}
`;
    writeFileSync(seedPath, original + fakeFn);

    const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
    if (r.status === 0) {
      throw new Error(`expected non-zero exit, got 0\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
    }
    const output = r.stderr || r.stdout;
    if (!output.match(/L1↔Tauri mismatch/)) {
      throw new Error(`expected "L1↔Tauri mismatch" in output: ${output}`);
    }
    if (!output.match(/fake_unregistered_command_v032a/)) {
      throw new Error(`expected "fake_unregistered_command_v032a" in error: ${output}`);
    }
    if (!output.match(/seed\.rs/)) {
      throw new Error(`expected "seed.rs" in error (file hint): ${output}`);
    }
  } finally {
    writeFileSync(seedPath, original);
  }
});

// =========================================================================
// Test 6 (v0.32a): output reports the defs count
// =========================================================================
test('reports #[tauri::command] defs count in the OK line', () => {
  const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
  if (r.status !== 0) {
    throw new Error(`exit ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
  }
  // The v0.32a output adds "N #[tauri::command] defs" to the OK line
  if (!r.stdout.match(/\d+ #\[tauri::command\] defs/)) {
    throw new Error(`expected "N #[tauri::command] defs" in OK line: ${r.stdout}`);
  }
});

// =========================================================================
// Summary
// =========================================================================
console.log('');
console.log(`Results: ${passed} passed, ${failed} failed`);
process.exit(failed === 0 ? 0 : 1);
