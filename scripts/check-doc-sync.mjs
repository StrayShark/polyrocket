#!/usr/bin/env node
/**
 * Pre-commit governance checks.
 *
 *   1. DocSync: refuses to commit if code under src/, src-tauri/
 *      or sidecar/ changed without a corresponding update to
 *      docs/polyrocket-*.md or docs/overview.md.
 *   2. LayerGuard: refuses to commit if any Rust file imports across
 *      a disallowed layer edge (see docs/overview.md §1.2).
 *
 * Both checks are run from this entry point so a single pre-commit
 * hook invocation catches both classes of violation.
 */
import { execSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const staged = execSync('git diff --cached --name-only')
  .toString()
  .trim()
  .split('\n')
  .filter(Boolean);

if (staged.length === 0) {
  console.log('✓ nothing staged, skipping governance checks');
  process.exit(0);
}

const codeChanged = staged.some(
  (f) => f.startsWith('src/') || f.startsWith('src-tauri/') || f.startsWith('sidecar/'),
);
const docChanged = staged.some((f) =>
  /^docs\/(polyradar-(blueprint|ui-spec|dev-governance)|polyrocket-.*|overview)\.md$/.test(f) ||
  /^(polyradar-(blueprint|ui-spec|dev-governance)|polyrocket-.*|overview)\.md$/.test(f),
);

if (codeChanged && !docChanged) {
  console.error('❌ DocSync violation');
  console.error('   code or sidecar/ changed but no polyrocket-*.md or overview.md doc updated');
  console.error('   see docs/overview.md §5 for the doc-sync table');
  process.exit(1);
}

console.log('✓ doc-sync OK');

// 2. LayerGuard — only run if Rust files are staged
const hasRs = staged.some((f) => f.endsWith('.rs') && f.startsWith('src-tauri/'));
if (hasRs) {
  const here = dirname(fileURLToPath(import.meta.url));
  const guard = resolve(here, 'check-layers.mjs');
  const r = spawnSync('node', [guard], { stdio: 'inherit' });
  if (r.status !== 0) {
    console.error('❌ LayerGuard failed (see above)');
    process.exit(r.status ?? 1);
  }
}

// 3. L1↔TauriGuard (v0.26a, generalized in v0.27a) —
//    only run if either the L1 wrapper file or the
//    Tauri lib.rs (which registers commands) is
//    staged. Catches "wire format but no Tauri command"
//    issues at commit time. v0.27a generalized the
//    guard from sidecar-only to all modules.
const hasL1 = staged.some((f) => f === 'src/ipc.ts');
const hasLibRs = staged.some((f) => f === 'src-tauri/src/lib.rs');
if (hasL1 || hasLibRs) {
  const here = dirname(fileURLToPath(import.meta.url));
  const guard = resolve(here, 'check-l1-tauri.mjs');
  const r = spawnSync('node', [guard], { stdio: 'inherit' });
  if (r.status !== 0) {
    console.error('❌ L1↔TauriGuard failed (see above)');
    process.exit(r.status ?? 1);
  }
}

// 4. SnapshotDiff lint (v0.35b) — informational only.
//    If any `docs/previews/*.png` is staged, compare it
//    against the version in HEAD using git. Reports the
//    list of changed PNGs. This is NOT a blocker (the
//    user may have intentionally regenerated snapshots),
//    but it makes the change visible in the commit
//    output so the reviewer knows what to expect.
//
//    To run a real "before/after" diff with byte
//    comparison, the user can call
//    `node scripts/diff-snapshots.mjs <before> <after>`
//    manually after `snapshot_pages.py` regenerates.
const hasSnapshotChanges = staged.some(
  (f) => f.startsWith('docs/previews/') && f.endsWith('.png'),
);
if (hasSnapshotChanges) {
  // Get the list of changed PNGs via git
  let diffOutput = '';
  try {
    diffOutput = execSync(
      'git diff --cached --name-only --diff-filter=AM -- docs/previews/',
      { encoding: 'utf8' },
    ).trim();
  } catch {
    // git not available or not in a repo; skip
  }
  if (diffOutput) {
    const changedPngs = diffOutput.split('\n').filter(Boolean);
    console.log('');
    console.log(`ℹ SnapshotDiff (v0.35b): ${changedPngs.length} PNG(s) changed`);
    for (const p of changedPngs.slice(0, 10)) {
      console.log(`   ~ ${p}`);
    }
    if (changedPngs.length > 10) {
      console.log(`   ... and ${changedPngs.length - 10} more`);
    }
    console.log('   (informational only; review the visual changes)');
  }
}
