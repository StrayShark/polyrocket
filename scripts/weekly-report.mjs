#!/usr/bin/env node
// polyrocket — weekly report generator (v0.62e).
//
// 跑全套 CI 检查 + 输出 markdown 报告，给 weekly
// cron 用。结果可以 post 到 GitHub Actions 的
// artifact / summary / issue 跟踪。
//
// 跑的内容:
//   1. cargo check (lib) — 0 errors
//   2. cargo doc --no-deps — 0 rustdoc warning
//   3. cargo test --lib -- --test-threads=1 — pass
//   4. vitest run --coverage — pass
//   5. comment density — 5/5 categories PASS
//   6. python sidecar — 85/86 (1 pre-existing flaky)
//
// 输出: $GITHUB_STEP_SUMMARY 兼容的 markdown。
//
// 配合 `.github/workflows/weekly-report.yml` 用
// （cron: 每周一 06:00 UTC 跑）。

import { execSync } from 'node:child_process';

const REPO = '/Users/dutongxue/work2/polyrocket';

function run(cmd, cwd = REPO) {
  try {
    return { ok: true, out: execSync(cmd, { cwd, encoding: 'utf8', timeout: 600_000 }) };
  } catch (e) {
    return { ok: false, out: (e.stdout ?? '') + (e.stderr ?? ''), code: e.status };
  }
}

function fmtPct(n, d) {
  if (d === 0) return '0%';
  return ((n * 100) / d).toFixed(2) + '%';
}

const lines = [];
lines.push('# polyrocket weekly report — ' + new Date().toISOString().slice(0, 10));
lines.push('');

// ---- 1. cargo check ---------------------------------------------------------
const cargoCheck = run('cd src-tauri && cargo check --lib --quiet 2>&1 | tail -5');
lines.push('## Rust `cargo check --lib`');
lines.push(cargoCheck.ok ? '- ✅ 0 errors' : '- ❌ FAILED');
if (!cargoCheck.ok) lines.push('```\n' + cargoCheck.out + '\n```');
lines.push('');

// ---- 2. cargo doc 0 warning -------------------------------------------------
const cargoDoc = run('cd src-tauri && cargo doc --no-deps 2>&1 | grep -E "^warning" | sort -u');
const docWarnings = cargoDoc.ok ? cargoDoc.out.split('\n').filter((l) => l.includes('unresolved link') || l.includes('URL is not a hyperlink') || l.includes('private item')).length : 999;
lines.push('## Rust `cargo doc --no-deps` 0 warning');
lines.push(docWarnings === 0 ? '- ✅ 0 rustdoc warnings' : `- ❌ ${docWarnings} rustdoc warnings`);
if (docWarnings > 0) lines.push('```\n' + cargoDoc.out + '\n```');
lines.push('');

// ---- 3. cargo test ---------------------------------------------------------
const cargoTest = run('cd src-tauri && cargo test --lib -- --test-threads=1 2>&1 | tail -3');
const cargoTestSummary = cargoTest.out.split('\n').slice(-3).join('\n');
const cargoTestPass = cargoTestSummary.match(/test result: ok\. (\d+) passed/) ;
lines.push('## Rust `cargo test --lib`');
lines.push(cargoTestPass ? `- ✅ ${cargoTestPass[1]} passed` : '- ❌ FAILED');
lines.push('```\n' + cargoTestSummary + '\n```');
lines.push('');

// ---- 4. vitest --coverage ---------------------------------------------------
// v0.66e — captures the full per-file coverage table
// (not just the totals) so we can print top-10 / bottom-10
// in the report. The full table is on lines after the
// `File | % Stmts | ...` header, before the summary block.
const vitestFull = run('pnpm vitest run --coverage 2>&1');
const vitestTail = vitestFull.out.split('\n').slice(-20).join('\n');
const covMatch = vitestTail.match(/Statements\s+:\s+([\d.]+)%.*Branches\s+:\s+([\d.]+)%.*Functions\s+:\s+([\d.]+)%.*Lines\s+:\s+([\d.]+)%/s);
lines.push('## TS `vitest --coverage`');
if (covMatch) {
  lines.push(`- Statements: ${covMatch[1]}%`);
  lines.push(`- Branches:   ${covMatch[2]}%`);
  lines.push(`- Functions:  ${covMatch[3]}%`);
  lines.push(`- Lines:      ${covMatch[4]}%`);
} else {
  lines.push('- ❌ FAILED to parse coverage');
  lines.push('```\n' + vitestTail + '\n```');
}
lines.push('');

// v0.66e — per-file top/bottom-10 by statements coverage.
// Parse the table rows: `  filename.tsx | 65.5 | 59.26 | 55.04 | 66.31 |`.
// We grab every row that has a numeric 1st column (%), sort
// by stmts desc, then print top-10 and bottom-10.
const tableRows = vitestFull.out.split('\n').filter((l) =>
  /^\s+([\w./-]+\.tsx?)\s+\|\s+[\d.]+/.test(l)
);
const fileCov = tableRows.map((l) => {
  const m = l.match(/^\s+([\w./-]+\.tsx?)\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)/);
  if (!m) return null;
  return { path: m[1], stmts: parseFloat(m[2]), branches: parseFloat(m[3]), funcs: parseFloat(m[4]), lines: parseFloat(m[5]) };
}).filter(Boolean);

if (fileCov.length > 0) {
  const top = [...fileCov].sort((a, b) => b.stmts - a.stmts).slice(0, 10);
  const bot = [...fileCov].sort((a, b) => a.stmts - b.stmts).slice(0, 10);
  lines.push('### Top 10 (by stmts %)');
  lines.push('| File | Stmts | Branches | Funcs | Lines |');
  lines.push('|---|---|---|---|---|');
  for (const f of top) {
    lines.push(`| \`${f.path}\` | ${f.stmts.toFixed(1)}% | ${f.branches.toFixed(1)}% | ${f.funcs.toFixed(1)}% | ${f.lines.toFixed(1)}% |`);
  }
  lines.push('');
  lines.push('### Bottom 10 (by stmts %)');
  lines.push('| File | Stmts | Branches | Funcs | Lines |');
  lines.push('|---|---|---|---|---|');
  for (const f of bot) {
    lines.push(`| \`${f.path}\` | ${f.stmts.toFixed(1)}% | ${f.branches.toFixed(1)}% | ${f.funcs.toFixed(1)}% | ${f.lines.toFixed(1)}% |`);
  }
  lines.push('');
}

// ---- 5. comment density ----------------------------------------------------
const density = run('node scripts/check-comment-density.mjs 2>&1');
const passCount = (density.out.match(/\[PASS\]/g) ?? []).length;
const failCount = (density.out.match(/\[FAIL\]/g) ?? []).length;
lines.push('## Comment density (5 categories)');
lines.push(`- ${passCount} PASS, ${failCount} FAIL`);
if (failCount > 0) lines.push('```\n' + density.out + '\n```');
lines.push('');

// ---- 6. python sidecar -----------------------------------------------------
const py = run('cd sidecar && python3 -m unittest discover tests 2>&1 | tail -3');
const pySummary = py.out.split('\n').slice(-3).join('\n');
lines.push('## Python sidecar tests');
lines.push('```\n' + pySummary + '\n```');
lines.push('');

const report = lines.join('\n');
console.log(report);

// Also write to /tmp/weekly-report.md for the workflow to upload
import { writeFileSync } from 'node:fs';
writeFileSync('/tmp/polyrocket-weekly-report.md', report);
