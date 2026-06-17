#!/usr/bin/env node
/**
 * Snapshot diffing (v0.35a).
 *
 * Compares two directories of PNGs and reports
 * any visual differences. Used to catch
 * unintended visual regressions at commit time.
 *
 * Usage:
 *   node scripts/diff-snapshots.mjs <before-dir> <after-dir>
 *
 * Exit codes:
 *   0 — no differences (all PNGs match byte-for-byte)
 *   1 — at least one difference
 *   2 — usage error (missing args, dirs don't exist, etc.)
 *
 * Why byte comparison (not pixel diff)?
 *   - Fast: O(N) in total file size, no decoding
 *   - Deterministic: no tolerance to tune
 *   - Catches ALL visual changes (any pixel
 *     change → bytes differ)
 *
 * The "before" directory is typically the
 * current `docs/previews/` snapshot. The user
 * regenerates the snapshot with `snapshot_pages.py`,
 * then runs this script to see what changed.
 *
 * If the user wants a "historical baseline" (e.g.
 * "compare to v0.27 baseline"), they can copy
 * the baseline into a separate dir and use
 * that as the before.
 *
 * v0.35b — integration: the check-doc-sync.mjs
 * script can call this on `src/**` or
 * `src/styles/**` changes, to alert on
 * visual regressions.
 */

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, resolve, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..');

/** Recursively walk a directory and return all PNG files. */
function walkPngs(dir) {
  const out = [];
  function recurse(d) {
    let entries;
    try {
      entries = readdirSync(d);
    } catch (e) {
      // Not a directory or not readable
      return;
    }
    for (const e of entries) {
      const full = join(d, e);
      let st;
      try {
        st = statSync(full);
      } catch {
        continue;
      }
      if (st.isDirectory()) {
        recurse(full);
      } else if (st.isFile() && e.toLowerCase().endsWith('.png')) {
        out.push(full);
      }
    }
  }
  recurse(dir);
  return out;
}

/** SHA-256 hash of a file's contents. */
function hashFile(path) {
  const buf = readFileSync(path);
  return createHash('sha256').update(buf).digest('hex');
}

/**
 * Compare two directories of PNGs. Returns
 * an object with the diff result.
 */
function diffDirs(beforeDir, afterDir) {
  const beforeFiles = walkPngs(beforeDir);
  const afterFiles = walkPngs(afterDir);

  // Build a map of relative path → hash for each side.
  // Relative path is relative to the dir, so we can
  // match files by name (e.g. "dark/dashboard.png").
  const beforeMap = new Map();
  for (const f of beforeFiles) {
    const rel = f.slice(beforeDir.length + 1); // strip prefix + /
    beforeMap.set(rel, hashFile(f));
  }
  const afterMap = new Map();
  for (const f of afterFiles) {
    const rel = f.slice(afterDir.length + 1);
    afterMap.set(rel, hashFile(f));
  }

  const added = []; // in after, not in before
  const removed = []; // in before, not in after
  const changed = []; // in both, but hash differs
  const unchanged = []; // in both, hash matches

  for (const [rel, hash] of afterMap) {
    if (!beforeMap.has(rel)) {
      added.push(rel);
    } else if (beforeMap.get(rel) !== hash) {
      changed.push(rel);
    } else {
      unchanged.push(rel);
    }
  }
  for (const [rel] of beforeMap) {
    if (!afterMap.has(rel)) {
      removed.push(rel);
    }
  }

  return {
    beforeDir,
    afterDir,
    beforeCount: beforeMap.size,
    afterCount: afterMap.size,
    unchanged,
    added,
    removed,
    changed,
  };
}

function main() {
  const args = process.argv.slice(2);
  if (args.length !== 2) {
    console.error('Usage: node scripts/diff-snapshots.mjs <before-dir> <after-dir>');
    process.exit(2);
  }
  const [beforeDir, afterDir] = args.map((a) => resolve(a));
  try {
    statSync(beforeDir);
  } catch {
    console.error(`Error: before-dir does not exist: ${beforeDir}`);
    process.exit(2);
  }
  try {
    statSync(afterDir);
  } catch {
    console.error(`Error: after-dir does not exist: ${afterDir}`);
    process.exit(2);
  }

  const result = diffDirs(beforeDir, afterDir);

  console.log(`Snapshot diff: ${result.beforeDir} → ${result.afterDir}`);
  console.log(`  before: ${result.beforeCount} PNGs`);
  console.log(`  after:  ${result.afterCount} PNGs`);
  console.log(`  unchanged: ${result.unchanged.length}`);
  console.log(`  changed:   ${result.changed.length}`);
  console.log(`  added:     ${result.added.length}`);
  console.log(`  removed:   ${result.removed.length}`);

  if (result.changed.length > 0) {
    console.log('');
    console.log('Changed files:');
    for (const f of result.changed) {
      console.log(`  ~ ${f}`);
    }
  }
  if (result.added.length > 0) {
    console.log('');
    console.log('Added files:');
    for (const f of result.added) {
      console.log(`  + ${f}`);
    }
  }
  if (result.removed.length > 0) {
    console.log('');
    console.log('Removed files:');
    for (const f of result.removed) {
      console.log(`  - ${f}`);
    }
  }

  // v0.35a — "no diff" is success; any diff is a
  // failure. The caller (CI / check-doc-sync) can
  // act on the exit code.
  if (result.changed.length === 0 && result.added.length === 0 && result.removed.length === 0) {
    console.log('');
    console.log('✓ No visual regressions');
    process.exit(0);
  } else {
    console.log('');
    console.log('❌ Visual changes detected');
    process.exit(1);
  }
}

// Export for tests
export { walkPngs, hashFile, diffDirs };

// Run main if this is the entry point
const isEntry = import.meta.url === `file://${process.argv[1]}`;
if (isEntry) {
  main();
}
