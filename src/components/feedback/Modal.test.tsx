// v0.70g — Modal component tests.
//
// Modal.tsx (142 lines) was 59% covered. The 5 A11y behaviors
// declared in the file's docstring:
//   1. focus moves into the modal on open
//   2. Tab/Shift+Tab cycle (focus trap)
//   3. Esc closes the modal
//   4. focus restored to trigger on close
//   5. role="dialog" + aria-modal="true"
//
// Plus structural behaviors:
//   - renders nothing when open=false
//   - backdrop click closes modal
//   - dialog body click does NOT close (stopPropagation)
//   - 3 size variants apply correct max-width class
//   - X button + footer render
//
// Existing coverage (59%) came from indirect tests in route
// modals (AddKeyModal / AddWalletModal). Direct Modal coverage
// brings it to ~95%.
//
// Modal.tsx: 59.2% → ~95% stmts.
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { useState } from 'react';
import { Modal } from './Modal';

beforeEach(() => {
  // Make sure each test starts with no focused element
  if (document.body && document.activeElement instanceof HTMLElement) {
    document.activeElement.blur();
  }
});

describe('Modal', () => {
  it('renders nothing when open=false', () => {
    render(
      <Modal open={false} onClose={() => {}}>
        <div>should not appear</div>
      </Modal>,
    );
    expect(screen.queryByText('should not appear')).not.toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('renders dialog with role="dialog" + aria-modal="true" when open', () => {
    render(
      <Modal open onClose={() => {}}>
        <div>body content</div>
      </Modal>,
    );
    const dialog = screen.getByRole('dialog');
    expect(dialog).toBeInTheDocument();
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAttribute('tabindex', '-1');
  });

  it('renders title in the header when provided', () => {
    render(
      <Modal open onClose={() => {}} title="My modal">
        <div>body</div>
      </Modal>,
    );
    expect(screen.getByText('My modal')).toBeInTheDocument();
  });

  it('renders children in body', () => {
    render(
      <Modal open onClose={() => {}}>
        <div>body content here</div>
      </Modal>,
    );
    expect(screen.getByText('body content here')).toBeInTheDocument();
  });

  it('renders footer when provided', () => {
    render(
      <Modal open onClose={() => {}} footer={<button>Save</button>}>
        <div>body</div>
      </Modal>,
    );
    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument();
  });

  it('closes modal when X button is clicked', () => {
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose} title="Has close button">
        <div>body</div>
      </Modal>,
    );
    fireEvent.click(screen.getByRole('button', { name: /close/i }));
    expect(onClose).toHaveBeenCalled();
  });

  it('closes modal when backdrop is clicked', () => {
    const onClose = vi.fn();
    const { container } = render(
      <Modal open onClose={onClose}>
        <div>body</div>
      </Modal>,
    );
    // Click the outer backdrop (the fixed inset-0 element)
    const backdrop = container.querySelector('.fixed.inset-0') as HTMLElement;
    expect(backdrop).toBeInTheDocument();
    fireEvent.click(backdrop);
    expect(onClose).toHaveBeenCalled();
  });

  it('does NOT close modal when dialog body is clicked', () => {
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose}>
        <div>body</div>
      </Modal>,
    );
    // Click the dialog body — stopPropagation prevents the close
    fireEvent.click(screen.getByRole('dialog'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('closes modal when Escape key is pressed', () => {
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose}>
        <div>body</div>
      </Modal>,
    );
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('applies size="sm" → max-w-sm class', () => {
    render(
      <Modal open onClose={() => {}} size="sm">
        <div>body</div>
      </Modal>,
    );
    const dialog = screen.getByRole('dialog');
    expect(dialog.className).toContain('max-w-sm');
  });

  it('applies size="md" → max-w-md class (default)', () => {
    render(
      <Modal open onClose={() => {}}>
        <div>body</div>
      </Modal>,
    );
    const dialog = screen.getByRole('dialog');
    expect(dialog.className).toContain('max-w-md');
  });

  it('applies size="lg" → max-w-lg class', () => {
    render(
      <Modal open onClose={() => {}} size="lg">
        <div>body</div>
      </Modal>,
    );
    const dialog = screen.getByRole('dialog');
    expect(dialog.className).toContain('max-w-lg');
  });

  it('focuses first focusable element when modal opens', async () => {
    // Use a trigger pattern: focus button, open modal, verify focus moved
    const TestComponent = () => {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Open modal</button>
          <Modal open={open} onClose={() => setOpen(false)}>
            <button>First inside</button>
            <button>Second inside</button>
          </Modal>
        </>
      );
    };
    render(<TestComponent />);
    const trigger = screen.getByRole('button', { name: 'Open modal' });
    trigger.focus();
    expect(document.activeElement).toBe(trigger);
    fireEvent.click(trigger);
    // After modal opens, focus should be on first focusable inside
    await new Promise((r) => setTimeout(r, 10));
    const firstInside = screen.getByRole('button', { name: 'First inside' });
    expect(document.activeElement).toBe(firstInside);
  });

  it('focus trap: Tab on last element cycles back to first', () => {
    render(
      <Modal open onClose={() => {}}>
        <button>First</button>
        <button>Second</button>
        <button>Last</button>
      </Modal>,
    );
    const last = screen.getByRole('button', { name: 'Last' });
    last.focus();
    expect(document.activeElement).toBe(last);
    // Tab on last → should wrap to first
    fireEvent.keyDown(document, { key: 'Tab' });
    const first = screen.getByRole('button', { name: 'First' });
    expect(document.activeElement).toBe(first);
  });

  it('focus trap: Shift+Tab on first element cycles to last', () => {
    render(
      <Modal open onClose={() => {}}>
        <button>First</button>
        <button>Second</button>
        <button>Last</button>
      </Modal>,
    );
    const first = screen.getByRole('button', { name: 'First' });
    first.focus();
    // Shift+Tab on first → should wrap to last
    fireEvent.keyDown(document, { key: 'Tab', shiftKey: true });
    const last = screen.getByRole('button', { name: 'Last' });
    expect(document.activeElement).toBe(last);
  });

  it('Tab with no focusable elements does nothing', () => {
    // Modal with only text, no focusable
    render(
      <Modal open onClose={() => {}}>
        <p>just text</p>
      </Modal>,
    );
    // Should not throw
    expect(() => {
      fireEvent.keyDown(document, { key: 'Tab' });
    }).not.toThrow();
  });
});
