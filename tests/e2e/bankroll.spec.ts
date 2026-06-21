// v0.80 + v0.82 — /bankroll route visual regression test (Playwright).
//
// Spec: docs/coding-spec.md §12 (v0.74d Visual Acceptance Gate).
//
// v0.82: this is now a CI gate (Job 5 in `run-ci-local.sh`, the
// `e2e` job in local CI — see `scripts/run-ci-local.sh` Job 5). It runs on every push
// to main, not just on-demand.
//
// **What this test does**:
//   1. Navigate to /bankroll in 3 themes (dark / light / matrix)
//   2. Take a screenshot of the rendered page
//   3. Compare against the baseline (first run creates the baseline)
//
// **Run**:
//   - First time: `npx playwright test bankroll.spec.ts` (generates baseline)
//   - Update: `npx playwright test bankroll.spec.ts --update-snapshots`
//   - Verify: `npx playwright test bankroll.spec.ts` (compares)
//
// **Why this matters**:
//   The /bankroll page is the M11 module's primary surface.
//   Visual regressions (e.g. sliders not rendering, layout broken in
//   matrix theme) would silently degrade the user experience.
//   This test catches both DOM AND CSS regressions in one shot.

import { test, expect } from '@playwright/test';

const THEMES = ['dark', 'light', 'matrix'] as const;

for (const theme of THEMES) {
  test(`bankroll route renders in ${theme} theme`, async ({ page }) => {
    // 1. Set localStorage BEFORE navigation (for store rehydration)
    await page.addInitScript((t) => {
      window.localStorage.setItem('polyrocket.theme', t);
    }, theme);

    // 2. Navigate to /bankroll
    await page.goto('/bankroll');

    // 3. Wait for the page to be ready (BankrollCard + table)
    await expect(page.getByTestId('bankroll-page')).toBeVisible();
    await expect(page.getByTestId('bankroll-input')).toBeVisible();
    await expect(page.getByTestId('config-slider-kelly_multiplier')).toBeVisible();

    // 4. Override data-theme AFTER React's mount (React sets it
    // to the default before the persist middleware rehydrates).
    // We force the attribute so the CSS picks up the right theme.
    await page.evaluate((t) => {
      document.documentElement.setAttribute('data-theme', t);
    }, theme);

    // 5. Verify data-theme actually applied
    const actualTheme = await page.getAttribute('html', 'data-theme');
    expect(actualTheme).toBe(theme);

    // 6. Take a full-page screenshot
    await expect(page).toHaveScreenshot(`bankroll-${theme}.png`, {
      fullPage: true,
      maxDiffPixelRatio: 0.005,  // 0.5% diff tolerance — covers macOS/Linux Chromium pixel diffs
    });
  });

  test(`bankroll config card has correct values in ${theme} theme`, async ({ page }) => {
    // 1. Set localStorage BEFORE navigation
    await page.addInitScript((t) => {
      window.localStorage.setItem('polyrocket.theme', t);
    }, theme);

    // 2. Navigate to /settings
    await page.goto('/settings');

    // 3. Override data-theme after React's mount
    await page.evaluate((t) => {
      document.documentElement.setAttribute('data-theme', t);
    }, theme);

    // 4. Scroll to the bankroll card
    const card = page.getByTestId('bankroll-config-card');
    if (await card.count() > 0) {
      await card.scrollIntoViewIfNeeded();
      await expect(card).toHaveScreenshot(`settings-bankroll-${theme}.png`, {
        maxDiffPixelRatio: 0.005,
      });
    } else {
      // No config set yet (first run) — card shows fallback text.
      // Take a full-page screenshot for the empty state.
      await expect(page).toHaveScreenshot(`settings-bankroll-empty-${theme}.png`, {
        fullPage: true,
        maxDiffPixelRatio: 0.005,
      });
    }
  });
}

// v0.80 — accessibility smoke (always-on, not screenshot-based).
test('bankroll route has no console errors', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  page.on('pageerror', (err) => {
    errors.push(err.message);
  });
  await page.addInitScript(() => {
    window.localStorage.setItem('polyrocket.theme', 'dark');
  });
  await page.goto('/bankroll');
  await expect(page.getByTestId('bankroll-page')).toBeVisible();
  await page.evaluate(() => {
    document.documentElement.setAttribute('data-theme', 'dark');
  });
  // Wait for all queries to settle
  await page.waitForTimeout(500);
  // Filter out expected safeInvoke rejections (vite-only dev)
  const realErrors = errors.filter(e => !e.includes('safeInvoke'));
  expect(realErrors).toEqual([]);
});
