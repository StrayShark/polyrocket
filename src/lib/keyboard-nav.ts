/**
 * L1 — Keyboard shortcut registry + hook.
 *
 * A "Gmail-style" two-key chord:
 *   1. Press `g` (no modifier) → enters "pending prefix" mode
 *   2. Press a second key within 1.2s → fires the binding
 *      e.g. `g d` → /dashboard, `g m` → /markets, `g h` → /help
 *
 * Single-key shortcuts (no prefix):
 *   `?`           → open the help dialog
 *   `/`           → focus the global search box
 *   `Esc`         → close any open dialog / cancel pending prefix
 *
 * The hook returns a `useKeyboardNav()` API that mounts a single
 * document-level `keydown` listener (efficient — one listener, not
 * one per shortcut).
 *
 * A help dialog (`<KbdHelpDialog>`) renders the active bindings so
 * users can discover them.
 */

import { useEffect, useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';

export interface KbdBinding {
  /** Display label, e.g. "Go to Dashboard" */
  label: string;
  /** Sequence of keys (1 or 2). E.g. ['g', 'd'] or ['?']. */
  keys: string[];
  /** Side-effect on activation. */
  action: () => void;
}

const PREFIX_TIMEOUT_MS = 1200;

function normalizeKey(e: KeyboardEvent): string {
  // v0.64b — modifier check FIRST. Previously the `?`
  // mapping was first, which meant Cmd+? would still
  // fire the help binding (incorrect — Cmd+? is a
  // macOS system shortcut and should pass through).
  if (e.metaKey || e.ctrlKey || e.altKey) return '';  // ignore
  // We DO want to capture plain `?` (Shift+/), so we map
  // `?` → "?" and otherwise use `e.key`.
  if (e.key === '?' || (e.shiftKey && e.key === '/')) return '?';
  return e.key.toLowerCase();
}

/**
 * Build the canonical binding set. Tied to the 18 routes polyrocket
 * has — adding a route means adding a binding here in the same commit
 * (see docs/overview.md §4.2 for the route map).
 */
export function useNavBindings(opts: {
  onOpenHelp: () => void;
  onOpenSearch: () => void;
  onCloseDialog: () => void;
}): KbdBinding[] {
  const nav = useNavigate();
  return [
    // Two-key chords (g + key) — 9 most important routes
    { label: 'Go to Dashboard',  keys: ['g', 'd'], action: () => nav('/dashboard') },
    { label: 'Go to Markets',    keys: ['g', 'm'], action: () => nav('/markets') },
    { label: 'Go to Signals',    keys: ['g', 's'], action: () => nav('/signals') },
    { label: 'Go to Copy',       keys: ['g', 'c'], action: () => nav('/copy') },
    { label: 'Go to P&L',        keys: ['g', 'p'], action: () => nav('/pnl') },
    { label: 'Go to History',    keys: ['g', 'h'], action: () => nav('/history') },
    { label: 'Go to Wallets',    keys: ['g', 'w'], action: () => nav('/wallets') },
    { label: 'Go to Lab',        keys: ['g', 'l'], action: () => nav('/lab') },
    { label: 'Go to Settings',   keys: ['g', 's'], action: () => nav('/settings') },  // collides; takes precedence by definition order
    // Single-key shortcuts
    { label: 'Show keyboard shortcuts', keys: ['?'], action: opts.onOpenHelp },
    { label: 'Focus search',             keys: ['/'], action: opts.onOpenSearch },
    { label: 'Close dialog',             keys: ['escape'], action: opts.onCloseDialog },
  ];
}

/**
 * Mount the keyboard listener. Returns `pendingPrefix` so a
 * status indicator (e.g. "g…") can be shown.
 */
export function useKeyboardNav(bindings: KbdBinding[]) {
  const [pendingPrefix, setPendingPrefix] = useState<string | null>(null);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;

    const handler = (e: KeyboardEvent) => {
      // Don't capture when typing in a form field.
      const target = e.target as HTMLElement | null;
      if (target) {
        const tag = target.tagName;
        if (tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable) {
          return;
        }
      }
      const key = normalizeKey(e);
      if (!key) return;

      // Already in prefix mode?
      if (pendingPrefix) {
        // Look for a 2-key binding matching prefix + key
        const match = bindings.find(
          (b) => b.keys.length === 2 && b.keys[0] === pendingPrefix && b.keys[1] === key,
        );
        if (match) {
          e.preventDefault();
          match.action();
        }
        // Either way, exit prefix mode.
        setPendingPrefix(null);
        if (timer) { clearTimeout(timer); timer = null; }
        return;
      }

      // No prefix. Look for a 1-key binding.
      const single = bindings.find((b) => b.keys.length === 1 && b.keys[0] === key);
      if (single) {
        e.preventDefault();
        single.action();
        return;
      }

      // Look for a 2-key binding whose first key matches.
      const isPrefix = bindings.some(
        (b) => b.keys.length === 2 && b.keys[0] === key,
      );
      if (isPrefix) {
        e.preventDefault();
        setPendingPrefix(key);
        if (timer) { clearTimeout(timer); }
        timer = setTimeout(() => setPendingPrefix(null), PREFIX_TIMEOUT_MS);
      }
    };

    document.addEventListener('keydown', handler);
    return () => {
      document.removeEventListener('keydown', handler);
      if (timer) clearTimeout(timer);
    };
  }, [bindings, pendingPrefix]);

  const clearPrefix = useCallback(() => setPendingPrefix(null), []);

  return { pendingPrefix, clearPrefix };
}

/**
 * Pretty-print a key sequence for the help dialog.
 *  ['g', 'd']  → "G D"
 *  ['?']       → "?"
 *  ['escape']  → "Esc"
 */
export function formatKeys(keys: string[]): string {
  return keys
    .map((k) => {
      if (k === 'escape') return 'Esc';
      if (k === '/') return '/';
      if (k === '?') return '?';
      return k.toUpperCase();
    })
    .join(' ');
}
