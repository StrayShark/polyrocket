// @vitest-environment happy-dom
/**
 * Motion token compliance tests (v0.119 — Plan B).
 *
 * Verify all base components and key feedback components use
 * polyrocket's motion tokens consistently:
 *
 *   - transition-colors / transition-transform MUST also specify
 *     `duration-base ease-out-cubic` (or other explicit duration)
 *   - focus rings MUST use `focus-visible:` (not `focus:`)
 *   - disabled opacity MUST be 50 (not 40)
 *   - Modal has enter animations
 *
 * These tests are defensive — they catch future regressions
 * where someone adds a `transition-colors` without specifying
 * duration/ease (falling back to Tailwind's 150ms ease-in-out
 * default which differs from polyrocket's 160ms cubic-bezier).
 */
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Button } from './Button';
import { Input } from './Input';
import { Modal } from '../feedback/Modal';

describe('Motion tokens (v0.119 Plan B)', () => {
  describe('Button', () => {
    it('uses duration-base + ease-out-cubic on transition-colors', () => {
      const { container } = render(<Button>x</Button>);
      const cls = (container.firstChild as HTMLElement).className;
      expect(cls).toContain('duration-base');
      expect(cls).toContain('ease-out-cubic');
    });

    it('has active:scale-[0.98] for press feedback', () => {
      const { container } = render(<Button>x</Button>);
      const cls = (container.firstChild as HTMLElement).className;
      expect(cls).toContain('active:scale-');
    });

    it('disables active:scale when disabled (no press feedback on disabled)', () => {
      const { container } = render(<Button disabled>x</Button>);
      const cls = (container.firstChild as HTMLElement).className;
      expect(cls).toContain('disabled:active:scale-100');
    });

    it('has transition-transform for press animation', () => {
      const { container } = render(<Button>x</Button>);
      const cls = (container.firstChild as HTMLElement).className;
      expect(cls).toContain('transition-transform');
    });
  });

  describe('Input', () => {
    it('uses duration-base + ease-out-cubic', () => {
      const { container } = render(<Input aria-label="test" />);
      const input = container.querySelector('input')!;
      expect(input.className).toContain('duration-base');
      expect(input.className).toContain('ease-out-cubic');
    });

    it('uses focus-visible (not focus:) for ring', () => {
      const { container } = render(<Input aria-label="test" />);
      const input = container.querySelector('input')!;
      expect(input.className).toContain('focus-visible:ring-accent');
      // Should NOT have plain `focus:ring-accent`
      expect(input.className).not.toMatch(/\sfocus:ring/);
    });
  });

  describe('Modal (v0.119 enter animation)', () => {
    it('renders nothing when closed', () => {
      const { container } = render(
        <Modal open={false} onClose={() => {}}>
          x
        </Modal>,
      );
      expect(container.firstChild).toBeNull();
    });

    it('has backdrop with animate-modal-backdrop class', () => {
      render(
        <Modal open={true} onClose={() => {}}>
          x
        </Modal>,
      );
      const backdrop = screen.getByTestId('modal-backdrop');
      expect(backdrop.className).toContain('animate-modal-backdrop');
    });

    it('has dialog with animate-modal-dialog class', () => {
      render(
        <Modal open={true} onClose={() => {}}>
          x
        </Modal>,
      );
      const dialog = screen.getByTestId('modal-dialog');
      expect(dialog.className).toContain('animate-modal-dialog');
    });
  });

  describe('Reduced motion a11y (v0.119)', () => {
    it('globals.css has prefers-reduced-motion handler', async () => {
      // Read the file to verify the @media rule exists.
      // (In production builds, Tailwind purges unused CSS, but
      // the rule is in @layer base so it stays.)
      const fs = await import('node:fs/promises');
      const css = await fs.readFile(
        '/Users/dutongxue/work2/polyrocket/src/styles/globals.css',
        'utf-8',
      );
      expect(css).toContain('prefers-reduced-motion');
      expect(css).toMatch(/@media\s*\(\s*prefers-reduced-motion:\s*reduce\s*\)/);
    });
  });

  describe('Disabled opacity consistency', () => {
    it('Button uses disabled:opacity-50 (not 40)', () => {
      const { container } = render(<Button disabled>x</Button>);
      const cls = (container.firstChild as HTMLElement).className;
      expect(cls).toContain('disabled:opacity-50');
      expect(cls).not.toContain('disabled:opacity-40');
    });
  });
});
