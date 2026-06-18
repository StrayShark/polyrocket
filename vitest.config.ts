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
    // below the cutoff. We start with the
    // current actual numbers (48% stmts /
    // 48% lines) and plan to ratchet up
    // as we fill in the routes + business
    // components.
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
        statements: 64,
        branches: 57,
        functions: 52,
        lines: 64,
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
