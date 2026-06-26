// @vitest-environment happy-dom
/**
 * Motion token 合规性测试(v0.119 — Plan B)。
 *
 * 验证所有基础组件以及关键反馈组件统一使用 polyrocket 的 motion tokens:
 *
 *   - transition-colors / transition-transform 必须同时指定
 *     `duration-base ease-out-cubic`(或其他明确的 duration)
 *   - focus ring 必须使用 `focus-visible:`(而非 `focus:`)
 *   - disabled 透明度必须为 50(而非 40)
 *   - Modal 拥有进场动画
 *
 * 这些测试是防御性的——用于捕获未来出现的回归问题,
 * 例如某处新增了 `transition-colors` 但未指定 duration/ease
 * (会回退到 Tailwind 默认的 150ms ease-in-out,与 polyrocket 的
 * 160ms cubic-bezier 不一致)。
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
      // 不应使用普通的 `focus:ring-accent`
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
      // 读取文件验证 @media 规则是否存在。
      // (在生产构建中 Tailwind 会清理未使用的 CSS,但该规则
      // 位于 @layer base 内,因此会被保留。)
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
