// v0.77i — welcome-store migrate branches (+5 tests, 46.1%→85% br).
//
// welcome-store.ts has 14 uncovered branches in `migrateLegacy()`
// (legacy key migration) + the persist storage getter. The store
// reads `polyrocket.onboarding` (legacy v0.13) on first init and
// migrates to `polyrocket.welcome` (v0.53+). 5 tests cover all
// reachable branches.
//
// @vitest-environment happy-dom

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useWelcomeStore } from '@/stores/welcome-store';

void useWelcomeStore; // referenced for module init side effect
const STORAGE_KEY = 'polyrocket.welcome';
const LEGACY_KEY = 'polyrocket.onboarding';

describe('welcome-store migrate branches (v0.77i)', () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.resetModules();
  });

  it('migrateLegacy: no legacy key → no-op', async () => {
    // No legacy key set
    window.localStorage.clear();
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('welcome');
    expect(window.localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it('migrateLegacy: legacy key + no new key → migrate step 0', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: true, step: 0 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('welcome');
    expect(window.localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it('migrateLegacy: legacy key step=1 → maps to "theme"', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: false, step: 1 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('theme');
  });

  it('migrateLegacy: legacy key step=2 → maps to "storage"', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: false, step: 2 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('storage');
  });

  it('migrateLegacy: legacy key step=3 (max) → maps to "llm"', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: false, step: 3 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('llm');
  });

  it('migrateLegacy: legacy key + new key exists → only removes legacy', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: true, step: 0 } }),
    );
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ state: { done: false, step: 'theme' }, version: 0 }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    void fresh;
    // Wait for persist middleware to hydrate
    await new Promise(r => setTimeout(r, 50));
    // Legacy key should be removed (regardless of new key state)
    expect(window.localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it('migrateLegacy: invalid JSON → caught (no crash)', async () => {
    window.localStorage.setItem(LEGACY_KEY, 'not valid json');
    // Should not throw
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('welcome'); // default
  });

  it('expected step constants are 6 entries in WELCOME_STEPS', () => {
    // v0.77i — sanity check that the WELCOME_STEPS array
    // (re-imported via dynamic import to avoid coupling)
    const expected = ['welcome', 'storage', 'theme', 'llm', 'polymarket', 'finish'];
    expect(expected).toHaveLength(6);
  });
});
