/**
 * L1 —— 带焦点陷阱的 Modal(v0.10c)。
 *
 * 真正的 modal 需要满足的 a11y 要求:
 *   1. 打开时焦点移入 modal 内
 *   2. Tab/Shift+Tab 在 modal 内可聚焦元素间循环
 *      (不能跳出到背后页面)
 *   3. Esc 关闭 modal
 *   4. 关闭时焦点恢复到触发元素
 *   5. modal 拥有 role="dialog" + aria-modal="true"
 *
 * 替代 v0.5c 仅处理 #3 的 Modal。
 *
 * v0.119 —— 新增进场/离场动画:
 *   - Backdrop 淡入淡出(`animate-modal-backdrop` 160ms)
 *   - Dialog 缩放 + 淡入(`animate-modal-dialog` 160ms)
 *   - 在 `prefers-reduced-motion` 下,globals.css 中的 @media
 *     规则会把 animation-duration 缩短到 0.01ms(基本瞬时)
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

  // 1. 保存之前的焦点 + 打开时聚焦首个可聚焦元素
  // 2. 设置焦点陷阱(Tab/Shift+Tab 循环)
  // 3. 关闭时恢复焦点
  useEffect(() => {
    if (!open) return;

    // 记住打开前被聚焦的元素
    lastFocusRef.current = document.activeElement as HTMLElement | null;

    // 将焦点移入 modal
    // 通过微任务执行,确保元素已经在 DOM 中
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
      // 焦点陷阱:在 dialog 内循环
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
      // 将焦点恢复到触发元素
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
