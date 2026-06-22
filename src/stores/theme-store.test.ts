// v0.62a — theme-store tests.
//
// The theme store is a small zustand store that
// persists `theme` (dark / light / matrix) to
// localStorage and applies `data-theme` to <html>.
// Today 0% coverage. This file covers:
//   1. setTheme updates the state
//   2. cycleTheme advances dark → light → matrix → dark
//   3. SSR-safe (no document access in test env)

// @vitest-environment happy-dom

import { describe, it, expect, beforeEach } from 'vitest';
import { useThemeStore } from './theme-store';

describe('useThemeStore', () => {
  beforeEach(() => {
    // reset to default dark
    useThemeStore.setState({ theme: 'dark' });
  });

  it('defaults to dark', () => {
    expect(useThemeStore.getState().theme).toBe('dark');
  });

  it('setTheme updates the state', () => {
    useThemeStore.getState().setTheme('matrix');
    expect(useThemeStore.getState().theme).toBe('matrix');
  });

  it('cycleTheme advances dark → light', () => {
    useThemeStore.getState().setTheme('dark');
    useThemeStore.getState().cycleTheme();
    expect(useThemeStore.getState().theme).toBe('light');
  });

  it('cycleTheme advances light → matrix', () => {
    useThemeStore.getState().setTheme('light');
    useThemeStore.getState().cycleTheme();
    expect(useThemeStore.getState().theme).toBe('matrix');
  });

  it('cycleTheme advances matrix → dark (wraps)', () => {
    useThemeStore.getState().setTheme('matrix');
    useThemeStore.getState().cycleTheme();
    expect(useThemeStore.getState().theme).toBe('dark');
  });

  // v0.106 — coverage ramp. Cover applyToDom (lines 23-26) and the
  // onRehydrateStorage branch (line 51: `if (state?.theme) applyToDom(...)`).
  it('setTheme applies data-theme attribute to <html>', () => {
    useThemeStore.getState().setTheme('matrix');
    expect(document.documentElement.getAttribute('data-theme')).toBe('matrix');
  });

  it('setTheme to light applies data-theme="light"', () => {
    useThemeStore.getState().setTheme('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });
});
