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
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
