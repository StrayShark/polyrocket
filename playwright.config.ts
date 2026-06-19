// v0.82 — Playwright config (visual regression for /bankroll).
//
// Spec: docs/coding-spec.md §12 (v0.74d Visual Acceptance Gate).
//
// **Setup**:
//   1. `pnpm add -D @playwright/test` (v0.80)
//   2. macOS: reuses puppeteer's chrome-headless-shell from
//      `~/.cache/puppeteer/chrome-headless-shell/` (no extra download).
//   3. Linux: `npx playwright install --with-deps chromium` (CI runner
//      does this in `.github/workflows/ci.yml` job `e2e`).
//   4. Override: set `PLAYWRIGHT_EXECUTABLE_PATH` env var to force a
//      specific binary (e.g. a system chromium on a Linux dev box).
//
// **Screenshots**:
//   - First run: generates `tests/e2e/__screenshots__/<test>.png` (baseline)
//   - Subsequent runs: compares against baseline. Diff > 0.1% → fail.
//   - Update baseline: `pnpm test:e2e --update-snapshots`
//
// **What this covers**:
//   - /bankroll route renders in 3 themes (dark / light / matrix)
//   - Config sliders are visible + have correct values
//   - BankrollCard shows 4 tiles
//   - AllocationTable renders the table
//   - Apply button is present + correctly disabled when no items
//   - No console errors on /bankroll (accessibility smoke)
//
// **Why screenshots not DOM assertions**:
//   - v0.74d nav-item bug was caught ONLY by visual review (8 versions
//     broken). happy-dom doesn't render real CSS, so DOM checks would
//     miss CSS-related visual regressions. Screenshots catch both DOM
//     AND CSS regressions.
//
// **v0.82 cross-platform**:
//   Previously hardcoded the macOS puppeteer cache path. Now resolves
//   the chromium binary per-platform:
//     - macOS arm64: puppeteer's chrome-headless-shell (no download)
//     - linux: Playwright's own chromium (CI installs via `playwright install`)
//     - other / explicit: `PLAYWRIGHT_EXECUTABLE_PATH` env var wins.

import { defineConfig, devices } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

/**
 * Resolve a chromium executable path that works on the current host.
 *
 * Order of resolution (first hit wins):
 *   1. `PLAYWRIGHT_EXECUTABLE_PATH` env var (explicit override)
 *   2. macOS arm64: puppeteer's cached chrome-headless-shell
 *   3. linux x64: Playwright's default chromium
 *   4. fallback: empty string → Playwright's own lookup (will error
 *      with a clear "playwright install" hint if not yet downloaded)
 */
function resolveChromiumPath(): string | undefined {
  const override = process.env.PLAYWRIGHT_EXECUTABLE_PATH;
  if (override && fs.existsSync(override)) {
    return override;
  }

  if (process.platform === 'darwin' && process.arch === 'arm64') {
    // Puppeteer's chrome-headless-shell cache layout (pnpm 11 / mac arm).
    // The version subdir changes per chromium version; we just pick the
    // newest one if multiple are present, otherwise fall through.
    const cacheRoot = path.join(
      os.homedir(),
      '.cache',
      'puppeteer',
      'chrome-headless-shell',
    );
    if (fs.existsSync(cacheRoot)) {
      const versions = fs
        .readdirSync(cacheRoot)
        .filter((name) => /^mac_arm-\d+\./.test(name))
        .sort()
        .reverse();
      for (const ver of versions) {
        const candidate = path.join(
          cacheRoot,
          ver,
          'chrome-headless-shell-mac-arm64',
          'chrome-headless-shell',
        );
        if (fs.existsSync(candidate)) {
          return candidate;
        }
      }
    }
  }

  // Linux + other: let Playwright use its own bundled chromium. The CI
  // job installs it via `npx playwright install --with-deps chromium`.
  return undefined;
}

export default defineConfig({
  testDir: './tests/e2e',
  // v0.82 — Playwright e2e is now a CI gate (Job 5 in run-ci-local.sh
  // and the `e2e` job in ci.yml). Was previously on-demand only.
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        launchOptions: (() => {
          const exec = resolveChromiumPath();
          return exec ? { executablePath: exec } : {};
        })(),
      },
    },
  ],
  webServer: {
    // v0.80 — start the Vite dev server before tests.
    // v0.82: also used by CI job `e2e` (set
    //   PLAYWRIGHT_WEBSERVER_COMMAND=... to override; default is
    //   `pnpm dev` everywhere for visual-diff consistency — both
    //   baseline PNGs and CI runs use the dev server).
    command: process.env.PLAYWRIGHT_WEBSERVER_COMMAND ?? 'pnpm dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
