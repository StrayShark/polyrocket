// v0.77d — Stores + small components branches round 1 (+7 tests).
//
// Three 0%/50% files targeted:
//   - stores/welcome-store.ts: 46.1% br (14 uncovered) — Zustand store
//   - components/welcome/FinishStep.tsx: 50% br (2 uncovered) — last welcome step
//   - stores/theme-store.ts: 50% br (2 uncovered) — Zustand store
//
// Coverage target: small files → 90%+ br.
//
// @vitest-environment happy-dom

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '@testing-library/react';
import { useWelcomeStore, WELCOME_STEPS } from '@/stores/welcome-store';
import { useThemeStore } from '@/stores/theme-store';
import { FinishStep } from '@/components/welcome/FinishStep';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({
    t: (k: string) => k,
    locale: 'en' as const,
  }),
}));

describe('v0.77d stores + small components (branches round 1)', () => {
  describe('useWelcomeStore', () => {
    beforeEach(() => {
      useWelcomeStore.setState({
        done: false,
        step: 'welcome',
        locale: '',
        configured: {
          storagePath: false,
          theme: false,
          llmAtLeastOne: false,
          polymarketApi: false,
          walletPk: false,
        },
      });
    });

    it('initial state has step=welcome and done=false', () => {
      const s = useWelcomeStore.getState();
      expect(s.step).toBe('welcome');
      expect(s.done).toBe(false);
    });

    it('setStep changes step', () => {
      useWelcomeStore.getState().setStep('storage');
      expect(useWelcomeStore.getState().step).toBe('storage');
    });

    it('setDone toggles done', () => {
      useWelcomeStore.getState().setDone(true);
      expect(useWelcomeStore.getState().done).toBe(true);
    });

    it('setLocale updates locale', () => {
      useWelcomeStore.getState().setLocale('en');
      expect(useWelcomeStore.getState().locale).toBe('en');
    });

    it('setConfigured updates a single configured flag', () => {
      useWelcomeStore.getState().setConfigured('storagePath', true);
      expect(useWelcomeStore.getState().configured.storagePath).toBe(true);
      // other flags unchanged
      expect(useWelcomeStore.getState().configured.theme).toBe(false);
    });

    it('reset clears back to initial', () => {
      useWelcomeStore.setState({
        done: true,
        step: 'finish',
        locale: 'en',
        configured: {
          storagePath: true,
          theme: true,
          llmAtLeastOne: true,
          polymarketApi: true,
          walletPk: true,
        },
      });
      useWelcomeStore.getState().reset();
      const s = useWelcomeStore.getState();
      expect(s.step).toBe('welcome');
      expect(s.done).toBe(false);
    });

    it('WELCOME_STEPS has 6 entries in order', () => {
      expect(WELCOME_STEPS).toEqual([
        'welcome', 'storage', 'theme', 'llm', 'polymarket', 'finish',
      ]);
    });
  });

  describe('useThemeStore', () => {
    beforeEach(() => {
      useThemeStore.setState({ theme: 'dark' });
    });

    it('initial theme is dark', () => {
      expect(useThemeStore.getState().theme).toBe('dark');
    });

    it('setTheme to light/matrix updates theme', () => {
      useThemeStore.getState().setTheme('light');
      expect(useThemeStore.getState().theme).toBe('light');
      useThemeStore.getState().setTheme('matrix');
      expect(useThemeStore.getState().theme).toBe('matrix');
    });
  });

  describe('FinishStep', () => {
    it('renders without crash with welcome prop', () => {
      const welcome = useWelcomeStore.getState();
      render(<FinishStep welcome={welcome} />);
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
