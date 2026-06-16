#!/usr/bin/env node
/**
 * DocSync pre-commit check (governance §5).
 * Refuses to commit if code under src/ or src-tauri/ changed without
 * a corresponding update to docs/polyrocket-*.md.
 */
import { execSync } from 'node:child_process';

const staged = execSync('git diff --cached --name-only')
  .toString()
  .trim()
  .split('\n')
  .filter(Boolean);

if (staged.length === 0) {
  console.log('✓ nothing staged, skipping doc-sync check');
  process.exit(0);
}

const codeChanged = staged.some(
  (f) => f.startsWith('src/') || f.startsWith('src-tauri/'),
);

const docChanged = staged.some((f) =>
  /^docs\/polyradar-(blueprint|ui-spec|dev-governance)/.test(f) ||
  /^(polyradar-blueprint|polyradar-ui-spec|polyradar-dev-governance)/.test(f),
);

if (codeChanged && !docChanged) {
  console.error('❌ DocSync violation');
  console.error('   code changed but no polyrocket-*.md doc updated');
  console.error('   see polyradar-dev-governance.md §2 for the sync table');
  process.exit(1);
}

console.log('✓ doc-sync OK');