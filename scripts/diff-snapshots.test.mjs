#!/usr/bin/env node
/**
 * Standalone test for scripts/diff-snapshots.mjs.
 *
 * Run with: `node scripts/diff-snapshots.test.mjs`
 *
 * Exits 0 on success, 1 on failure.
 *
 * Tests:
 *  1. walkPngs finds PNGs recursively
 *  2. walkPngs ignores non-PNG files
 *  3. hashFile is deterministic
 *  4. diffDirs returns no diffs for identical dirs
 *  5. diffDirs detects changed files
 *  6. diffDirs detects added files
 *  7. diffDirs detects removed files
 *  8. CLI exits 0 for no diffs
 *  9. CLI exits 1 for diffs
 * 10. CLI exits 2 for missing args
 */

import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

import { walkPngs, hashFile, diffDirs } from './diff-snapshots.mjs';

const REPO = process.cwd();
const SCRIPT = join(REPO, 'scripts/diff-snapshots.mjs');

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

// Minimal 1x1 transparent PNG (smallest valid PNG)
const MINIMAL_PNG = Buffer.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
  0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk header
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
  0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, // bit depth, color type, etc.
  0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, // IDAT chunk header
  0x54, 0x78, 0x9c, 0x62, 0x00, 0x01, 0x00, 0x00, // IDAT data
  0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, // CRC
  0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, // IEND chunk
  0x42, 0x60, 0x82,
]);

function makeTempDir() {
  return mkdtempSync(join(tmpdir(), 'diff-snap-test-'));
}

function makeTempPngs(dir, files) {
  // files is an array of { path, bytes (override or use MINIMAL_PNG) }
  for (const f of files) {
    const full = join(dir, f.path);
    // Create parent dirs (mkdirSync with recursive: true)
    mkdirSync(join(full, '..'), { recursive: true });
    writeFileSync(full, f.bytes || MINIMAL_PNG);
  }
}

// =========================================================================
// Test 1: walkPngs finds PNGs recursively
// =========================================================================
test('walkPngs finds PNGs recursively', () => {
  const dir = makeTempDir();
  try {
    makeTempPngs(dir, [
      { path: 'a.png' },
      { path: 'sub/b.png' },
      { path: 'sub/deep/c.png' },
    ]);
    const found = walkPngs(dir);
    if (found.length !== 3) {
      throw new Error(`expected 3 PNGs, got ${found.length}`);
    }
  } finally {
    rmSync(dir, { recursive: true });
  }
});

// =========================================================================
// Test 2: walkPngs ignores non-PNG files
// =========================================================================
test('walkPngs ignores non-PNG files', () => {
  const dir = makeTempDir();
  try {
    makeTempPngs(dir, [
      { path: 'a.png' },
      { path: 'b.txt', bytes: Buffer.from('not a png') },
      { path: 'c.jpg', bytes: Buffer.from('not a png either') },
    ]);
    const found = walkPngs(dir);
    if (found.length !== 1) {
      throw new Error(`expected 1 PNG, got ${found.length}`);
    }
    if (!found[0].endsWith('a.png')) {
      throw new Error(`expected a.png, got ${found[0]}`);
    }
  } finally {
    rmSync(dir, { recursive: true });
  }
});

// =========================================================================
// Test 3: hashFile is deterministic
// =========================================================================
test('hashFile is deterministic', () => {
  const dir = makeTempDir();
  try {
    const p = join(dir, 'a.png');
    writeFileSync(p, MINIMAL_PNG);
    const h1 = hashFile(p);
    const h2 = hashFile(p);
    if (h1 !== h2) {
      throw new Error(`hashFile non-deterministic: ${h1} vs ${h2}`);
    }
    if (h1.length !== 64) {
      throw new Error(`expected 64-char SHA-256, got ${h1.length}`);
    }
  } finally {
    rmSync(dir, { recursive: true });
  }
});

// =========================================================================
// Test 4: diffDirs returns no diffs for identical dirs
// =========================================================================
test('diffDirs returns no diffs for identical dirs', () => {
  const before = makeTempDir();
  const after = makeTempDir();
  try {
    makeTempPngs(before, [{ path: 'a.png' }, { path: 'b.png' }]);
    makeTempPngs(after, [{ path: 'a.png' }, { path: 'b.png' }]);
    const r = diffDirs(before, after);
    if (r.changed.length !== 0 || r.added.length !== 0 || r.removed.length !== 0) {
      throw new Error(`expected no diffs, got changed=${r.changed.length} added=${r.added.length} removed=${r.removed.length}`);
    }
    if (r.unchanged.length !== 2) {
      throw new Error(`expected 2 unchanged, got ${r.unchanged.length}`);
    }
  } finally {
    rmSync(before, { recursive: true });
    rmSync(after, { recursive: true });
  }
});

// =========================================================================
// Test 5: diffDirs detects changed files
// =========================================================================
test('diffDirs detects changed files', () => {
  const before = makeTempDir();
  const after = makeTempDir();
  try {
    makeTempPngs(before, [{ path: 'a.png' }]);
    // Different content
    makeTempPngs(after, [{ path: 'a.png', bytes: Buffer.from('different') }]);
    const r = diffDirs(before, after);
    if (r.changed.length !== 1 || r.changed[0] !== 'a.png') {
      throw new Error(`expected 1 changed (a.png), got ${JSON.stringify(r.changed)}`);
    }
  } finally {
    rmSync(before, { recursive: true });
    rmSync(after, { recursive: true });
  }
});

// =========================================================================
// Test 6: diffDirs detects added files
// =========================================================================
test('diffDirs detects added files', () => {
  const before = makeTempDir();
  const after = makeTempDir();
  try {
    makeTempPngs(before, [{ path: 'a.png' }]);
    makeTempPngs(after, [{ path: 'a.png' }, { path: 'b.png' }]);
    const r = diffDirs(before, after);
    if (r.added.length !== 1 || r.added[0] !== 'b.png') {
      throw new Error(`expected 1 added (b.png), got ${JSON.stringify(r.added)}`);
    }
  } finally {
    rmSync(before, { recursive: true });
    rmSync(after, { recursive: true });
  }
});

// =========================================================================
// Test 7: diffDirs detects removed files
// =========================================================================
test('diffDirs detects removed files', () => {
  const before = makeTempDir();
  const after = makeTempDir();
  try {
    makeTempPngs(before, [{ path: 'a.png' }, { path: 'b.png' }]);
    makeTempPngs(after, [{ path: 'a.png' }]);
    const r = diffDirs(before, after);
    if (r.removed.length !== 1 || r.removed[0] !== 'b.png') {
      throw new Error(`expected 1 removed (b.png), got ${JSON.stringify(r.removed)}`);
    }
  } finally {
    rmSync(before, { recursive: true });
    rmSync(after, { recursive: true });
  }
});

// =========================================================================
// Test 8: CLI exits 0 for no diffs
// =========================================================================
test('CLI exits 0 for no diffs', () => {
  const before = makeTempDir();
  const after = makeTempDir();
  try {
    makeTempPngs(before, [{ path: 'a.png' }]);
    makeTempPngs(after, [{ path: 'a.png' }]);
    const r = spawnSync('node', [SCRIPT, before, after], { encoding: 'utf8' });
    if (r.status !== 0) {
      throw new Error(`expected exit 0, got ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
    }
    if (!r.stdout.match(/No visual regressions/)) {
      throw new Error(`expected "No visual regressions" in output: ${r.stdout}`);
    }
  } finally {
    rmSync(before, { recursive: true });
    rmSync(after, { recursive: true });
  }
});

// =========================================================================
// Test 9: CLI exits 1 for diffs
// =========================================================================
test('CLI exits 1 for diffs', () => {
  const before = makeTempDir();
  const after = makeTempDir();
  try {
    makeTempPngs(before, [{ path: 'a.png' }]);
    makeTempPngs(after, [{ path: 'a.png', bytes: Buffer.from('changed') }]);
    const r = spawnSync('node', [SCRIPT, before, after], { encoding: 'utf8' });
    if (r.status !== 1) {
      throw new Error(`expected exit 1, got ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
    }
    if (!r.stdout.match(/Visual changes detected/)) {
      throw new Error(`expected "Visual changes detected" in output: ${r.stdout}`);
    }
    if (!r.stdout.match(/~ a\.png/)) {
      throw new Error(`expected "~ a.png" in output: ${r.stdout}`);
    }
  } finally {
    rmSync(before, { recursive: true });
    rmSync(after, { recursive: true });
  }
});

// =========================================================================
// Test 10: CLI exits 2 for missing args
// =========================================================================
test('CLI exits 2 for missing args', () => {
  const r = spawnSync('node', [SCRIPT], { encoding: 'utf8' });
  if (r.status !== 2) {
    throw new Error(`expected exit 2, got ${r.status}\nstdout: ${r.stdout}\nstderr: ${r.stderr}`);
  }
  if (!r.stderr.match(/Usage:/)) {
    throw new Error(`expected "Usage:" in stderr: ${r.stderr}`);
  }
});

// =========================================================================
// Summary
// =========================================================================
console.log('');
console.log(`Results: ${passed} passed, ${failed} failed`);
process.exit(failed === 0 ? 0 : 1);
