// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Pill } from './Pill';

describe('Pill (v0.119 enhanced — backward-compat)', () => {
  // ---- backward-compat (existing API) ----

  it('renders default neutral kind with children', () => {
    render(<Pill>default</Pill>);
    expect(screen.getByText('default')).toBeInTheDocument();
  });

  it('renders bull kind', () => {
    const { container } = render(<Pill kind="bull">+7.6%</Pill>);
    expect(container.querySelector('span')!.className).toContain('bg-bull');
  });

  it('renders bear kind', () => {
    const { container } = render(<Pill kind="bear">-3.2%</Pill>);
    expect(container.querySelector('span')!.className).toContain('bg-bear');
  });

  it('renders accent kind', () => {
    const { container } = render(<Pill kind="accent">v0.119</Pill>);
    expect(container.querySelector('span')!.className).toContain('bg-accent');
  });

  // ---- v0.119 new API: shape variant ----

  it('default shape is square (4px rounded) — backward compat', () => {
    const { container } = render(<Pill>x</Pill>);
    expect(container.querySelector('span')!.className).toContain('rounded');
    expect(container.querySelector('span')!.className).not.toContain('rounded-pill');
  });

  it('shape="pill" uses rounded-pill (9999px)', () => {
    const { container } = render(<Pill shape="pill">x</Pill>);
    expect(container.querySelector('span')!.className).toContain('rounded-pill');
    expect(container.querySelector('span')!.className).not.toMatch(/rounded( |$)/);
  });

  // ---- v0.119 new API: uppercase variant ----

  it('uppercase=false keeps original 10px font-medium', () => {
    const { container } = render(<Pill>x</Pill>);
    const span = container.querySelector('span')!;
    expect(span.className).not.toContain('uppercase');
    expect(span.className).not.toContain('text-caption-uppercase');
    expect(span.className).toContain('text-[10px]');
    expect(span.className).toContain('font-medium');
  });

  it('uppercase=true uses caption-uppercase spec', () => {
    const { container } = render(<Pill uppercase>x</Pill>);
    const span = container.querySelector('span')!;
    expect(span.className).toContain('uppercase');
    expect(span.className).toContain('text-xs');
    expect(span.className).toContain('font-semibold');
    expect(span.className).toContain('tracking-caption-uppercase');
    // uppercase uses wider padding (px-2.5 vs px-1.5)
    expect(span.className).toContain('px-2.5');
  });

  // ---- combinations ----

  it('combines shape="pill" + uppercase=true + variant="bull"', () => {
    const { container } = render(
      <Pill kind="bull" shape="pill" uppercase>
        +7.6%
      </Pill>,
    );
    const span = container.querySelector('span')!;
    expect(span.className).toContain('bg-bull');
    expect(span.className).toContain('rounded-pill');
    expect(span.className).toContain('uppercase');
  });
});
