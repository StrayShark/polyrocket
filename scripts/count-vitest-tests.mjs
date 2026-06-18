#!/usr/bin/env node
// polyrocket — count vitest tests (v0.66f helper).
//
// Runs `pnpm vitest run --reporter=json` and prints
// the test-file count + total test count as JSON.
//
// **Why**: the README test-totals line is a manual
// number that drifts after every ratchet. We automate
// the vitest count by re-running vitest in JSON mode
// and parsing the result. The run takes ~3s in CI
// (vitest's full pass).
//
// **Why a separate script**: update-readme-coverage.mjs
// already shells out for the density check; we want the
// vitest count to be a separate, cacheable step so the
// coverage + density + count updates can be skipped
// independently if one of them fails.
//
// **Output**: one line of JSON on stdout:
//   { "testFiles": 64, "tests": 569 }
// Exit 0 on success, 1 on failure.

import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, '..');

const result = spawnSync('pnpm', ['vitest', 'run', '--reporter=json'], {
  cwd: REPO_ROOT,
  encoding: 'utf8',
  timeout: 120_000,
});

if (result.status !== 0) {
  process.exit(1);
}

// The JSON output: vitest prints a single JSON object
// at the START of stdout. The shape is
// { numTotalTestSuites, numTotalTests, ... }.
// We just JSON.parse the first line of stdout (which
// is the JSON). pnpm adds progress prefixes later
// (e.g. " PASS  src/...") which we ignore.

const out = result.stdout;
console.error('=== DEBUG: first 200 chars of stdout ===');
console.error(out.slice(0, 200));
console.error('=== END DEBUG ===');
const firstLine = out.split('\n')[0] ?? '';
let parsed = null;
try {
  parsed = JSON.parse(firstLine);
} catch (e) {
  console.error('Parse error:', e.message);
}

if (!parsed || typeof parsed.numTotalTests !== 'number') {
  console.error('Failed to find numTotalTests in vitest JSON output');
  process.exit(1);
}

const numTotalTests = parsed.numTotalTests ?? 0;
const numTotalTestFiles = parsed.numTotalTestFiles ?? 0;

if (numTotalTests === 0) {
  process.exit(1);
}

console.log(JSON.stringify({ testFiles: numTotalTestFiles, tests: numTotalTests }));
