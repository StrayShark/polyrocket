#!/usr/bin/env node
// polyrocket — comment density checker (v0.61k).
//
// 按 coding-spec §5 的密度目标，扫描所有非测试源文件，
// 计算 /// / // / # 注释行占比，输出报告 + 失败条件。
//
// 密度目标（来自 docs/coding-spec.md §5）：
//   - Rust commands / domain / infra: ≥ 15%
//   - Rust platform:                  ≥ 10%
//   - TS routes / components / lib / stores: ≥ 10%
//   - TS types:                       ≥  5%
//   - Python polyrocket_sidecar:     ≥ 12%
//
// 失败阈值：上述 5 类模块中任何一类 < 50% 文件达标即 fail。
//  不到 50% 不卡死 PR（避免一次 PR 大量翻新），但作为
//  weekly cron 报告 + CI warning。
//
// 排除：
//   - *.test.* / test_*.py / test-setup.ts — 测试文件
//   - **/*.unused — archeology 归档
//   - coverage/ / dist/ / node_modules/ — 生成物
//   - 单文件 < 10 行的 too-small-to-judge
//
// 运行：node scripts/check-comment-density.mjs
// CI：在 .github/workflows/ci.yml 的 guards job 里作为 warning 运行。
import { readFileSync, statSync, readdirSync } from 'node:fs';
import { join, relative, extname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = __filename.replace(/\/[^/]+$/, '');
const REPO_ROOT = join(__dirname, '..');

// ----- Density targets per coding-spec §5 -------------------------------
// v0.68c round 1: rust-commands-domain-infra 10%→15%, ts-routes 5%→10%.
// v0.70 round 2: ts-routes-components-lib 10%→15% (avg ratio 21.9%;
//       58/78 files at 15%, 20/78 below). Bumping forces more
//       /// / JSDoc on the bottom 5 (PolymarketStep 5.5%, Copy 5.7%,
//       Markets 5.8%, History 6.5%, format 6.7%) — done incrementally
//       in v0.71+ sub-versions.
const TARGETS = [
  { name: 'rust-commands-domain-infra',  pattern: /^src-tauri\/src\/(commands|domain|infra)\//,         min: 15, lang: 'rust' },
  { name: 'rust-platform',                pattern: /^src-tauri\/src\/platform\//,                        min: 10, lang: 'rust' },
  { name: 'ts-routes-components-lib',     pattern: /^src\/(routes|components|lib|stores)\//,            min: 15, lang: 'ts'   },
  { name: 'ts-types',                     pattern: /^src\/types\//,                                      min: 5,  lang: 'ts'   },
  { name: 'py-sidecar',                   pattern: /^sidecar\/polyrocket_sidecar\//,                     min: 12, lang: 'py'   },
];

// ----- Skip patterns ------------------------------------------------------
// Per coding-spec §1.2 / §6: pure-presentation components
// (Button/Card/Input/Pill/Spinner) are explicitly exempt from
// the docstring requirement — they're thin wrappers around
// HTML elements with className variants.
const SKIP_FILES = [
  /\/coverage\//,
  /\/node_modules\//,
  /\/dist\//,
  /\/target\//,
  /\.unused$/,                        // archeology
  /\.test\.(ts|tsx|rs|py)$/,         // vitest / cargo / python tests
  /\/test_[^/]+\.py$/,               // python test_*.py
  /^test_/,                           // python test_*.py at top
  /\/test-setup\.ts$/,                // vitest setup
  /__main__\.py$/,                   // entrypoint, no public API to doc
  /__init__\.py$/,                   // package init
  // TS base components: presentation wrappers (spec §6 exempt)
  /\/components\/base\/(Button|Card|Input|Pill|Spinner|Toggle)\.tsx$/,
];

const MIN_FILE_LINES = 10;

// ----- Comment regexes ----------------------------------------------------
const COMMENT_RE = {
  rust: /^\s*(\/\/\/|\/\/|\/\*|\*)/,
  ts:   /^\s*(\/\/|\/\*\*|\/\*|\*)/,
  py:   /^\s*(#|"""|''')/,
};

function countComments(filePath, lang) {
  const text = readFileSync(filePath, 'utf8');
  const lines = text.split('\n');
  let total = 0;
  let comments = 0;
  let inBlockComment = false;
  for (const line of lines) {
    if (line.trim().length === 0) continue;  // skip blank lines (denominator only)
    total++;
    if (inBlockComment) {
      comments++;
      if (line.includes('*/')) inBlockComment = false;
      continue;
    }
    if (line.trim().startsWith('/*')) {
      comments++;
      if (line.includes('*/') && line.indexOf('*/') > line.indexOf('/*')) {
        // single-line block comment
      } else {
        inBlockComment = true;
      }
      continue;
    }
    if (COMMENT_RE[lang].test(line)) {
      comments++;
    }
  }
  return { total, comments, ratio: total > 0 ? comments * 100 / total : 0 };
}

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith('.') && entry.name !== '.unused') continue;
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (['node_modules', 'target', 'dist', 'coverage', '__pycache__'].includes(entry.name)) continue;
      out.push(...walk(full));
    } else {
      out.push(full);
    }
  }
  return out;
}

const allFiles = walk(REPO_ROOT);
const targets = TARGETS.map((t) => ({ ...t, files: [] }));

for (const file of allFiles) {
  const rel = relative(REPO_ROOT, file);
  if (SKIP_FILES.some((p) => p.test(rel))) continue;
  const ext = extname(file);
  const lang =
    ext === '.rs' ? 'rust' :
    ext === '.ts' || ext === '.tsx' ? 'ts' :
    ext === '.py' ? 'py' : null;
  if (!lang) continue;
  const stat = statSync(file);
  if (stat.size < 200) continue; // skip near-empty

  const matched = targets.find((t) => t.lang === lang && t.pattern.test(rel + (rel.endsWith('/') ? '' : '/')));
  if (!matched) continue;

  const lineCount = readFileSync(file, 'utf8').split('\n').length;
  if (lineCount < MIN_FILE_LINES) continue;

  const { total, comments, ratio } = countComments(file, lang);
  matched.files.push({ path: rel, total, comments, ratio });
}

// ----- Report -------------------------------------------------------------
let fail = false;
console.log('=== polyrocket comment density report (v0.61k) ===\n');
for (const t of targets) {
  if (t.files.length === 0) {
    console.log(`[${t.name}] (no files matched)`);
    continue;
  }
  const passing = t.files.filter((f) => f.ratio >= t.min).length;
  const passPct = (passing * 100 / t.files.length).toFixed(1);
  const avgRatio = (t.files.reduce((s, f) => s + f.ratio, 0) / t.files.length).toFixed(1);
  const ok = passing / t.files.length >= 0.5;
  if (!ok) fail = true;
  const status = ok ? 'PASS' : 'FAIL';
  console.log(`[${status}] ${t.name} (target ${t.min}%+, ${t.lang})`);
  console.log(`        ${passing}/${t.files.length} files passing (${passPct}%); avg ratio ${avgRatio}%`);
  // Print bottom-5 failing files
  const failing = t.files.filter((f) => f.ratio < t.min).sort((a, b) => a.ratio - b.ratio).slice(0, 5);
  if (failing.length > 0) {
    console.log(`        Bottom 5 failing:`);
    for (const f of failing) {
      console.log(`          ${f.ratio.toFixed(1).padStart(5)}%  ${f.comments}/${f.total}  ${f.path}`);
    }
  }
  console.log();
}

if (fail) {
  console.error('Comment density check FAILED (≥1 category < 50% passing).');
  process.exit(1);
} else {
  console.log('All categories PASS (≥50% files meet target).');
}
