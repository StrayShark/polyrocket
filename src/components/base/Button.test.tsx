// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { Button } from './Button';

describe('Button (v0.119 Cursor-merge)', () => {
  it('renders children text', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByRole('button', { name: 'Click me' })).toBeInTheDocument();
  });

  it('defaults to variant=secondary + size=md', () => {
    const { container } = render(<Button>x</Button>);
    const btn = container.querySelector('button')!;
    expect(btn.className).toContain('bg-surface-2'); // secondary
    expect(btn.className).toContain('h-9'); // md = 36px
  });

  it('applies rounded-md (8px) per Cursor button CTA radius', () => {
    const { container } = render(<Button>x</Button>);
    expect(container.firstChild).toBeDefined();
    expect((container.firstChild as HTMLElement).className).toContain('rounded-md');
  });

  describe('sizes (Cursor touch target system)', () => {
    it('xs = 24px (only for inline tag-style)', () => {
      const { container } = render(<Button size="xs">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('h-6');
    });
    it('sm = 32px (Cursor min touch target)', () => {
      const { container } = render(<Button size="sm">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('h-8');
    });
    it('md = 36px (polyrocket desktop default)', () => {
      const { container } = render(<Button size="md">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('h-9');
    });
    it('lg = 40px (matches Cursor button-primary)', () => {
      const { container } = render(<Button size="lg">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('h-10');
    });
    it('xl = 44px (matches Cursor button-download)', () => {
      const { container } = render(<Button size="xl">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('h-11');
    });
  });

  describe('variants', () => {
    it('primary uses bg-accent', () => {
      const { container } = render(<Button variant="primary">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('bg-accent');
    });
    it('secondary uses bg-surface-2', () => {
      const { container } = render(<Button variant="secondary">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('bg-surface-2');
    });
    it('ghost is transparent', () => {
      const { container } = render(<Button variant="ghost">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('bg-transparent');
    });
    it('danger uses bg-bear', () => {
      const { container } = render(<Button variant="danger">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('bg-bear');
    });
    it('success uses bg-bull', () => {
      const { container } = render(<Button variant="success">x</Button>);
      expect((container.firstChild as HTMLElement).className).toContain('bg-bull');
    });
  });

  describe('loading state', () => {
    it('disables the button when loading=true', () => {
      render(<Button loading>x</Button>);
      expect(screen.getByRole('button')).toBeDisabled();
    });
    it('renders a spinner when loading', () => {
      const { container } = render(<Button loading>x</Button>);
      expect(container.querySelector('.animate-spin')).toBeInTheDocument();
    });
  });

  describe('icons', () => {
    it('renders iconLeft before children', () => {
      render(
        <Button iconLeft={<span data-testid="left">L</span>}>x</Button>,
      );
      expect(screen.getByTestId('left')).toBeInTheDocument();
    });
    it('renders iconRight after children', () => {
      render(
        <Button iconRight={<span data-testid="right">R</span>}>x</Button>,
      );
      expect(screen.getByTestId('right')).toBeInTheDocument();
    });
  });

  it('triggers onClick when clicked', () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Click</Button>);
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('does not call onClick when disabled', () => {
    const onClick = vi.fn();
    render(
      <Button disabled onClick={onClick}>
        Click
      </Button>,
    );
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).not.toHaveBeenCalled();
  });

  it('applies focus-visible ring on accent', () => {
    const { container } = render(<Button>x</Button>);
    expect((container.firstChild as HTMLElement).className).toContain('focus-visible:ring-accent');
  });

  it('merges custom className', () => {
    const { container } = render(<Button className="my-btn">x</Button>);
    expect((container.firstChild as HTMLElement).className).toContain('my-btn');
  });
});
