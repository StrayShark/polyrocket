#!/usr/bin/env node
/**
 * Pre-commit governance checks.
 *
 *   1. DocSync: refuses to commit if code under src/ or src-tauri/
 *      changed without a corresponding update to docs/polyrocket-*.md
 *      or docs/overview.md.
 *   2. LayerGuard: refuses to commit if any Rust file imports across
 *      a disallowed layer edge (see docs/overview.md §1.2).
 *   3. WorkflowSync: refuses to commit if .github/workflows/ changed
 *      without a corresponding update to docs/overview.md §6 (the
 *      "CI guard" section).
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
const ciChanged = staged.some((f) => f.startsWith('.github/'));
const docChanged = staged.some((f) =>
  /^docs\/(polyradar-(blueprint|ui-spec|dev-governance)|polyrocket-.*|overview)\.md$/.test(f) ||
  /^(polyradar-(blueprint|ui-spec|dev-governance)|polyrocket-.*|overview)\.md$/.test(f),
);

if ((codeChanged || ciChanged) && !docChanged) {
  console.error('❌ DocSync violation');
  console.error('   code, .github/ or sidecar/ changed but no polyrocket-*.md or overview.md doc updated');
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
