import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  test: {
    // .test.tsx files declare their own environment with `// @vitest-environment`
    // at the top (see ErrorBoundary.test.tsx, KbdHelpDialog.test.tsx).
    // This is the modern Vitest pattern and works across versions.
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    globals: false,
    setupFiles: ['./src/test-setup.ts'],
    // v0.60b — coverage gate. Vitest exits
    // non-zero when any of the thresholds
    // below the cutoff. We ratchet up by
    // ~1% per sub-version as we fill in
    // routes + business components.
    //
    // v0.62a.2: 64/57/52/64 (504 tests)
    // v0.63a:   65/58/55/65 (504 tests) — 7 new
    //           test files, 13 new tests
    //           covering Copy / Welcome /
    //           MarketDetail / LlmMgmt /
    //           LlmPerf / Trade routes +
    //           extra LlmMgmt edge cases.
    //           Brief test also fixed to
    //           target the Re-score CTA
    //           (the right one calls the
    //           IPC; Refresh just refetches
    //           the React Query cache).
    // v0.63b:   67/63/56/68 (522 tests) — 3
    //           high-coverage route tests
    //           (PnL 25%→75%, Wallets
    //           14%→75%, Copy 25%→80%),
    //           +19 tests total.  Big
    //           jump on branches (+4.6%)
    //           because all 3 routes are
    //           pure-render / pure-form
    //           patterns with many
    //           branch-rich UI states.
    // v0.64:    69/65/57/70 (545 tests) —
    //           keyboard-nav.test.tsx
    //           (+22 tests, 12%→80% cov)
    //           + fix a real bug in
    //           normalizeKey() where
    //           Cmd+? fired the help
    //           binding (modifier check
    //           was after the `?` mapping).
    // v0.65:    71/67/60/72 (569 tests) —
    //           • Markets.more.test.tsx
    //             (+7 tests, 17%→57% br)
    //           • LlmStep.test.tsx
    //             (+8 tests, 32%→90% stmts)
    //           • ModelLab.more.test.tsx
    //             (+9 active, 1 skipped;
    //             triggers a pre-existing
    //             rules-of-hooks violation
    //             in ModelLab.tsx that's
    //             tracked for v0.66)
    //           • keyboard-nav prefix-
    //             timeout test reworked
    //           Stmts +2.1, branches +2.2,
    //           funcs +2.9.
    // v0.66:    72/68/61/73 (575 tests) —
    //           • PolymarketStep.test.tsx
    //             (+6 tests, 17%→70% br)
    //           • ts-routes-components-lib
    //             density 65% → 70%
    //           • auto-bump version in
    //             update-readme-coverage
    //           • fix sendNotification
    //             missing from LlmStep mock
    //             (4 unhandled rejections)
    //           • ModelLab mock has
    //             zustand-like getState()
    // v0.67:    73/68/62/74 (580 tests) —
    //           • Copy.more.test.tsx
    //             (+5 tests, AddTargetModal
    //             validation + success + cap)
    //           • centralize @/ipc mock
    //             via test-mocks.ts
    //             (createIpcMock + createPrefsStoreMock)
    //           • ThemeSwitcher density
    //             (1.7% → 10.7%)
    //           • README badges sync
    //             check in CI (v0.67d)
    //           • test totals regex
    //             fix (match `N/**`)
    // v0.68:    74/69/63/75 (581 tests) —
    //           • Analysis.more.test.tsx
    //             (+5 tests, Run / mount /
    //             ErrorState paths)
    //           • @testing-library/user-event
    //             added as devDep (kept
    //             as future-option)
    //           • density polish (5 files
    //             bumped 4-5% → 10-11%)
    //           • CI cleanup coverage/ dir
    //             between runs (v0.68c)
    // v0.69-v0.74: coverage ratchet from
    //              74→83% (28 commits, +213 tests).
    //              See polyrocket-v0.7*-final.md
    //              for per-version details.
    // v0.75:    83/81/76/84 (794+ tests) — only
    //           stmts bumped (others tight).
    //           +25 tests across ModelLab
    //           round 3 + Settings round 2 +
    //           LlmMgmt round 4 + Wallets
    //           round 3. Headroom 0.19/0.64/
    //           0.9/0.54pp — branches tight.
    //
    // We exclude pure-presentation files
    // (BarChart, Sparkline, KpiCard, etc.)
    // that have no real logic — they're
    // rendered, not tested. ipc.ts is
    // excluded because it's a thin wrapper
    // around `invoke` (tested via the
    // integration tests that mock the IPC).
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json-summary', 'html'],
      include: [
        'src/components/feedback/**/*.{ts,tsx}',
        'src/components/welcome/**/*.{ts,tsx}',
        'src/lib/**/*.{ts,tsx}',
        'src/routes/**/*.{ts,tsx}',
        'src/stores/**/*.{ts,tsx}',
      ],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/test-setup.ts',
        'src/main.tsx',
        'src/ipc.ts',
        'src/types/**',
        '**/*.unused',
      ],
      thresholds: {
        statements: 86,
        branches: 83,
        functions: 79,
        lines: 87,
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
