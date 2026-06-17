/**
 * Vitest setup file — runs before every test file.
 *
 * - Imports `@testing-library/jest-dom` for `toBeInTheDocument()`
 *   and friends in .test.tsx files
 * - (Reserved for future global mocks / window.matchMedia shims)
 */
import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// happy-dom doesn't auto-clean DOM between tests, which leaks
// modals / portals from one test into the next.
afterEach(() => cleanup());
