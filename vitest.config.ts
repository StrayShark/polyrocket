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
    // v0.63b:   67/63/56/68 (504 tests) — 3
    //           high-coverage route tests
    //           (PnL 25%→75%, Wallets
    //           14%→75%, Copy 25%→80%),
    //           +18 tests total.  Big
    //           jump on branches (+4.6%)
    //           because all 3 routes are
    //           pure-render / pure-form
    //           patterns with many
    //           branch-rich UI states.
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
        statements: 67,
        branches: 63,
        functions: 56,
        lines: 68,
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
