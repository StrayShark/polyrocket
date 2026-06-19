// v0.74d — test for scripts/check-class-coverage.mjs (unit test).
//
// Validates:
//   1. Pass: no missing classes
//   2. Fail: detects a missing custom class
//   3. Pass: ignores Tailwind utility classes (text-[13px], hover:..., etc.)
//   4. Pass: ignores lucide-* icon classes
//   5. Pass: handles cn('a', 'b', cond && 'c') template

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const SCRIPT = join(process.cwd(), 'scripts/check-class-coverage.mjs');

/**
 * Run the linter in a temporary directory with synthesized files.
 * Returns { exitCode, stdout }.
 */
function runLinter(opts: { tsxFiles: { path: string; content: string }[]; cssFiles: { path: string; content: string }[] }): { exitCode: number; stdout: string } {
  const tmpDir = join(tmpdir(), `class-cov-test-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`);
  mkdirSync(tmpDir, { recursive: true });
  mkdirSync(join(tmpDir, 'src/styles'), { recursive: true });

  // Default: write the script into the temp dir as a sibling so node can
  // find it (we run node from the temp dir).
  // Actually simpler: run the script with cwd = tmpDir and let it
  // walk relative paths from there.

  try {
    // Create the .tsx and .css files
    for (const f of opts.tsxFiles) {
      const full = join(tmpDir, f.path);
      mkdirSync(join(full, '..'), { recursive: true });
      writeFileSync(full, f.content);
    }
    for (const f of opts.cssFiles) {
      const full = join(tmpDir, f.path);
      mkdirSync(join(full, '..'), { recursive: true });
      writeFileSync(full, f.content);
    }

    // Run the linter with cwd = tmpDir
    const result = spawnSync('node', [SCRIPT], {
      cwd: tmpDir,
      encoding: 'utf8',
      timeout: 30000,
    });

    return { exitCode: result.status ?? -1, stdout: result.stdout + result.stderr };
  } finally {
    if (existsSync(tmpDir)) rmSync(tmpDir, { recursive: true, force: true });
  }
}

describe('check-class-coverage.mjs', () => {
  it('PASS: all custom classes have CSS rules', () => {
    const r = runLinter({
      tsxFiles: [
        {
          path: 'src/components/Button.tsx',
          content: `
            export function Button() {
              return <div className="nav-item sidebar-row">click</div>;
            }
          `,
        },
      ],
      cssFiles: [
        {
          path: 'src/styles/globals.css',
          content: `
            .nav-item { display: flex; }
            .sidebar-row { padding: 4px; }
          `,
        },
      ],
    });
    expect(r.exitCode).toBe(0);
    expect(r.stdout).toMatch(/✓/);
  });

  it('FAIL: missing custom class detected', () => {
    const r = runLinter({
      tsxFiles: [
        {
          path: 'src/components/Button.tsx',
          content: `<div className="nav-item sidebar-row">x</div>`,
        },
      ],
      cssFiles: [
        {
          path: 'src/styles/globals.css',
          content: `.nav-item { display: flex; }`,
        },
      ],
    });
    expect(r.exitCode).toBe(1);
    expect(r.stdout).toMatch(/sidebar-row/);
  });

  it('PASS: ignores Tailwind utility classes', () => {
    const r = runLinter({
      tsxFiles: [
        {
          path: 'src/components/Card.tsx',
          content: `
            <div className="flex bg-accent text-[13px] hover:bg-accent-hover
              md:grid-cols-2 -mt-4 focus-visible:ring-2 border rounded">
              x
            </div>
          `,
        },
      ],
      cssFiles: [
        { path: 'src/styles/globals.css', content: '/* nothing */' },
      ],
    });
    expect(r.exitCode).toBe(0);
    expect(r.stdout).toMatch(/✓/);
  });

  it('PASS: ignores lucide-* icon classes', () => {
    const r = runLinter({
      tsxFiles: [
        {
          path: 'src/components/Icon.tsx',
          content: `<svg className="lucide lucide-home w-3.5 h-3.5">x</svg>`,
        },
      ],
      cssFiles: [
        { path: 'src/styles/globals.css', content: '/* nothing */' },
      ],
    });
    expect(r.exitCode).toBe(0);
  });

  it('PASS: handles className={cn("a", cond && "b")}', () => {
    const r = runLinter({
      tsxFiles: [
        {
          path: 'src/components/Item.tsx',
          content: `
            import { cn } from '@/lib/cn';
            export function Item({ active }: { active: boolean }) {
              return <div className={cn("nav-item", active && "is-active")}>x</div>;
            }
          `,
        },
      ],
      cssFiles: [
        {
          path: 'src/styles/globals.css',
          content: `
            .nav-item { display: flex; }
            .nav-item.is-active { color: red; }
          `,
        },
      ],
    });
    expect(r.exitCode).toBe(0);
  });

  it('FAIL: lists all missing classes (not just first)', () => {
    const r = runLinter({
      tsxFiles: [
        {
          path: 'src/components/X.tsx',
          content: `
            <div className="missing-one">x</div>
            <div className="missing-two">x</div>
            <div className="missing-three">x</div>
          `,
        },
      ],
      cssFiles: [
        { path: 'src/styles/globals.css', content: '/* empty */' },
      ],
    });
    expect(r.exitCode).toBe(1);
    expect(r.stdout).toMatch(/missing-one/);
    expect(r.stdout).toMatch(/missing-two/);
    expect(r.stdout).toMatch(/missing-three/);
  });

  it('PASS: ignores "active" (sub-state of .nav-item.active)', () => {
    // 'active' is used as `className={cn("nav-item", isActive && "active")}`
    // The full selector is `.nav-item.active` which is in CSS.
    // The linter should NOT report `active` as missing because it's
    // a sub-state modifier, not a standalone class.
    const r = runLinter({
      tsxFiles: [
        {
          path: 'src/components/Item.tsx',
          content: `<div className="nav-item active">x</div>`,
        },
      ],
      cssFiles: [
        {
          path: 'src/styles/globals.css',
          content: `
            .nav-item { display: flex; }
            .nav-item.active { color: red; }
          `,
        },
      ],
    });
    expect(r.exitCode).toBe(0);
  });
});