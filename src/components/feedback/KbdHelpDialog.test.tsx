// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { render, screen, within, renderHook, act } from '@testing-library/react';
import { KbdHelpDialog, useKbdHelpDialog } from './KbdHelpDialog';
import type { KbdBinding } from '@/lib/keyboard-nav';

const bindings: KbdBinding[] = [
  { label: 'Go to Dashboard',  keys: ['g', 'd'], action: () => {} },
  { label: 'Go to Markets',    keys: ['g', 'm'], action: () => {} },
  { label: 'Show keyboard shortcuts', keys: ['?'], action: () => {} },
  { label: 'Focus search',     keys: ['/'],      action: () => {} },
  { label: 'Close dialog',     keys: ['escape'], action: () => {} },
];

describe('KbdHelpDialog', () => {
  it('renders the dialog with bindings when open', () => {
    render(<KbdHelpDialog bindings={bindings} open onClose={() => {}} />);
    const dialog = screen.getByRole('dialog');
    const inDialog = within(dialog);
    // 对话框标题现在已被翻译(英文为 "Keyboard shortcuts")
    expect(inDialog.getByText('Keyboard shortcuts')).toBeInTheDocument();
    expect(inDialog.getByText('Go to Dashboard')).toBeInTheDocument();
    expect(inDialog.getByText('Go to Markets')).toBeInTheDocument();
    expect(inDialog.getByText('Show keyboard shortcuts')).toBeInTheDocument();
  });

  it('groups 1-key and 2-key bindings separately', () => {
    render(<KbdHelpDialog bindings={bindings} open onClose={() => {}} />);
    const dialog = screen.getByRole('dialog');
    const inDialog = within(dialog);
    expect(inDialog.getByText('Navigate')).toBeInTheDocument();
    expect(inDialog.getByText('Actions')).toBeInTheDocument();
  });

  it('formats key sequences for display', () => {
    render(<KbdHelpDialog bindings={bindings} open onClose={() => {}} />);
    const dialog = screen.getByRole('dialog');
    const row = within(dialog).getByText('Go to Dashboard').closest('div');
    expect(row).not.toBeNull();
    expect(row!.querySelectorAll('kbd').length).toBe(2);
  });

  it('does not render when closed', () => {
    render(<KbdHelpDialog bindings={bindings} open={false} onClose={() => {}} />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  // v0.116 —— 覆盖率提升第 16 轮。覆盖 useKbdHelpDialog hook
  // (78-82 行:useState + openDialog / closeDialog)。
  it('useKbdHelpDialog: openDialog sets open=true, closeDialog sets open=false', () => {
    const { result } = renderHook(() => useKbdHelpDialog());
    expect(result.current.open).toBe(false);
    act(() => {
      result.current.openDialog();
    });
    expect(result.current.open).toBe(true);
    act(() => {
      result.current.closeDialog();
    });
    expect(result.current.open).toBe(false);
  });
});
