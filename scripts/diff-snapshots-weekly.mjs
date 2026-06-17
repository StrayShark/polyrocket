#!/usr/bin/env node
/**
 * Weekly snapshot diff (v0.37b).
 *
 * Diffs the current `docs/previews/` against
 * the snapshot from N days ago (default 7).
 * Used to catch slow visual drift: "this
 * page looked the same yesterday, but it
 * looked different a week ago".
 *
 * Usage:
 *   node scripts/diff-snapshots-weekly.mjs
 *   node scripts/diff-snapshots-weekly.mjs --days=14
 *
 * Looks for the snapshot from N days ago in
 * docs/previews/history/YYYY-MM-DD/. If no
 * snapshot exists for that exact date, looks
 * for the closest one within ±2 days. If still
 * nothing, exits 0 with a friendly message.
 *
 * Exits 0 if no diffs, 1 if diffs, 2 on usage
 * error.
 */

import { statSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';

import { diffDirs } from './diff-snapshots.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..');
const previewsDir = resolve(repoRoot, 'docs/previews');
const historyDir = resolve(previewsDir, 'history');

/** Format Date as YYYY-MM-DD (UTC). */
function ymd(d) {
  return d.toISOString().slice(0, 10);
}

/** Find the snapshot dir for "N days ago", with
 *  fallback to ±2 days. Returns the path or null. */
function findSnapshotFor(daysAgo) {
  const today = new Date();
  // Try exact date first
  for (let offset = 0; offset <= 2; offset++) {
    for (const sign of [1, -1]) {
      const d = new Date(today);
      d.setUTCDate(d.getUTCDate() - daysAgo + sign * offset);
      const dir = resolve(historyDir, ymd(d));
      if (existsSync(dir)) return dir;
    }
  }
  return null;
}

function main() {
  let days = 7;
  for (const arg of process.argv.slice(2)) {
    const m = arg.match(/^--days=(\d+)$/);
    if (m) days = parseInt(m[1], 10);
    else {
      console.error(`Unknown arg: ${arg}`);
      console.error('Usage: node scripts/diff-snapshots-weekly.mjs [--days=N]');
      process.exit(2);
    }
  }

  // v0.37b — diff the current previews against
  // the snapshot from N days ago.
  if (!existsSync(previewsDir)) {
    console.error(`Error: ${previewsDir} does not exist`);
    process.exit(2);
  }
  const baselineDir = findSnapshotFor(days);
  if (!baselineDir) {
    console.log(`No snapshot found for ~${days} days ago`);
    console.log(`  expected: ${historyDir}/YYYY-MM-DD`);
    console.log(`  (skipped; run rotate-snapshots.sh to build up history)`);
    process.exit(0);
  }

  console.log(`Weekly diff: ${baselineDir} → ${previewsDir}`);
  const r = diffDirs(baselineDir, previewsDir);

  console.log(`  baseline: ${r.beforeCount} PNGs`);
  console.log(`  current:  ${r.afterCount} PNGs`);
  console.log(`  unchanged: ${r.unchanged.length}`);
  console.log(`  changed:   ${r.changed.length}`);
  console.log(`  added:     ${r.added.length}`);
  console.log(`  removed:   ${r.removed.length}`);

  if (r.changed.length > 0) {
    console.log('');
    console.log('Changed files:');
    for (const f of r.changed) {
      console.log(`  ~ ${f}`);
    }
  }
  if (r.added.length > 0) {
    console.log('');
    console.log('Added files:');
    for (const f of r.added) {
      console.log(`  + ${f}`);
    }
  }
  if (r.removed.length > 0) {
    console.log('');
    console.log('Removed files:');
    for (const f of r.removed) {
      console.log(`  - ${f}`);
    }
  }

  if (r.changed.length === 0 && r.added.length === 0 && r.removed.length === 0) {
    console.log('');
    console.log('✓ No visual drift over the last week');
    process.exit(0);
  } else {
    console.log('');
    console.log('❌ Visual changes since last week');
    process.exit(1);
  }
}

// Export for tests
export { findSnapshotFor, ymd };

// Run if entry point
const isEntry = import.meta.url === `file://${process.argv[1]}`;
if (isEntry) {
  main();
}
