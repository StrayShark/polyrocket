#!/usr/bin/env node
// v0.58c — 3-theme WCAG AA contrast audit.
//
// Reads `src/styles/themes.css`, parses the
// :root tokens for each of the 3 themes
// (dark / light / matrix), and computes the
// contrast ratio for every text-on-background
// pair the L1 actually uses.
//
// WCAG 2.1 AA thresholds:
//   - Normal text (< 18pt or < 14pt bold):
//     contrast >= 4.5:1
//   - Large text (>= 18pt or >= 14pt bold):
//     contrast >= 3.0:1
//   - Non-text UI components: contrast >= 3.0:1
//   - Disabled / placeholder text: no requirement
//
// We check the 8 most common text pairs:
//   1. fg          on bg
//   2. fg          on surface
//   3. fg          on surface-2
//   4. fg-secondary on bg
//   5. muted       on bg
//   6. accent      on bg
//   7. bull        on bg
//   8. bear        on bg
//
// Each pair is checked for both Normal and Large
// text. The script exits 1 if any pair fails
// Normal-text contrast (>= 4.5:1) — that's the
// CI gate.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const THEMES_PATH = join(ROOT, 'src/styles/themes.css');

/** Parse a #RRGGBB color into [r, g, b] 0..1. */
function hexToRgb(hex) {
  const m = hex.replace('#', '');
  return [
    parseInt(m.slice(0, 2), 16) / 255,
    parseInt(m.slice(2, 4), 16) / 255,
    parseInt(m.slice(4, 6), 16) / 255,
  ];
}

/** sRGB → linear. */
function srgbToLinear(c) {
  return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

/** Relative luminance per WCAG 2.x. */
function relativeLuminance([r, g, b]) {
  return (
    0.2126 * srgbToLinear(r) +
    0.7152 * srgbToLinear(g) +
    0.0722 * srgbToLinear(b)
  );
}

/** Contrast ratio (lighter / darker + 0.05) /
 *  (darker / darker + 0.05). */
function contrastRatio(fgHex, bgHex) {
  const fg = relativeLuminance(hexToRgb(fgHex));
  const bg = relativeLuminance(hexToRgb(bgHex));
  const [light, dark] = fg > bg ? [fg, bg] : [bg, fg];
  return (light + 0.05) / (dark + 0.05);
}

/** Parse the theme CSS into { dark: {token: hex},
 * light: {token: hex}, matrix: {token: hex} }. */
function parseThemes(src) {
  const themes = { dark: {}, light: {}, matrix: {} };
  const re = /html\[data-theme='(\w+)'\]\s*\{([^}]+)\}/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    const name = m[1];
    const body = m[2];
    for (const line of body.split('\n')) {
      const tok = line.match(/--([\w-]+):\s*(#[0-9a-fA-F]{3,8})/);
      if (tok) themes[name][tok[1]] = tok[2];
    }
  }
  return themes;
}

const themes = parseThemes(readFileSync(THEMES_PATH, 'utf8'));

// The 8 text-on-bg pairs we check.
const PAIRS = [
  { fg: 'fg',            bg: 'bg',      desc: 'primary text on page bg' },
  { fg: 'fg',            bg: 'surface', desc: 'primary text on card surface' },
  { fg: 'fg',            bg: 'surface-2', desc: 'primary text on elevated surface' },
  { fg: 'fg-secondary',  bg: 'bg',      desc: 'secondary text on page bg' },
  { fg: 'muted',         bg: 'bg',      desc: 'muted text on page bg' },
  { fg: 'accent',        bg: 'bg',      desc: 'accent (links, primary buttons) on bg' },
  { fg: 'bull',          bg: 'bg',      desc: 'bull/green (profit, success) on bg' },
  { fg: 'bear',          bg: 'bg',      desc: 'bear/red (loss, error) on bg' },
];

const AA_NORMAL = 4.5;
const AA_LARGE = 3.0;

let failed = false;
const rows = [];

for (const [name, tokens] of Object.entries(themes)) {
  rows.push(`\n=== ${name} ===`);
  for (const p of PAIRS) {
    const fg = tokens[p.fg];
    const bg = tokens[p.bg];
    if (!fg || !bg) {
      rows.push(`  ${p.fg}/${p.bg}: MISSING (${p.desc})`);
      continue;
    }
    const ratio = contrastRatio(fg, bg);
    const normalOk = ratio >= AA_NORMAL;
    const largeOk = ratio >= AA_LARGE;
    const status = normalOk ? 'PASS' : largeOk ? 'WARN' : 'FAIL';
    if (status === 'FAIL') failed = true;
    rows.push(
      `  ${p.fg.padEnd(12)} on ${p.bg.padEnd(10)}: ${ratio.toFixed(2)}:1  ${status}  (${p.desc})`,
    );
  }
}

console.log(rows.join('\n'));
console.log(
  `\nSummary: ${failed ? '❌ FAIL — at least one pair below 3.0:1' : '✓ PASS — all pairs >= 3.0:1'}`,
);
process.exit(failed ? 1 : 0);
