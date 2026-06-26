// v0.70g — Modal 组件测试。
//
// Modal.tsx(142 行)此前覆盖率为 59%。文件 docstring 中
// 声明的 5 项 a11y 行为:
//   1. 打开时焦点移入 modal
//   2. Tab/Shift+Tab 循环(焦点陷阱)
//   3. Esc 关闭 modal
//   4. 关闭时焦点恢复到触发元素
//   5. role="dialog" + aria-modal="true" (a11y 属性)
//
// 加上结构性行为:
//   - open=false 时不渲染任何内容
//   - 点击 backdrop 关闭 modal
//   - 点击 dialog body 不会关闭(stopPropagation)
//   - 3 种 size 应用正确的 max-width class
//   - X 按钮 + footer 渲染
//
// 既有覆盖率(59%)来自路由 modal 的间接测试
// (AddKeyModal / AddWalletModal)。直接覆盖 Modal 后
// 提升到约 95%。
//
// Modal.tsx:59.2% → 约 95% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { useState } from 'react';
import { Modal } from './Modal';

beforeEach(() => {
  // 确保每个测试开始时没有聚焦元素
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
    // 点击外层 backdrop(fixed inset-0 元素)
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
    // 点击 dialog body —— stopPropagation 阻止关闭
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
    // 使用 trigger 模式:聚焦按钮,打开 modal,验证焦点已移动
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
    // modal 打开后,焦点应在内部的第一个可聚焦元素上
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
    // 在最后一个上按 Tab → 应回卷到第一个
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
    // 在第一个上按 Shift+Tab → 应回卷到最后一个
    fireEvent.keyDown(document, { key: 'Tab', shiftKey: true });
    const last = screen.getByRole('button', { name: 'Last' });
    expect(document.activeElement).toBe(last);
  });

  it('Tab with no focusable elements does nothing', () => {
    // Modal 只包含文本,无可聚焦元素
    render(
      <Modal open onClose={() => {}}>
        <p>just text</p>
      </Modal>,
    );
    // 不应抛错
    expect(() => {
      fireEvent.keyDown(document, { key: 'Tab' });
    }).not.toThrow();
  });
});
