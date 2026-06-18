#!/usr/bin/env node
// polyrocket — README.md badge auto-updater (v0.64a).
//
// Reads `coverage/coverage-summary.json` (output of
// `pnpm test:coverage`) and rewrites the badge URLs
// + test-totals line in `README.md` so the badges
// always reflect the current actuals.
//
// **Why this script exists**: v0.63c added 5
// shields.io badges to the README with hard-coded
// percentages. After every coverage ratchet we
// needed to manually edit 3 lines. This script
// automates that.
//
// **Why a script and not a workflow**: the project
// uses local-only commits (see user.md "Git push
// convention"). A workflow that auto-commits to
// main would conflict with that. This script is
// intended to be run by the maintainer locally
// right after `pnpm test:coverage`, then committed
// as part of the ratchet. 1 command, no GHA push
// concerns.
//
// **Usage**:
//   pnpm test:coverage
//   node scripts/update-readme-coverage.mjs
//   git add README.md
//   git commit -m "v0.64+: ratchet X% → Y% (auto badge update)"
//
// **Side effects**:
//   - In-place edit of `README.md` (idempotent —
//     running twice with the same coverage is a no-op).
//   - Exits 1 if coverage-summary.json is missing
//     or README.md doesn't contain the expected
//     badge anchors. Never touches git.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, '..');
const SUMMARY_PATH = join(REPO_ROOT, 'coverage', 'coverage-summary.json');
const README_PATH = join(REPO_ROOT, 'README.md');

// ----- Read coverage -----------------------------------------------------
if (!existsSync(SUMMARY_PATH)) {
  console.error(
    `coverage-summary.json not found at ${SUMMARY_PATH}\n` +
    `Run \`pnpm test:coverage\` first, then re-run this script.`,
  );
  process.exit(1);
}

let summary;
try {
  summary = JSON.parse(readFileSync(SUMMARY_PATH, 'utf8'));
} catch (e) {
  console.error(`Failed to parse coverage-summary.json: ${e.message}`);
  process.exit(1);
}

const total = summary.total;
if (!total) {
  console.error('coverage-summary.json has no `total` key — bad vitest output?');
  process.exit(1);
}

const sPct = total.statements?.pct ?? 0;
const bPct = total.branches?.pct ?? 0;
const fPct = total.functions?.pct ?? 0;
const lPct = total.lines?.pct ?? 0;

// Format for badge: integer % or 1 decimal.
// shields.io truncates 67.4 → 67, but "67.4%25" works
// because we percent-encode the % and the badge shows
// the label as-is. 1 decimal place feels more honest.
const fmt = (n) => (Math.round(n * 10) / 10).toFixed(1);

// ----- Color (red/yellow/green bucket) -----------------------------------
// shields.io `?color=` accepts any hex. We use a small
// bucketing so the badge reacts to regressions:
//   >= 80%  → brightgreen
//   >= 60%  → green
//   >= 40%  → yellow
//   <  40%  → red
function colorFor(pct) {
  if (pct >= 80) return 'brightgreen';
  if (pct >= 60) return 'green';
  if (pct >= 40) return 'yellow';
  return 'red';
}

const COVERAGE_COLOR = colorFor(sPct);

// ----- Update README ------------------------------------------------------
if (!existsSync(README_PATH)) {
  console.error(`README.md not found at ${README_PATH}`);
  process.exit(1);
}

let readme = readFileSync(README_PATH, 'utf8');
const before = readme;

// 1. Coverage badge — replaces the existing badge URL with
//    the same anchor (./vitest.config.ts) but new label.
const COV_BADGE_RE = /\[!\[coverage\]\(https:\/\/img\.shields\.io\/badge\/vitest%20cov-[^)]+\)\]\(\.\/vitest\.config\.ts\)/;
const newCovBadge = `[![coverage](https://img.shields.io/badge/vitest%20cov-${fmt(sPct)}%25%20stmts-${COVERAGE_COLOR})](./vitest.config.ts)`;
if (COV_BADGE_RE.test(readme)) {
  readme = readme.replace(COV_BADGE_RE, newCovBadge);
} else {
  console.error('README.md does not contain the expected coverage badge anchor. Run v0.63c first.');
  process.exit(1);
}

// 2. Coverage gate table line — "vitest 67.4% stmts / ..."
const COVERAGE_LINE_RE = /\| Coverage gate \| vitest [0-9.]+% stmts \/ [0-9.]+% branches \/ [0-9.]+% funcs \/ [0-9.]+% lines .*?\|/;
const newCoverageLine = '| Coverage gate | vitest ' + fmt(sPct) + '% stmts / ' + fmt(bPct) + '% branches / ' + fmt(fPct) + '% funcs / ' + fmt(lPct) + '% lines (`vitest.config.ts`) |';
if (COVERAGE_LINE_RE.test(readme)) {
  readme = readme.replace(COVERAGE_LINE_RE, newCoverageLine);
} else {
  console.error('README.md does not contain the expected coverage gate line. Run v0.63c first.');
  process.exit(1);
}

// 3. Density badge — run scripts/check-comment-density.mjs and
//    parse the output. Updates the `[![density](...)]` badge
//    URL with the current "N/5 PASS" count and a color
//    bucket (red if 0/5, yellow if 1-2/5, yellowgreen if
//    3/5, green if 4/5, brightgreen if 5/5).
//
//    v0.65c — was hand-maintained "5/5 PASS" in v0.63c.
//    Now we re-run the check and update the badge from
//    real output.
import { spawnSync } from 'node:child_process';

const DENSITY_RESULT = spawnSync('node', ['scripts/check-comment-density.mjs'], {
  cwd: REPO_ROOT,
  encoding: 'utf8',
});

if (DENSITY_RESULT.status !== 0) {
  // density check failed (e.g. some category dropped below 50% passing).
  // We still want to fail loudly so the maintainer notices, BUT
  // the script shouldn't lose the previous badge value silently.
  console.error('Density check FAILED — not updating README density badge.');
  console.error(DENSITY_RESULT.stdout);
  console.error(DENSITY_RESULT.stderr);
  process.exit(1);
}

// Parse: "[PASS] rust-commands-domain-infra (target 15%+, rust)"
//        "        85/85 files passing (100.0%); avg ratio 22.4%"
// 5 lines of [PASS] / [FAIL] for the 5 categories.
const densityLines = DENSITY_RESULT.stdout.split('\n');
const passingCount = densityLines.filter((l) => /^\[PASS\]/.test(l)).length;
const totalCount = densityLines.filter((l) => /^\[(PASS|FAIL)\]/.test(l)).length;

const DENSITY_BADGE_RE = /\[!\[density\]\(https:\/\/img\.shields\.io\/badge\/comment%20density-[^)]+\)\]\(\.\/scripts\/check-comment-density\.mjs\)/;
const densityColor =
  passingCount === totalCount ? 'brightgreen' :
  passingCount >= 4 ? 'green' :
  passingCount >= 3 ? 'yellowgreen' :
  passingCount >= 2 ? 'yellow' :
  passingCount >= 1 ? 'orange' : 'red';
const newDensityBadge = `[![density](https://img.shields.io/badge/comment%20density-${passingCount}%2F${totalCount}%20PASS-${densityColor})](./scripts/check-comment-density.mjs)`;

if (DENSITY_BADGE_RE.test(readme)) {
  readme = readme.replace(DENSITY_BADGE_RE, newDensityBadge);
} else {
  console.error('README.md does not contain the expected density badge anchor. Run v0.63c first.');
  process.exit(1);
}

if (readme === before) {
  console.log('README.md is already up to date (no changes needed).');
  process.exit(0);
}

writeFileSync(README_PATH, readme);
console.log(`README.md updated:`);
console.log(`  coverage badge → vitest ${fmt(sPct)}% stmts (color: ${COVERAGE_COLOR})`);
console.log(`  coverage gate line → vitest ${fmt(sPct)}% stmts / ${fmt(bPct)}% branches / ${fmt(fPct)}% funcs / ${fmt(lPct)}% lines`);
console.log(`  density badge → ${passingCount}/${totalCount} PASS (color: ${densityColor})`);
console.log('');
console.log('Next: git add README.md && git commit -m "..."');
