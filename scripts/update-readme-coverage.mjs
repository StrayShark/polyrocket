#!/usr/bin/env node
// polyrocket — README.md badge auto-updater (v0.64a → v0.66f).
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
//   node scripts/update-readme-coverage.mjs --version v0.66
//   git add README.md docs/overview.md
//   git commit -m "v0.66: ratchet X% → Y% (auto update)"
//
// **Side effects**:
//   - In-place edit of `README.md` (idempotent —
//     running twice with the same coverage is a no-op).
//   - In-place edit of `docs/overview.md` if
//     `--version` is given (updates the version
//     header).
//   - Re-runs `scripts/check-comment-density.mjs`
//     to read the current density state for the
//     density badge.
//   - Re-runs `scripts/count-vitest-tests.mjs`
//     to read the current vitest test count for
//     the test-totals line.
//   - Exits 1 if any of the above fails. Never
//     touches git.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, '..');
const SUMMARY_PATH = join(REPO_ROOT, 'coverage', 'coverage-summary.json');
const README_PATH = join(REPO_ROOT, 'README.md');
const OVERVIEW_PATH = join(REPO_ROOT, 'docs', 'overview.md');

// ----- CLI args ----------------------------------------------------------
//   --version v0.XX  → also update README "Status" + overview
//                       version header. Doc version is computed
//                       from the sub-version number.
const args = process.argv.slice(2);
let NEW_VERSION = null;
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--version' && args[i + 1]) {
    NEW_VERSION = args[i + 1];
    i++;
  }
}

function versionToDocVersion(v) {
  // v0.XX → v2.YY. Pattern: v0.65→v2.27, v0.66→v2.28, ...
  // Each sub-version bump advances the doc version by 1.
  // For pre-0.65 commits the offset varies; the
  // maintainer can hand-adjust if the formula drifts.
  const m = v.match(/^v?0\.(\d+)$/);
  if (!m) return null;
  const sub = parseInt(m[1], 10);
  return `v2.${27 + (sub - 65)}`;
}

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

// v0.66f — count vitest tests via a side script
let vitestCount = null;
const countResult = spawnSync('node', ['scripts/count-vitest-tests.mjs'], {
  cwd: REPO_ROOT,
  encoding: 'utf8',
  timeout: 120_000,
});
if (countResult.status === 0) {
  try {
    vitestCount = JSON.parse(countResult.stdout.trim()).tests;
  } catch {
    // ignore — keep the existing test-totals line
  }
}

// Format for badge: integer % or 1 decimal.
const fmt = (n) => (Math.round(n * 10) / 10).toFixed(1);

function colorFor(pct) {
  if (pct >= 80) return 'brightgreen';
  if (pct >= 60) return 'green';
  if (pct >= 40) return 'yellow';
  return 'red';
}

const COVERAGE_COLOR = colorFor(sPct);

// ----- Read README -------------------------------------------------------
if (!existsSync(README_PATH)) {
  console.error(`README.md not found at ${README_PATH}`);
  process.exit(1);
}

let readme = readFileSync(README_PATH, 'utf8');
const before = readme;

// 1. Coverage badge
const COV_BADGE_RE = /\[!\[coverage\]\(https:\/\/img\.shields\.io\/badge\/vitest%20cov-[^)]+\)\]\(\.\/vitest\.config\.ts\)/;
const newCovBadge = `[![coverage](https://img.shields.io/badge/vitest%20cov-${fmt(sPct)}%25%20stmts-${COVERAGE_COLOR})](./vitest.config.ts)`;
if (COV_BADGE_RE.test(readme)) {
  readme = readme.replace(COV_BADGE_RE, newCovBadge);
} else {
  console.error('README.md does not contain the expected coverage badge anchor. Run v0.63c first.');
  process.exit(1);
}

// 2. Coverage gate table line
const COVERAGE_LINE_RE = /\| Coverage gate \| vitest [0-9.]+% stmts \/ [0-9.]+% branches \/ [0-9.]+% funcs \/ [0-9.]+% lines .*?\|/;
const newCoverageLine = '| Coverage gate | vitest ' + fmt(sPct) + '% stmts / ' + fmt(bPct) + '% branches / ' + fmt(fPct) + '% funcs / ' + fmt(lPct) + '% lines (`vitest.config.ts`) |';
if (COVERAGE_LINE_RE.test(readme)) {
  readme = readme.replace(COVERAGE_LINE_RE, newCoverageLine);
} else {
  console.error('README.md does not contain the expected coverage gate line. Run v0.63c first.');
  process.exit(1);
}

// 3. Density badge — re-run check-comment-density.mjs
const DENSITY_RESULT = spawnSync('node', ['scripts/check-comment-density.mjs'], {
  cwd: REPO_ROOT,
  encoding: 'utf8',
});
if (DENSITY_RESULT.status !== 0) {
  console.error('Density check FAILED — not updating README density badge.');
  console.error(DENSITY_RESULT.stdout);
  console.error(DENSITY_RESULT.stderr);
  process.exit(1);
}

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

// 4. Test totals line — "319 cargo + 892 vitest + 85 Python = 1296/**"
//    v0.66f — vitest derived from scripts/count-vitest-tests.mjs
//    v0.82 — cargo + python + playwright derived dynamically too
//    (previously hardcoded — drifted from 319 → 348 etc).
//    The trailing `/**` is markdown bold close (NOT a count).
//
//    Each sub-count comes from a `--list` style command that
//    enumerates tests without running them. They're fast
//    (<5s each) and idempotent. The script exits 0 on a
//    transient failure for one of these (we keep the existing
//    totals line) and only exits 1 on a real parse error.
function countVia(cmd, args, parser) {
  const r = spawnSync(cmd, args, { cwd: REPO_ROOT, encoding: 'utf8', timeout: 120_000 });
  if (r.status !== 0) return null;
  try { return parser(r.stdout); } catch { return null; }
}

// cargo: `cargo test --lib -- --list` enumerates tests without
// running them. Output ends with: "N tests, 0 benchmarks".
// (No "test result: ok. N passed" line because tests don't run.)
//
// v0.82 — run from src-tauri/ where Cargo.toml lives. The local
// CI uses `rustup run 1.89 cargo` to match the CI toolchain; for
// just listing tests (not running them) the system cargo works
// because `cargo test --list` doesn't compile the lib.
const cargoCount = (() => {
  const r = spawnSync('cargo', ['test', '--lib', '--', '--list'], {
    cwd: join(REPO_ROOT, 'src-tauri'),
    encoding: 'utf8',
    timeout: 60_000,
  });
  if (r.status !== 0) return null;
  // "348 tests, 0 benchmarks"
  const m = r.stdout.match(/(\d+)\s+tests?,/);
  if (m) return parseInt(m[1], 10);
  // Fallback: count "test ..." lines
  return r.stdout.split('\n').filter((l) => /^\s*\S+::\S+: test\s*$/.test(l)).length;
})();

// Python (sidecar): `pytest --collect-only -q` prints
//   "N tests collected in T.Ts"
const pythonCount = countVia('python3', ['-m', 'pytest', '--collect-only', '-q'], (out) => {
  const m = out.match(/(\d+)\s+tests?\s+collected/);
  return m ? parseInt(m[1], 10) : null;
});

// Playwright: `playwright test --list` prints
//   "Total: N tests in M files"
const playwrightCount = countVia('npx', ['playwright', 'test', '--list'], (out) => {
  const m = out.match(/Total:\s+(\d+)\s+tests?\b/i);
  return m ? parseInt(m[1], 10) : null;
});

if (vitestCount != null && cargoCount != null && pythonCount != null) {
  // v0.82 — include Playwright in the totals. Backwards compat:
  //   - the regex still matches the 3-component line so old READMEs work
  //   - the new line has 4 components
  const pw = playwrightCount ?? 0;
  const total = cargoCount + vitestCount + pythonCount + pw;
  const TEST_TOTALS_RE_3 = /\| Test totals \|\s*\*\*\d+ cargo \+ \d+ vitest \+ \d+ Python = \d+\/\*\*\s*\|/;
  const TEST_TOTALS_RE_4 = /\| Test totals \|\s*\*\*\d+ cargo \+ \d+ vitest \+ \d+ Python \+ \d+ e2e = \d+\/\*\*\s*\|/;
  const newLine4 = `| Test totals | **${cargoCount} cargo + ${vitestCount} vitest + ${pythonCount} Python + ${pw} e2e = ${total}/** |`;
  const newLine3 = `| Test totals | **${cargoCount} cargo + ${vitestCount} vitest + ${pythonCount} Python = ${cargoCount + vitestCount + pythonCount}/** |`;
  if (TEST_TOTALS_RE_4.test(readme)) {
    readme = readme.replace(TEST_TOTALS_RE_4, newLine4);
  } else if (TEST_TOTALS_RE_3.test(readme)) {
    // v0.82 — first migration to 4-component form
    readme = readme.replace(TEST_TOTALS_RE_3, newLine4);
  }
  // If the regex doesn't match, leave the line as-is.
}

// ----- Optional: --version flag → also update README "Status" + overview.md
if (NEW_VERSION) {
  // 1. README "Status" line — "| Status | v0.XX[a-z]? — ..."
  const STATUS_LINE_RE = /\| Status \| v0\.\d+[a-z]? — [^|]+\|/;
  const newStatusLine = `| Status | ${NEW_VERSION} — auto-bumped by update-readme-coverage.mjs |`;
  if (STATUS_LINE_RE.test(readme)) {
    readme = readme.replace(STATUS_LINE_RE, newStatusLine);
    console.log(`  status line → ${NEW_VERSION}`);
  } else {
    console.error('README.md does not contain the expected status line. Run v0.63c first.');
    process.exit(1);
  }

  // 2. overview.md version header
  if (existsSync(OVERVIEW_PATH)) {
    const docVer = versionToDocVersion(NEW_VERSION);
    if (docVer) {
      let overview = readFileSync(OVERVIEW_PATH, 'utf8');
      const VER_HEADER_RE = new RegExp(`^> 版本：v2\\.\\d+ · .+$`, 'm');
      const newHeader = `> 版本：${docVer} · ${new Date().toISOString().slice(0, 10)} (${NEW_VERSION} auto-bumped)`;
      if (VER_HEADER_RE.test(overview)) {
        overview = overview.replace(VER_HEADER_RE, newHeader);
        writeFileSync(OVERVIEW_PATH, overview);
        console.log(`  overview.md header → ${docVer}`);
      } else {
        console.error('overview.md does not contain the expected version header. Update manually.');
      }
    }
  }
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
if (vitestCount != null) {
  console.log(`  test totals → ${vitestCount} vitest tests`);
}

console.log('');
console.log('Next: git add README.md docs/overview.md && git commit -m "..."');
