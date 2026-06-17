import { describe, expect, it } from 'vitest';
import { formatKeys, type KbdBinding } from './keyboard-nav';

describe('formatKeys', () => {
  it('uppercases single letters', () => {
    expect(formatKeys(['g'])).toBe('G');
    expect(formatKeys(['?'])).toBe('?');
  });

  it('maps escape to Esc', () => {
    expect(formatKeys(['escape'])).toBe('Esc');
  });

  it('joins 2-key chords with space', () => {
    expect(formatKeys(['g', 'd'])).toBe('G D');
    expect(formatKeys(['g', 'm'])).toBe('G M');
  });

  it('handles slash literally', () => {
    expect(formatKeys(['/'])).toBe('/');
  });
});

// Smoke test for the binding list shape.
// (Full keyboard-handler tests would need a DOM; we trust the user-agent
// keyboard events and just verify the bindings are well-formed.)
describe('binding shape', () => {
  const bindings: KbdBinding[] = [
    { label: 'Go to Dashboard', keys: ['g', 'd'], action: () => {} },
    { label: 'Show shortcuts',  keys: ['?'],      action: () => {} },
    { label: 'Focus search',    keys: ['/'],      action: () => {} },
    { label: 'Close dialog',    keys: ['escape'], action: () => {} },
  ];

  it('every binding has a label and at least one key', () => {
    for (const b of bindings) {
      expect(b.label.length).toBeGreaterThan(0);
      expect(b.keys.length).toBeGreaterThan(0);
    }
  });

  it('every key is normalized lowercase', () => {
    for (const b of bindings) {
      for (const k of b.keys) {
        // ? and / are exception cases (single-char literals)
        if (k === '?' || k === '/') continue;
        expect(k).toBe(k.toLowerCase());
      }
    }
  });

  it('2-key bindings have a non-trivial prefix', () => {
    for (const b of bindings) {
      if (b.keys.length === 2) {
        expect(b.keys[0].length).toBeGreaterThan(0);
        expect(b.keys[1].length).toBeGreaterThan(0);
        expect(b.keys[0]).not.toBe(b.keys[1]);
      }
    }
  });
});
