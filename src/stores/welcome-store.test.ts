// v0.53c — vitest coverage for the welcome store.
//
// 1. defaults are correct
// 2. setStep / setDone / setConfigured transitions
// 3. pendingConfigs returns the right keys
// 4. legacy polyrocket.onboarding localStorage key
//    is migrated on first read

import { describe, it, expect, beforeEach } from 'vitest';
import {
  useWelcomeStore,
  WELCOME_STEPS,
  pendingConfigs,
} from './welcome-store';

beforeEach(() => {
  // Reset the store between tests.
  useWelcomeStore.getState().reset();
  // Wipe the localStorage so the legacy migration
  // tests start from a clean slate.
  if (typeof window !== 'undefined') {
    window.localStorage.clear();
  }
});

describe('welcome store defaults', () => {
  it('starts on the welcome step', () => {
    const s = useWelcomeStore.getState();
    expect(s.step).toBe('welcome');
    expect(s.done).toBe(false);
    expect(s.configured).toEqual({
      storagePath: false,
      theme: false,
      llmAtLeastOne: false,
      polymarketApi: false,
      walletPk: false,
    });
  });

  it('exposes 6 step names in order', () => {
    expect(WELCOME_STEPS).toEqual([
      'welcome',
      'storage',
      'theme',
      'llm',
      'polymarket',
      'finish',
    ]);
  });
});

describe('welcome store transitions', () => {
  it('setStep advances the step', () => {
    useWelcomeStore.getState().setStep('llm');
    expect(useWelcomeStore.getState().step).toBe('llm');
  });

  it('setDone=true jumps to finish', () => {
    useWelcomeStore.getState().setStep('llm');
    useWelcomeStore.getState().setDone(true);
    const s = useWelcomeStore.getState();
    expect(s.done).toBe(true);
    expect(s.step).toBe('finish');
  });

  it('setDone=false resets to welcome', () => {
    useWelcomeStore.getState().setStep('polymarket');
    useDone();
    function useDone() {
      useWelcomeStore.getState().setDone(false);
    }
    const s = useWelcomeStore.getState();
    expect(s.step).toBe('welcome');
  });

  it('setConfigured updates the matching flag', () => {
    const s = useWelcomeStore.getState();
    s.setConfigured('llmAtLeastOne', true);
    s.setConfigured('polymarketApi', true);
    expect(useWelcomeStore.getState().configured.llmAtLeastOne).toBe(true);
    expect(useWelcomeStore.getState().configured.polymarketApi).toBe(true);
    // Other flags stay false.
    expect(useWelcomeStore.getState().configured.walletPk).toBe(false);
  });
});

describe('pendingConfigs', () => {
  it('returns the list of unconfigured items', () => {
    const s = useWelcomeStore.getState();
    // Nothing configured yet.
    expect(pendingConfigs(s)).toEqual(['llm', 'polymarket', 'wallet']);
    s.setConfigured('llmAtLeastOne', true);
    expect(pendingConfigs(useWelcomeStore.getState())).toEqual([
      'polymarket',
      'wallet',
    ]);
    s.setConfigured('polymarketApi', true);
    s.setConfigured('walletPk', true);
    expect(pendingConfigs(useWelcomeStore.getState())).toEqual([]);
  });

  it('storage + theme are NOT in the pending list (they\'re setup, not secrets)', () => {
    const s = useWelcomeStore.getState();
    // Even unconfigured, storage/theme don't appear
    // — they're nice-to-have, not blocking. The
    // user can complete the wizard without picking
    // a custom path or a non-default theme.
    expect(pendingConfigs(s)).not.toContain('storage');
    expect(pendingConfigs(s)).not.toContain('theme');
  });
});

describe('legacy polyrocket.onboarding migration', () => {
  it('copies done + step from the old key on first read', () => {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(
      'polyrocket.onboarding',
      JSON.stringify({
        state: { done: true, step: 3 },
        version: 0,
      }),
    );
    // Trigger migration by reading.
    useWelcomeStore.persist.rehydrate();
    // After migration, the old key is gone, the
    // new key exists with done=true and step set
    // to a sensible WelcomeStep.
    expect(window.localStorage.getItem('polyrocket.onboarding')).toBeNull();
    const newRaw = window.localStorage.getItem('polyrocket.welcome');
    expect(newRaw).not.toBeNull();
    const parsed = JSON.parse(newRaw as string);
    expect(parsed.state.done).toBe(true);
    // Old step 3 (llm) maps to 'llm'.
    expect(parsed.state.step).toBe('llm');
  });
});
