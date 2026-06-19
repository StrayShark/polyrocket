// v0.80 — Playwright config (visual regression for /bankroll).
//
// Spec: docs/coding-spec.md §12 (v0.74d Visual Acceptance Gate, road-mapped
// for v0.80).
//
// **Setup**:
//   1. `pnpm add -D @playwright/test` (done in v0.80)
//   2. `npx playwright install chromium` (downloads ~200MB — run once per CI runner)
//   3. `pnpm test:e2e` runs all tests in `tests/e2e/`
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
//
// **Why screenshots not DOM assertions**:
//   - v0.74d nav-item bug was caught ONLY by visual review (8 versions
//     broken). happy-dom doesn't render real CSS, so DOM checks would
//     miss CSS-related visual regressions. Screenshots catch both DOM
//     AND CSS regressions.

import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  // v0.80 — only run on-demand via `pnpm test:e2e` (not in CI yet).
  // Future v0.80+ will add this to the `test` script in CI.
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
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    // v0.80 — start the Vite dev server before tests.
    // In v0.80+ this would point to the built dist/.
    command: 'pnpm dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
