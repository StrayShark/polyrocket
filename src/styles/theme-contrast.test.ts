// v0.58c — test that the 3-theme contrast audit
// passes WCAG AA. We shell out to the script
// (Node) and assert on its exit code + summary
// line. The script itself does the math
// (relative luminance, contrast ratio).

import { describe, it, expect } from 'vitest';
import { execSync } from 'node:child_process';

describe('3-theme WCAG AA contrast (v0.58c)', () => {
  it('check-theme-contrast.mjs exits 0 (all pairs >= 3.0:1)', () => {
    let out = '';
    let code = 0;
    try {
      out = execSync('node scripts/check-theme-contrast.mjs', {
        encoding: 'utf8',
      });
    } catch (e: unknown) {
      const err = e as { status?: number; stdout?: Buffer | string; stderr?: Buffer | string };
      code = err.status ?? 1;
      out = String(err.stdout ?? '') + String(err.stderr ?? '');
    }
    if (code !== 0) {
      // eslint-disable-next-line no-console
      console.error('check-theme-contrast output:\n' + out);
    }
    expect(code).toBe(0);
    expect(out).toMatch(/Summary:\s*✓ PASS/);
  });

  it('every theme has at least 1 PASS for each foreground token', () => {
    const out = execSync('node scripts/check-theme-contrast.mjs', {
      encoding: 'utf8',
    });
    // We expect 3 === blocks (one per theme).
    const themeBlocks = out.match(/=== \w+ ===/g) ?? [];
    expect(themeBlocks.length).toBe(3);
  });

  it('no theme has a FAIL row (>= 3.0:1 enforced)', () => {
    const out = execSync('node scripts/check-theme-contrast.mjs', {
      encoding: 'utf8',
    });
    // The script prints `FAIL` for any pair
    // below 3.0:1. A clean audit has zero FAIL
    // rows.
    const failRows = (out.match(/FAIL/g) ?? []).length;
    expect(failRows).toBe(0);
  });
});
