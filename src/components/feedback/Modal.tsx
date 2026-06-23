/**
 * L1 — Modal with focus trap (v0.10c).
 *
 * A11y requirements for a true modal:
 *   1. focus moves into the modal when it opens
 *   2. Tab/Shift+Tab cycle through focusable elements inside
 *      the modal (never escape to the page behind)
 *   3. Esc closes the modal
 *   4. focus is restored to the trigger element on close
 *   5. The modal has role="dialog" + aria-modal="true"
 *
 * Replaces the v0.5c Modal which only handled #3.
 *
 * v0.119 — added enter/exit animation:
 *   - Backdrop fades in/out (`animate-modal-backdrop` 160ms)
 *   - Dialog scales + fades (`animate-modal-dialog` 160ms)
 *   - On `prefers-reduced-motion`, the @media rule in globals.css
 *     shortens animation-duration to 0.01ms (effectively instant)
 */

import { useEffect, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { cn } from '@/lib/cn';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  size?: 'sm' | 'md' | 'lg';
}

const SIZE = { sm: 'max-w-sm', md: 'max-w-md', lg: 'max-w-lg' } as const;

const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export function Modal({ open, onClose, title, children, footer, size = 'md' }: ModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const lastFocusRef = useRef<HTMLElement | null>(null);

  // 1. Save previous focus + focus first focusable on open
  // 2. Set up focus trap (Tab/Shift+Tab cycle)
  // 3. Restore focus on close
  useEffect(() => {
    if (!open) return;

    // Remember what was focused before
    lastFocusRef.current = document.activeElement as HTMLElement | null;

    // Move focus into the modal
    // We do it on a microtask so the element is in the DOM
    queueMicrotask(() => {
      const dialog = dialogRef.current;
      if (!dialog) return;
      const focusables = dialog.querySelectorAll<HTMLElement>(FOCUSABLE);
      const first = focusables[0] ?? dialog;
      first.focus();
    });

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key !== 'Tab') return;
      // Focus trap: cycle within the dialog
      const dialog = dialogRef.current;
      if (!dialog) return;
      const focusables = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE))
        .filter((el) => !el.hasAttribute('disabled') && el.tabIndex !== -1);
      if (focusables.length === 0) {
        e.preventDefault();
        return;
      }
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey) {
        if (active === first || !dialog.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (active === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };

    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      // Restore focus to the trigger
      if (lastFocusRef.current && document.body.contains(lastFocusRef.current)) {
        lastFocusRef.current.focus();
      }
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div
      data-testid="modal-backdrop"
      className="fixed inset-0 z-50 grid place-items-center bg-black/40 animate-modal-backdrop"
      onClick={onClose}
    >
      <div
        ref={dialogRef}
        data-testid="modal-dialog"
        className={cn(
          'w-[90vw] rounded-lg border bg-surface border-border shadow-xl outline-none animate-modal-dialog',
          SIZE[size],
        )}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        tabIndex={-1}
      >
        {title && (
          <div className="flex items-center justify-between gap-2 px-4 py-3 border-b border-border">
            <div className="text-title-sm font-semibold">{title}</div>
            <button
              onClick={onClose}
              className="text-muted hover:text-fg"
              aria-label="close"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        )}
        <div className="p-4">{children}</div>
        {footer && (
          <div className="px-4 py-3 border-t border-border flex justify-end gap-2">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
