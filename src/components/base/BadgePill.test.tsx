// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { BadgePill } from './BadgePill';

describe('BadgePill (v0.119 Cursor-merge)', () => {
  it('renders children content', () => {
    render(<BadgePill>v0.119</BadgePill>);
    expect(screen.getByText('v0.119')).toBeInTheDocument();
  });

  it('applies uppercase + tracking + pill shape per Cursor caption-uppercase spec', () => {
    const { container } = render(<BadgePill>v0.119</BadgePill>);
    const pill = container.querySelector('[data-testid="badge-pill"]')!;
    expect(pill.className).toMatch(/uppercase/);
    expect(pill.className).toMatch(/tracking-caption-uppercase/);
    expect(pill.className).toMatch(/rounded-pill/);
    expect(pill.className).toMatch(/text-xs/);
    expect(pill.className).toMatch(/font-semibold/);
  });

  it('renders all 5 variants with correct data-variant attribute', () => {
    const variants = ['neutral', 'accent', 'bull', 'bear', 'warning'] as const;
    for (const v of variants) {
      const { unmount } = render(<BadgePill variant={v}>x</BadgePill>);
      expect(screen.getByTestId('badge-pill')).toHaveAttribute('data-variant', v);
      unmount();
    }
  });

  it('uses accent classes for accent variant', () => {
    const { container } = render(<BadgePill variant="accent">+7.6%</BadgePill>);
    const pill = container.querySelector('[data-testid="badge-pill"]')!;
    expect(pill.className).toContain('bg-accent');
    expect(pill.className).toContain('text-accent');
  });

  it('uses bull classes for bull variant', () => {
    const { container } = render(<BadgePill variant="bull">BULL</BadgePill>);
    const pill = container.querySelector('[data-testid="badge-pill"]')!;
    expect(pill.className).toContain('bg-bull');
    expect(pill.className).toContain('text-bull');
  });

  it('uses bear classes for bear variant', () => {
    const { container } = render(<BadgePill variant="bear">BEAR</BadgePill>);
    const pill = container.querySelector('[data-testid="badge-pill"]')!;
    expect(pill.className).toContain('bg-bear');
    expect(pill.className).toContain('text-bear');
  });

  it('passes through additional className', () => {
    const { container } = render(
      <BadgePill variant="warning" className="my-extra-class">
        WIP
      </BadgePill>,
    );
    expect(
      container.querySelector('[data-testid="badge-pill"]')!.className,
    ).toContain('my-extra-class');
  });

  it('default variant is neutral', () => {
    render(<BadgePill>x</BadgePill>);
    expect(screen.getByTestId('badge-pill')).toHaveAttribute('data-variant', 'neutral');
  });
});
